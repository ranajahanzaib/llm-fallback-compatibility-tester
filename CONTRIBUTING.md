# Contributing

Contributions to LLM Fallback Compatibility Tester are welcome. Open an issue to discuss a substantial change, then submit a pull request with a focused description of the behavior it changes.

## Development setup

Install stable Rust with the `rustfmt` and `clippy` components. Python 3.11 or newer is needed for the optional adapter and its tests.

```bash
git clone https://github.com/ranajahanzaib/llm-fallback-compatibility-tester.git
cd llm-fallback-compatibility-tester
cargo build --workspace
```

Install the optional Python adapter when working on Ollama integration:

```bash
python3 -m pip install ./python/fallbackcontract_adapters
```

## Before submitting

Run the Rust checks and deterministic mock demo:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/demo.sh
```

Run the Python adapter tests with the package source on `PYTHONPATH`:

```bash
python3 -m pip install ./python/fallbackcontract_adapters
PYTHONPATH=python/fallbackcontract_adapters/src python3 -m unittest discover -s python/fallbackcontract_adapters/tests -v
```

Changes to contract behavior should include a contract example or regression test. Keep mock tests independent of paid services and installed models. Document behavior and limitations in the user guides when they change.

## Pull requests

Include the problem and intended behavior, relevant validation commands and results, and any compatibility or security impact. Do not include API keys, private prompts, generated run artifacts, or unrelated formatting changes.
