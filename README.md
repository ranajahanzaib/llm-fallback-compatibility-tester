# LLM Fallback Compatibility Tester

[![CI](https://github.com/ranajahanzaib/llm-fallback-compatibility-tester/actions/workflows/ci.yml/badge.svg)](https://github.com/ranajahanzaib/llm-fallback-compatibility-tester/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**Test whether your AI application still behaves correctly when its model provider fails.**

A fallback can return HTTP 200 and valid JSON while violating a business rule. LLM Fallback Compatibility Tester runs declarative scenarios, injects provider failures, evaluates the response that handled the request, and generates CI-friendly results and a local HTML report.

It tests fallback behavior; it does not route production traffic or execute proposed business tools.

All bundled prompts, customer details, and provider responses are synthetic examples.

## Quick start

Stable Rust and Cargo are required. Mock scenario execution needs no API key, model, database, or network access.

```bash
git clone https://github.com/ranajahanzaib/llm-fallback-compatibility-tester.git
cd llm-fallback-compatibility-tester
cargo install --path .
llm-fallback-compatibility-tester init /tmp/fallback-demo
llm-fallback-compatibility-tester validate --config /tmp/fallback-demo/llm-fallback-compatibility-tester.yaml
llm-fallback-compatibility-tester run --config /tmp/fallback-demo/llm-fallback-compatibility-tester.yaml --scenario safe-fallback --output /tmp/fallback-demo/runs/demo
```

The command above runs the passing safe scenario. The sample also includes an intentionally unsafe fallback, which fails its contract. To run both scenarios and handle that expected failure:

```bash
./scripts/demo.sh
```

Open `/tmp/fallback-demo/runs/demo/report/index.html` in a browser to inspect the provider timeline, compatibility matrix, and assertion results. The demo script writes its report to `runs/demo/report/index.html` in the repository.

## What gets checked

Contracts are YAML or JSON files validated against the published [version 1 schema](schemas/contract.schema.json). They describe provider candidates, test cases, assertions, failure injections, and optional latency or cost budgets.

The engine supports JSON Schema validation; JSON Pointer equality and inequality; allowed and forbidden values; required and forbidden fields; numeric bounds; case-sensitive or insensitive string equality; string or array containment; array length; regular-expression matching; conditional field rules; manual-review outcomes; and tool-call presence, absence, and argument-schema checks. Unsupported assertions and unknown required capabilities are reported as inconclusive. Critical failures fail the scenario.

Tool calls are evaluated as proposals. LLM Fallback Compatibility Tester never invokes the named tools or performs business actions.

## Providers

| Provider            | Use                                                 | Requirements                                                                                                                                |
| ------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `mock`              | Deterministic fixtures for local development and CI | None; no network access                                                                                                                     |
| `openai_compatible` | Non-streaming chat-completions endpoint             | Explicit `base_url` and `model`; remote endpoints must use HTTPS; credentials are read from the environment variable named by `api_key_env` |
| `ollama`            | Optional local model through the Python adapter     | Python 3.11+, installed adapter package, running loopback Ollama server, and an already available model                                     |

Install the Ollama adapter from the repository root with `python3 -m pip install ./python/fallbackcontract_adapters`. LLM Fallback Compatibility Tester does not download models. See [provider adapters](docs/provider-adapters.md) for configuration and behavior.

Providers may declare known capabilities in the contract. The tool does not infer model capabilities from a provider or model name. A required capability declared unsupported fails; one with no declaration is inconclusive.

## Reports and privacy

Each run writes `run.json`, `summary.json`, `junit.xml`, and `report/index.html` under the selected output directory. The result record includes attempts, transitions, contract verdicts, timing, and usage when available. The HTML report is self-contained and works offline.

Prompt content, response content, proposed tool arguments, and assertion actual values are redacted or omitted by default. `--include-content` includes prompts and normalized responses in local artifacts; use it only with data approved for that purpose. API credentials are read from environment variables and are not written to reports. See the [security guide](docs/security.md).

## Commands

```text
llm-fallback-compatibility-tester init [PATH]
llm-fallback-compatibility-tester validate --config contract.yaml
llm-fallback-compatibility-tester run --config contract.yaml --output runs/latest
llm-fallback-compatibility-tester compare --config contract.yaml --providers primary,backup
llm-fallback-compatibility-tester report --input runs/latest/run.json --output runs/report
llm-fallback-compatibility-tester doctor
llm-fallback-compatibility-tester adapters list
```

`run` accepts `--scenario ID` and bounded repetition with `--repeat 1..20`. Repeated outcomes remain individually inspectable and summaries use the label “observed sample pass rate.” `compare` executes each selected provider against the configured cases without fallback transitions.

Exit status is 0 when every scenario passes, 1 when a run contains a failed or inconclusive result, and 2 for invalid configuration or execution errors.

## Continuous integration

GitHub Actions runs formatting, Clippy, Rust and Python tests, Python compilation, and the mock demo. A project can use the same commands:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m pip install ./python/fallbackcontract_adapters
PYTHONPATH=python/fallbackcontract_adapters/src python3 -m unittest discover -s python/fallbackcontract_adapters/tests -v
./scripts/demo.sh
```

Upload the demo output directory as a CI artifact to retain its JSON, JUnit, and HTML reports.

## Where it fits

Production gateways route requests and apply operational fallback policies. LLM Fallback Compatibility Tester tests whether a configured fallback preserves explicit application requirements when that transition occurs. Broader evaluation suites can measure other quality dimensions; this project focuses on reproducible provider-transition contracts. It complements routing and monitoring systems rather than replacing them.

## Limitations

The live adapters are non-streaming. Provider capability discovery is not implemented, and user-declared capabilities are not independently verified. Cost checks require cost data; live API cost is usually unavailable and is reported as inconclusive. Assertions evaluate explicit deterministic rules and do not claim general natural-language understanding. Repeated runs are observations of the configured cases, not evidence that all future requests will be compatible.

## Documentation

- [Contract format and assertions](docs/contracts.md)
- [Architecture and gateway integration](docs/architecture.md)
- [Fault injection](docs/fault-injection.md)
- [Provider adapters and local models](docs/provider-adapters.md)
- [Security and data handling](docs/security.md)
- [Support-triage example](examples/support-triage/README.md)
- [Contributing](CONTRIBUTING.md)

## License

LLM Fallback Compatibility Tester is distributed under the [MIT License](LICENSE).
