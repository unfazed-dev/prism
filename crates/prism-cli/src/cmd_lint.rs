use std::env;

use prism_core::icm::{validate_icm, IcmSettings, Scope};

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir()?;
    let violations = validate_icm(&project_root, &Scope::Project, IcmSettings::default());

    if violations.is_empty() {
        println!("ICM: clean — 0 violations.");
        return Ok(());
    }

    println!("ICM violations: {}", violations.len());
    for v in &violations {
        let loc = match (&v.file, v.line) {
            (Some(f), Some(ln)) => format!("{}:{}", f.display(), ln),
            (Some(f), None) => f.display().to_string(),
            _ => "<project>".to_string(),
        };
        println!("  [{}] {} — {}", v.rule.id(), loc, v.message);
    }
    // Non-zero exit so CI / hooks can branch on outcome.
    std::process::exit(1);
}
