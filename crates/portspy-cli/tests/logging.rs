use std::process::Command;

#[test]
fn help_runs_when_file_logging_is_unavailable() -> Result<(), Box<dyn std::error::Error>> {
    let existing_file = std::env::current_exe()?;
    let output = Command::new(env!("CARGO_BIN_EXE_portspy"))
        .args(["--help"])
        .env("LOCALAPPDATA", &existing_file)
        .env("XDG_DATA_HOME", &existing_file)
        .env("PORTSPY_LOG", "warn")
        .output()?;

    assert!(output.status.success(), "{output:?}");
    assert!(std::str::from_utf8(&output.stdout)?.contains("Usage: portspy"));
    assert!(std::str::from_utf8(&output.stderr)?.contains("Could not open log file"));
    Ok(())
}
