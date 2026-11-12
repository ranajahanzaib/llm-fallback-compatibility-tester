# LLM Fallback Compatibility Tester Python adapters

This package provides the optional JSON Lines adapter process used by the Rust CLI to call a local Ollama server. The adapter uses HTTPX, sends non-streaming chat requests, and normalizes response content, proposed tool calls, and token usage.

## Install

From the repository root:

```bash
python3 -m pip install ./python/fallbackcontract_adapters
```

Install it in the same Python environment that runs `llm-fallback-compatibility-tester`. In a contract, configure a provider with `kind: ollama`, an existing local model name, and an optional loopback `base_url`. The CLI launches the adapter process when that provider is selected.

The adapter never downloads models. Install and start Ollama separately, then verify that the configured model is already available. Ollama providers are restricted to loopback endpoints. No API key is required.

## Tests

```bash
python3 -m pip install ./python/fallbackcontract_adapters
PYTHONPATH=python/fallbackcontract_adapters/src python3 -m unittest discover -s python/fallbackcontract_adapters/tests -v
```

The HTTP adapter test uses a local mock server and does not require Ollama or an external service.
