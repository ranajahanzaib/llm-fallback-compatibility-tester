use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use llm_fallback_compatibility_tester::{Run, execute, load_contract, write_artifacts};
use std::{collections::BTreeMap, fs, path::PathBuf, process::ExitCode};
#[derive(Parser)]
#[command(
    name = "llm-fallback-compatibility-tester",
    version,
    about = "Test whether AI provider fallbacks preserve application behavior"
)]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    no_color: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    Validate {
        #[arg(short, long, default_value = "llm-fallback-compatibility-tester.yaml")]
        config: PathBuf,
    },
    Run {
        #[arg(short, long, default_value = "llm-fallback-compatibility-tester.yaml")]
        config: PathBuf,
        #[arg(short, long, default_value = "runs/latest")]
        output: PathBuf,
        #[arg(long)]
        include_content: bool,
        #[arg(long)]
        scenario: Option<String>,
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=20))]
        repeat: u8,
    },
    Compare {
        #[arg(short, long, default_value = "llm-fallback-compatibility-tester.yaml")]
        config: PathBuf,
        #[arg(short, long, default_value = "runs/compare")]
        output: PathBuf,
        #[arg(long, value_delimiter = ',')]
        providers: Vec<String>,
    },
    Report {
        #[arg(short, long, default_value = "runs/latest/run.json")]
        input: PathBuf,
        #[arg(short, long, default_value = "runs/report")]
        output: PathBuf,
    },
    Doctor,
    Adapters {
        #[command(subcommand)]
        command: AdapterCommand,
    },
}
#[derive(Subcommand)]
enum AdapterCommand {
    List,
}
fn main() -> ExitCode {
    match run() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let json_output = cli.json;
    let _no_color = cli.no_color;
    match cli.command {
        Command::Init { path } => {
            fs::create_dir_all(&path)?;
            let files = [
                (
                    "llm-fallback-compatibility-tester.yaml",
                    include_str!("../examples/support-triage/contract.yaml"),
                ),
                (
                    "README.md",
                    include_str!("../examples/support-triage/README.md"),
                ),
                (
                    "fixtures/primary.json",
                    include_str!("../examples/support-triage/fixtures/primary.json"),
                ),
                (
                    "fixtures/safe-backup.json",
                    include_str!("../examples/support-triage/fixtures/safe-backup.json"),
                ),
                (
                    "fixtures/unsafe-backup.json",
                    include_str!("../examples/support-triage/fixtures/unsafe-backup.json"),
                ),
                (
                    "schemas/response.json",
                    include_str!("../examples/support-triage/schemas/response.json"),
                ),
            ];
            for (dst, _) in files {
                if path.join(dst).exists() {
                    anyhow::bail!(
                        "refusing to overwrite existing file {}",
                        path.join(dst).display()
                    );
                }
            }
            for (dst, contents) in files {
                let dest = path.join(dst);
                fs::create_dir_all(dest.parent().context("destination parent")?)?;
                fs::write(&dest, contents)?;
            }
            println!("Initialized project in {}", path.display());
            Ok(ExitCode::SUCCESS)
        }
        Command::Validate { config } => {
            let c = load_contract(&config)?;
            println!(
                "Valid: {} — {} cases, {} scenarios",
                c.name,
                c.cases.len(),
                c.scenarios.len()
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Run {
            config,
            output,
            include_content,
            scenario,
            repeat,
        } => {
            let mut c = load_contract(&config)?;
            if let Some(scenario_id) = scenario {
                c.scenarios.retain(|candidate| candidate.id == scenario_id);
                if c.scenarios.is_empty() {
                    anyhow::bail!("unknown scenario filter: {scenario_id}");
                }
            }
            let mut result: Option<Run> = None;
            for repetition in 0..repeat {
                let mut execution = execute(&c, include_content)?;
                if repeat > 1 {
                    for item in &mut execution.scenarios {
                        item.id = format!("{} [run {}]", item.id, repetition + 1);
                    }
                }
                if let Some(result) = &mut result {
                    result.scenarios.extend(execution.scenarios);
                } else {
                    result = Some(execution);
                }
            }
            let r = result.expect("validated repeat count is at least one");
            write_artifacts(&r, &output)?;
            if json_output {
                let summary = serde_json::json!({"run_id":r.run_id,"contract":r.contract,"scenarios":r.scenarios.iter().map(|s|serde_json::json!({"id":s.id,"verdict":s.verdict})).collect::<Vec<_>>()});
                println!("{}", serde_json::to_string(&summary)?);
                return Ok(if r.scenarios.iter().all(|s| s.verdict == "passed") {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                });
            }
            if r.scenarios
                .iter()
                .any(|scenario| scenario.id.contains(" [run "))
            {
                let mut totals: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
                for scenario in &r.scenarios {
                    let name = scenario
                        .id
                        .split_once(" [run ")
                        .map(|(name, _)| name)
                        .unwrap_or(&scenario.id)
                        .to_owned();
                    let count = totals.entry(name).or_default();
                    count.0 += 1;
                    match scenario.verdict.as_str() {
                        "passed" => count.1 += 1,
                        "failed" | "error" => count.2 += 1,
                        _ => count.3 += 1,
                    }
                }
                for (name, (runs, passed, failed, inconclusive)) in totals {
                    println!(
                        "{name}: {passed}/{runs} passed ({:.0}% observed sample pass rate; {failed} failed, {inconclusive} inconclusive)",
                        passed as f64 * 100.0 / runs as f64
                    );
                }
            }
            for s in &r.scenarios {
                println!("{}: {}", s.id, s.verdict.to_uppercase());
                for case in &s.cases {
                    println!("  {}: {}", case.case_id, case.verdict.to_uppercase());
                    for a in &case.assertions {
                        if a.verdict != "passed" {
                            println!("    {} [{}]: {}", a.verdict, a.id, a.message)
                        }
                    }
                }
            }
            println!(
                "HTML report: {}",
                output.join("report/index.html").display()
            );
            Ok(if r.scenarios.iter().all(|s| s.verdict == "passed") {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Command::Compare {
            config,
            output,
            providers,
        } => {
            let c = load_contract(&config)?;
            let selected = if providers.is_empty() {
                c.providers.keys().cloned().collect::<Vec<_>>()
            } else {
                providers
            };
            let mut result: Option<Run> = None;
            for provider in selected {
                if !c.providers.contains_key(&provider) {
                    anyhow::bail!("unknown comparison provider: {provider}");
                }
                let mut candidate = c.clone();
                for scenario in &mut candidate.scenarios {
                    scenario.id = format!("{provider}/{}", scenario.id);
                    scenario.primary = provider.clone();
                    scenario.fallbacks.clear();
                    scenario.injections.clear();
                }
                let execution = execute(&candidate, false)?;
                if let Some(result) = &mut result {
                    result.scenarios.extend(execution.scenarios);
                } else {
                    result = Some(execution);
                }
            }
            let r = result.expect("validated provider list is non-empty");
            write_artifacts(&r, &output)?;
            if json_output {
                println!("{}", serde_json::to_string(&r)?);
                return Ok(if r.scenarios.iter().all(|s| s.verdict == "passed") {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                });
            }
            for s in &r.scenarios {
                let p = s
                    .attempts
                    .last()
                    .map(|a| a.provider.as_str())
                    .unwrap_or("none");
                println!("{}\t{}\t{}", s.id, p, s.verdict)
            }
            Ok(if r.scenarios.iter().any(|s| s.verdict != "passed") {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::Report { input, output } => {
            let r: Run = serde_json::from_slice(&fs::read(input)?)?;
            write_artifacts(&r, &output)?;
            println!(
                "HTML report: {}",
                output.join("report/index.html").display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Doctor => {
            let python = std::process::Command::new("python3")
                .arg("--version")
                .output()
                .ok()
                .filter(|result| result.status.success())
                .map(|result| String::from_utf8_lossy(&result.stdout).trim().to_owned())
                .unwrap_or_else(|| "not found".into());
            println!(
                "LLM Fallback Compatibility Tester {}\nMock provider: ready\nOpenAI-compatible adapter: ready (endpoint configured per contract)\nPython: {}\nOllama: optional Python adapter; local server/model not checked here\nMock execution uses no network or credentials",
                env!("CARGO_PKG_VERSION"),
                python
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Adapters {
            command: AdapterCommand::List,
        } => {
            println!(
                "mock\tavailable\tfixtures, usage, simulated latency and cost\nopenai_compatible\tavailable\tnon-streaming chat, JSON Schema, tools, usage, env credentials\nollama\toptional\tPython JSONL subprocess adapter; local models, JSON mode, tools, usage"
            );
            Ok(ExitCode::SUCCESS)
        }
    }
}
