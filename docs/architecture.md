# Architecture

The Rust library loads and validates a declarative contract, selects a configured provider chain, applies scenario fault injections, normalizes provider responses, evaluates JSON Schema and explicit assertions, and serializes versioned run results. The CLI invokes these operations, and the self-contained HTML report is generated from the resulting run record.

The runner executes mock, OpenAI-compatible HTTP, and optional Ollama providers through a normalized response boundary. It records attempts and transitions as run events, then writes versioned JSON, summary JSON, JUnit XML, and a self-contained HTML report from the same result record. Proposed tool calls are checked against tool schemas and are never executed.

Gateway integration is optional and file-based: export representative normalized responses as mock fixtures, then use a contract to evaluate behavior. LLM Fallback Compatibility Tester does not add a gateway or routing service. Reports are self-contained static HTML and need no JavaScript runtime. Provider capability discovery, streaming, and gateway event ingestion are not supported; capability declarations in contracts are supplied explicitly by the user.
