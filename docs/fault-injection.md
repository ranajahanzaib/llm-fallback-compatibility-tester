# Fault injection

Each scenario can inject a fault at a named provider. The injected attempt is classified, recorded with its optional HTTP status and retry metadata, then retried or advanced to a configured fallback only when policy allows. The sequence is bounded by `defaults.max_total_attempts` (1–20), per-provider retries, and the optional overall deadline. Supported categories include `rate_limit`, `timeout`, `connection_failure`, `provider_unavailable`, `internal_server_error`, `context_window_exceeded`, `invalid_json`, `authentication_failure`, `permission_denied`, and `unsupported_capability`. Authentication, permission, invalid-request, and unknown errors are non-retryable by default. Retryable categories can be restricted with `defaults.fallback_on`.

An injected `status` is included in the attempt result and drives HTTP-classification rules where applicable (for example, 429 becomes `rate_limited`). Mock latency and retry-after values are simulated without sleeping; real adapter retry backoff does wait for its configured delay.

Mock fault injection never contacts a provider. Use synthetic fixtures and prompts for shared examples. `--include-content` exports prompts and responses for local debugging, so use it only when the data is authorized for inclusion.
