# Contracts

Contract format version 1 is YAML or JSON. `version`, `name`, `providers`, `cases`, and `scenarios` are required. `schemas/contract.schema.json` publishes the top-level format; the CLI additionally checks provider and case references. Paths to fixtures and case schemas resolve relative to the contract file.

A case contains messages, an optional JSON Schema path, and deterministic assertions evaluated against the normalized response's `content` member. Assertions have a unique descriptive `id`, `type`, and optional severity (`critical` by default). Supported types are `json_equals`/`equals`, `not_equals`, `one_of`, `none_of`, `exists`/`required`, `absent`/`forbidden`, `number_range`, `less_than`, `greater_than`, `string_equals`, `contains`, `not_contains`, `array_length`, `matches_regex`, `conditional_equals`, `manual_review`, `tool_called`, and `tool_not_called`. Unsupported types are inconclusive.

The sample under `examples/support-triage` demonstrates an explicit expected decision and a schema. Manual review is not inferred from prose: encode it as a required field/value or mark the check inconclusive.

Cases can declare `requires_capabilities`, using `structured_json`, `json_schema`, `tool_calling`, `usage_reporting`, `token_counting`, or `streaming`. Provider `capabilities` are optional user-declared booleans, for example `capabilities: {tool_calling: true, streaming: false}`. The runner does not infer capabilities from provider names or model IDs. A required capability declared `false` creates a critical failed assertion; a missing declaration creates a critical inconclusive assertion. Declaring a capability `true` records the explicit claim and does not guarantee identical behavior across model versions.
