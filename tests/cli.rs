use std::{fs, path::PathBuf, process::Command};

fn temp_dir(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("fallbackcontract-{label}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn help_lists_project_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_llm-fallback-compatibility-tester"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    for command in [
        "init", "validate", "run", "compare", "report", "doctor", "adapters",
    ] {
        assert!(help.contains(command), "missing command {command}");
    }
}

#[test]
fn init_creates_working_example_and_refuses_overwrite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let destination = temp_dir("init");
    let binary = env!("CARGO_BIN_EXE_llm-fallback-compatibility-tester");
    assert!(
        Command::new(binary)
            .arg("init")
            .arg(&destination)
            .status()
            .unwrap()
            .success()
    );
    let config = destination.join("fallbackcontract.yaml");
    assert!(
        Command::new(binary)
            .args(["validate", "--config"])
            .arg(&config)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        !Command::new(binary)
            .arg("init")
            .arg(&destination)
            .status()
            .unwrap()
            .success()
    );
    assert!(root.join("examples/support-triage/contract.yaml").is_file());
    fs::remove_dir_all(destination).unwrap();
}

#[test]
fn run_generates_machine_and_html_reports_with_contract_exit_code() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = temp_dir("run");
    let config = root.join("examples/support-triage/contract.yaml");
    let result = Command::new(env!("CARGO_BIN_EXE_llm-fallback-compatibility-tester"))
        .args(["run", "--config"])
        .arg(config)
        .args(["--output"])
        .arg(&output)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    let run: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("run.json")).unwrap()).unwrap();
    assert_eq!(run["schema_version"], 1);
    assert!(output.join("summary.json").is_file());
    assert!(output.join("junit.xml").is_file());
    let html = fs::read_to_string(output.join("report/index.html")).unwrap();
    assert!(html.contains("Compatibility matrix"));
    assert!(html.contains("no-auto-refund"));
    fs::remove_dir_all(output).unwrap();
}

#[test]
fn compare_runs_selected_providers_directly() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = temp_dir("compare");
    let result = Command::new(env!("CARGO_BIN_EXE_llm-fallback-compatibility-tester"))
        .args(["compare", "--config"])
        .arg(root.join("examples/support-triage/contract.yaml"))
        .args(["--providers", "primary,safe_backup"])
        .args(["--output"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success());
    let run: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("run.json")).unwrap()).unwrap();
    assert_eq!(run["scenarios"].as_array().unwrap().len(), 4);
    assert!(
        run["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .all(|scenario| scenario["attempts"][0]["outcome"] == "responded")
    );
    fs::remove_dir_all(output).unwrap();
}
