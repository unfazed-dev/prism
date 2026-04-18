//! Subprocess invocation abstraction.
//!
//! All code that shells out to external CLIs (claude, git, kill) goes through
//! [`CommandRunner`]. Production code uses [`SystemRunner`] (wraps
//! [`std::process::Command`]). Tests can inject a mock runner that returns
//! scripted responses — exercises error branches that would otherwise require
//! process-level mocking.

use std::io;
use std::path::Path;
use std::time::Duration;

/// Captured result of a single subprocess invocation.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// Exit code; `None` when terminated by a signal.
    pub status: Option<i32>,
    /// Captured stdout bytes.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes.
    pub stderr: Vec<u8>,
}

/// Outcome of a bounded-duration subprocess invocation.
#[derive(Debug, Clone)]
pub enum RunResult {
    /// Process finished within the deadline.
    Completed(CommandOutput),
    /// Deadline elapsed before process finished; caller may best-effort kill.
    TimedOut,
}

impl CommandOutput {
    /// `true` when the process exited with status 0.
    pub fn success(&self) -> bool {
        matches!(self.status, Some(0))
    }

    /// Lossy UTF-8 view of stdout.
    pub fn stdout_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }

    /// Lossy UTF-8 view of stderr.
    pub fn stderr_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.stderr)
    }
}

/// Abstraction over subprocess execution. Implementors:
/// - [`SystemRunner`] for production (`std::process::Command`).
/// - [`MockRunner`] for tests (scripted responses).
pub trait CommandRunner: Send + Sync {
    /// Execute `program` with `args`, optionally with `cwd` and `stdin`. Block
    /// until exit; return the captured output.
    fn run(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        stdin: Option<&str>,
    ) -> io::Result<CommandOutput>;

    /// Execute with a wall-clock `timeout`. Default impl delegates to
    /// [`CommandRunner::run`] (ignoring the deadline) — suitable for mocks.
    /// Production runners must override with a real bounded wait.
    fn run_timeout(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        stdin: Option<&str>,
        _timeout: Duration,
    ) -> io::Result<RunResult> {
        self.run(program, args, cwd, stdin)
            .map(RunResult::Completed)
    }
}

// ── Production runner ────────────────────────────────────────────────

/// Real subprocess execution via [`std::process::Command`].
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        stdin: Option<&str>,
    ) -> io::Result<CommandOutput> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut cmd = Command::new(program);
        cmd.args(args);
        if let Some(d) = cwd {
            cmd.current_dir(d);
        }
        if stdin.is_some() {
            cmd.stdin(Stdio::piped());
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            pipe.write_all(input.as_bytes())?;
            drop(pipe);
        }
        let output = child.wait_with_output()?;
        Ok(CommandOutput {
            status: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    fn run_timeout(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        stdin: Option<&str>,
        timeout: Duration,
    ) -> io::Result<RunResult> {
        use std::io::Write;
        use std::process::{Command, Stdio};
        use std::sync::mpsc;
        use std::thread;

        let mut cmd = Command::new(program);
        cmd.args(args);
        if let Some(d) = cwd {
            cmd.current_dir(d);
        }
        if stdin.is_some() {
            cmd.stdin(Stdio::piped());
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            pipe.write_all(input.as_bytes())?;
            drop(pipe);
        }

        let child_id = child.id();
        let (tx, rx) = mpsc::channel::<io::Result<std::process::Output>>();
        thread::spawn(move || {
            let _ = tx.send(child.wait_with_output());
        });

        match rx.recv_timeout(timeout) {
            Ok(Ok(output)) => Ok(RunResult::Completed(CommandOutput {
                status: output.status.code(),
                stdout: output.stdout,
                stderr: output.stderr,
            })),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                let _ = Command::new("kill")
                    .arg("-9")
                    .arg(child_id.to_string())
                    .status();
                Ok(RunResult::TimedOut)
            }
        }
    }
}

// ── Test runner ──────────────────────────────────────────────────────

/// One registered mock-runner script entry. `timeout` response, when present,
/// is consumed by [`CommandRunner::run_timeout`]; otherwise that path falls
/// back to wrapping the non-timeout response in `RunResult::Completed`.
type MockScript = (
    String,
    Option<String>,
    io::Result<CommandOutput>,
    Option<io::Result<RunResult>>,
);

/// Scripted test runner. Register `(program, first_arg)` pairs to canned
/// responses; unknown invocations return `ErrorKind::NotFound`.
#[derive(Default)]
pub struct MockRunner {
    scripts: std::sync::Mutex<Vec<MockScript>>,
}

impl MockRunner {
    /// Create a mock runner with no scripted responses.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a response for `(program, None)` matching any args, or
    /// `(program, Some(first_arg))` matching only that first argument.
    /// Scripts match in registration order; each consumed once.
    pub fn expect(
        &self,
        program: &str,
        first_arg: Option<&str>,
        response: io::Result<CommandOutput>,
    ) {
        self.scripts.lock().unwrap_or_else(|e| e.into_inner()).push((
            program.to_string(),
            first_arg.map(str::to_string),
            response,
            None,
        ));
    }

    /// Register a timeout-path response. The `run` field is a placeholder and
    /// will not be consumed unless [`CommandRunner::run`] (not `run_timeout`)
    /// is called against this script; use [`MockRunner::not_found`] to make
    /// such a mismatch loud.
    pub fn expect_timeout(
        &self,
        program: &str,
        first_arg: Option<&str>,
        run_placeholder: io::Result<CommandOutput>,
        timeout_response: io::Result<RunResult>,
    ) {
        self.scripts.lock().unwrap_or_else(|e| e.into_inner()).push((
            program.to_string(),
            first_arg.map(str::to_string),
            run_placeholder,
            Some(timeout_response),
        ));
    }

    /// Convenience: build an `Ok(CommandOutput)` with status 0 and the given stdout.
    pub fn ok(stdout: &str) -> io::Result<CommandOutput> {
        Ok(CommandOutput {
            status: Some(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    /// Convenience: build an `Ok(CommandOutput)` with non-zero exit and stderr.
    pub fn fail(code: i32, stderr: &str) -> io::Result<CommandOutput> {
        Ok(CommandOutput {
            status: Some(code),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    /// Convenience: simulate the program being missing from PATH.
    pub fn not_found() -> io::Result<CommandOutput> {
        Err(io::Error::new(io::ErrorKind::NotFound, "program not found"))
    }
}

impl CommandRunner for MockRunner {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        _cwd: Option<&Path>,
        _stdin: Option<&str>,
    ) -> io::Result<CommandOutput> {
        let mut scripts = self.scripts.lock().unwrap_or_else(|e| e.into_inner());
        let first_arg = args.first().copied();
        let idx = scripts
            .iter()
            .position(|(p, a, _, _)| p == program && (a.is_none() || a.as_deref() == first_arg));
        if let Some(i) = idx {
            let (_, _, response, _) = scripts.remove(i);
            response
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no mock script for {program} {args:?}"),
            ))
        }
    }

    fn run_timeout(
        &self,
        program: &str,
        args: &[&str],
        _cwd: Option<&Path>,
        _stdin: Option<&str>,
        _timeout: Duration,
    ) -> io::Result<RunResult> {
        let mut scripts = self.scripts.lock().unwrap_or_else(|e| e.into_inner());
        let first_arg = args.first().copied();
        let idx = scripts
            .iter()
            .position(|(p, a, _, _)| p == program && (a.is_none() || a.as_deref() == first_arg));
        if let Some(i) = idx {
            let (_, _, run_resp, timeout_resp) = scripts.remove(i);
            match timeout_resp {
                Some(r) => r,
                None => run_resp.map(RunResult::Completed),
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no mock script for {program} {args:?}"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_returns_scripted_ok() {
        let mock = MockRunner::new();
        mock.expect("git", Some("status"), MockRunner::ok("hello"));
        let out = mock.run("git", &["status"], None, None).unwrap();
        assert_eq!(out.status, Some(0));
        assert_eq!(out.stdout_str(), "hello");
        assert!(out.success());
    }

    #[test]
    fn mock_returns_scripted_fail() {
        let mock = MockRunner::new();
        mock.expect("git", None, MockRunner::fail(128, "fatal"));
        let out = mock.run("git", &["anything"], None, None).unwrap();
        assert_eq!(out.status, Some(128));
        assert!(!out.success());
        assert_eq!(out.stderr_str(), "fatal");
    }

    #[test]
    fn mock_not_found_on_unscripted_call() {
        let mock = MockRunner::new();
        let err = mock.run("unmocked", &[], None, None).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn mock_not_found_variant() {
        let mock = MockRunner::new();
        mock.expect("claude", None, MockRunner::not_found());
        let err = mock.run("claude", &["-p"], None, None).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn system_runner_echo_roundtrip() {
        let runner = SystemRunner;
        let out = runner
            .run("sh", &["-c", "echo hello"], None, None)
            .expect("echo runs");
        assert_eq!(out.stdout_str().trim(), "hello");
        assert!(out.success());
    }

    #[test]
    fn system_runner_stdin_forwarding() {
        let runner = SystemRunner;
        let out = runner
            .run("cat", &[], None, Some("piped input"))
            .expect("cat runs");
        assert_eq!(out.stdout_str(), "piped input");
    }

    #[test]
    fn mock_run_timeout_completed_uses_run_response_when_no_timeout_script() {
        let mock = MockRunner::new();
        mock.expect("claude", None, MockRunner::ok("stream"));
        let res = mock
            .run_timeout("claude", &["-p"], None, None, Duration::from_secs(1))
            .unwrap();
        match res {
            RunResult::Completed(o) => assert_eq!(o.stdout_str(), "stream"),
            RunResult::TimedOut => panic!("expected Completed"),
        }
    }

    #[test]
    fn mock_run_timeout_honors_scripted_timeout_response() {
        let mock = MockRunner::new();
        mock.expect_timeout(
            "claude",
            Some("-p"),
            MockRunner::ok("unused"),
            Ok(RunResult::TimedOut),
        );
        let res = mock
            .run_timeout("claude", &["-p"], None, None, Duration::from_secs(1))
            .unwrap();
        assert!(matches!(res, RunResult::TimedOut));
    }

    #[test]
    fn mock_run_timeout_unscripted_returns_not_found() {
        let mock = MockRunner::new();
        let err = mock
            .run_timeout("nope", &[], None, None, Duration::from_secs(1))
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    struct BareRunner {
        inner: MockRunner,
    }
    impl CommandRunner for BareRunner {
        fn run(
            &self,
            program: &str,
            args: &[&str],
            cwd: Option<&Path>,
            stdin: Option<&str>,
        ) -> io::Result<CommandOutput> {
            self.inner.run(program, args, cwd, stdin)
        }
        // Intentionally does NOT override run_timeout — exercises default body.
    }

    #[test]
    fn default_run_timeout_delegates_to_run() {
        let inner = MockRunner::new();
        inner.expect("git", Some("status"), MockRunner::ok("clean"));
        let bare = BareRunner { inner };
        let res = bare
            .run_timeout("git", &["status"], None, None, Duration::from_secs(1))
            .unwrap();
        match res {
            RunResult::Completed(o) => assert_eq!(o.stdout_str(), "clean"),
            RunResult::TimedOut => panic!("default impl must not time out"),
        }
    }

    #[test]
    fn system_runner_run_timeout_completed_branch() {
        let runner = SystemRunner;
        let res = runner
            .run_timeout("sh", &["-c", "echo ok"], None, None, Duration::from_secs(5))
            .unwrap();
        match res {
            RunResult::Completed(o) => assert_eq!(o.stdout_str().trim(), "ok"),
            RunResult::TimedOut => panic!("expected Completed"),
        }
    }

    #[test]
    fn system_runner_run_timeout_times_out_slow_process() {
        let runner = SystemRunner;
        let res = runner
            .run_timeout(
                "sh",
                &["-c", "sleep 10"],
                None,
                None,
                Duration::from_millis(100),
            )
            .unwrap();
        assert!(matches!(res, RunResult::TimedOut));
    }
}
