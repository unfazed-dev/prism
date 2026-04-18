use std::env;

use prism_core::hooks::protocol::{read_input, write_output, HookContext, HookOutput};
use prism_core::hooks::{post_tool_use, session_start};

pub fn run(event: &str) -> anyhow::Result<()> {
    let input = read_input()?;
    let cwd = env::current_dir()?;
    let session_id = input.session_id.clone().unwrap_or_default();
    let ctx = HookContext::from_cwd(&cwd, &session_id)
        .unwrap_or_else(|| HookContext::new(cwd.clone(), &session_id));

    let output = match event {
        "session-start" => session_start::run(&ctx).unwrap_or_else(|_| HookOutput::allow(None)),
        "post-tool-use" => {
            post_tool_use::run(&input, &ctx).unwrap_or_else(|_| HookOutput::allow(None))
        }
        _ => HookOutput::allow(None),
    };
    write_output(&output)?;
    Ok(())
}
