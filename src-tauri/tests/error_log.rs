use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn cli_app_errors_are_written_to_configured_error_log() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let config_path = tempdir.path().join("agenta.local.yaml");
    let context_dir = tempdir.path().join("context");
    let error_log_path = tempdir.path().join("logs").join("error.log");

    std::fs::write(
        &config_path,
        "paths:\n  data_dir: ./data\n  error_log: ./logs/error.log\nproject_context:\n  paths:\n    - ./context\n  manifest: project.yaml\n",
    )?;
    std::fs::create_dir_all(&context_dir)?;

    Command::cargo_bin("agenta")?
        .arg("--config")
        .arg(&config_path)
        .args(["project", "get", "--project", "missing-project"])
        .assert()
        .failure();

    let content = std::fs::read_to_string(error_log_path)?;
    let line = content.lines().next().expect("jsonl line");
    let event: Value = serde_json::from_str(line)?;

    assert_eq!(event["surface"], "cli");
    assert_eq!(event["component"], "command");
    assert_eq!(event["action"], "project.get");
    assert_eq!(event["error_code"], "not_found");
    assert_eq!(event["details"]["entity"], "project");

    Ok(())
}
