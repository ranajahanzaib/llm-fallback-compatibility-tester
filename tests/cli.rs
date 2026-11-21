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
    let config = destination.join("llm-fallback-compatibility-tester.yaml");
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

#[test]
fn doctor_reports_environment_readiness() {
    let output = Command::new(env!("CARGO_BIN_EXE_llm-fallback-compatibility-tester"))
        .arg("doctor")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Mock provider: ready"));
    assert!(text.contains("OpenAI-compatible adapter: ready"));
    assert!(text.contains("Python:"));
}

#[test]
fn adapters_list_displays_supported_adapters() {
    let output = Command::new(env!("CARGO_BIN_EXE_llm-fallback-compatibility-tester"))
        .args(["adapters", "list"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("mock\tavailable"));
    assert!(text.contains("openai_compatible\tavailable"));
    assert!(text.contains("ollama\toptional"));
}

#[test]
fn run_scenario_filter_and_report_generation() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = temp_dir("scenario-filter");
    let report_out = temp_dir("scenario-report");
    let config = root.join("examples/support-triage/contract.yaml");
    let result = Command::new(env!("CARGO_BIN_EXE_llm-fallback-compatibility-tester"))
        .args(["run", "--config"])
        .arg(&config)
        .args(["--scenario", "safe-fallback"])
        .args(["--output"])
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0));

    let run_data: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("run.json")).unwrap()).unwrap();
    assert_eq!(run_data["scenarios"].as_array().unwrap().len(), 1);
    assert_eq!(run_data["scenarios"][0]["verdict"], "passed");

    let report_result = Command::new(env!("CARGO_BIN_EXE_llm-fallback-compatibility-tester"))
        .args(["report", "--input"])
        .arg(output.join("run.json"))
        .args(["--output"])
        .arg(&report_out)
        .output()
        .unwrap();
    assert!(report_result.status.success());
    assert!(report_out.join("report/index.html").is_file());

    fs::remove_dir_all(output).unwrap();
    fs::remove_dir_all(report_out).unwrap();
}

#[test]
fn run_with_repetition_reports_sample_pass_rate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = temp_dir("repeat");
    let config = root.join("examples/support-triage/contract.yaml");
    let result = Command::new(env!("CARGO_BIN_EXE_llm-fallback-compatibility-tester"))
        .args(["run", "--config"])
        .arg(&config)
        .args(["--repeat", "2"])
        .args(["--output"])
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("observed sample pass rate"));
    fs::remove_dir_all(output).unwrap();
}
