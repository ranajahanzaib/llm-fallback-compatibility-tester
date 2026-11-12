use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;
pub mod provider;
pub mod python_bridge;

pub const RESULT_SCHEMA_VERSION: u32 = 1;
const MAX_CONFIG_BYTES: u64 = 8 * 1024 * 1024;
const MAX_FIXTURE_BYTES: u64 = 16 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub providers: BTreeMap<String, ProviderConfig>,
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub latency: Option<LatencyBudget>,
    #[serde(default)]
    pub cost: Option<CostBudget>,
    #[serde(default)]
    pub cases: Vec<Case>,
    #[serde(default)]
    pub scenarios: Vec<Scenario>,
    #[serde(skip)]
    pub source_digest: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBudget {
    pub max_end_to_end_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBudget {
    pub max_total_usd: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: String,
    #[serde(default)]
    pub fixture: Option<PathBuf>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    /// User-declared model capabilities. Missing entries are unknown.
    #[serde(default)]
    pub capabilities: Option<BTreeMap<String, bool>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    #[serde(default = "timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub overall_deadline_ms: Option<u64>,
    #[serde(default = "attempts")]
    pub max_total_attempts: usize,
    #[serde(default)]
    pub max_retries_per_provider: usize,
    #[serde(default)]
    pub retry_backoff_ms: u64,
    #[serde(default = "fallback_categories")]
    pub fallback_on: Vec<String>,
    #[serde(default = "cost")]
    pub max_cost_usd: f64,
}
impl Default for Defaults {
    fn default() -> Self {
        Self {
            timeout_ms: timeout(),
            overall_deadline_ms: None,
            max_total_attempts: attempts(),
            max_retries_per_provider: 0,
            retry_backoff_ms: 0,
            fallback_on: fallback_categories(),
            max_cost_usd: cost(),
        }
    }
}
fn timeout() -> u64 {
    3000
}
fn attempts() -> usize {
    3
}
fn fallback_categories() -> Vec<String> {
    [
        "rate_limited",
        "timeout",
        "connection_failure",
        "provider_unavailable",
        "internal_server_error",
        "context_window_exceeded",
        "invalid_provider_response",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}
fn cost() -> f64 {
    f64::INFINITY
}
fn valid_json_pointer(pointer: &str) -> bool {
    if pointer.is_empty() {
        return true;
    }
    if !pointer.starts_with('/') {
        return false;
    }
    let bytes = pointer.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            if index + 1 >= bytes.len() || !matches!(bytes[index + 1], b'0' | b'1') {
                return false;
            }
            index += 1
        }
        index += 1
    }
    true
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub output_schema: Option<PathBuf>,
    #[serde(default)]
    pub assertions: Vec<Assertion>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    #[serde(default)]
    pub requires_capabilities: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub parameters: Value,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Assertion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub expected: Option<Value>,
    #[serde(default)]
    pub values: Vec<Value>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub forbidden_values: Vec<Value>,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    #[serde(default)]
    pub min_items: Option<usize>,
    #[serde(default)]
    pub max_items: Option<usize>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub if_path: Option<String>,
    #[serde(default)]
    pub if_equals: Option<Value>,
    #[serde(default)]
    pub then_path: Option<String>,
    #[serde(default)]
    pub then_equals: Option<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub primary: String,
    #[serde(default)]
    pub fallbacks: Vec<String>,
    #[serde(default)]
    pub cases: Vec<String>,
    #[serde(default)]
    pub injections: Vec<Injection>,
    #[serde(default)]
    pub repetitions: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Injection {
    pub provider: String,
    pub fault: String,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub retry_after_ms: Option<u64>,
    #[serde(default)]
    pub times: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub schema_version: u32,
    pub run_id: String,
    pub contract: String,
    pub tool_version: String,
    pub config_digest: String,
    pub started_at_unix_ms: u128,
    pub environment: String,
    pub scenarios: Vec<ScenarioResult>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub id: String,
    pub description: String,
    pub verdict: String,
    pub attempts: Vec<Attempt>,
    pub cases: Vec<CaseResult>,
    pub events: Vec<Event>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub attempt_id: String,
    pub provider: String,
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<BTreeMap<String, bool>>,
    pub case_id: String,
    pub outcome: String,
    pub failure: Option<String>,
    pub classification: Option<String>,
    pub http_status: Option<u16>,
    pub retryable: bool,
    pub retry_after_ms: Option<u64>,
    pub failure_stage: Option<String>,
    pub latency_ms: u64,
    pub cost_usd: Option<f64>,
    pub usage: Option<Value>,
    pub response: Option<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub case_id: String,
    pub provider: Option<String>,
    pub input_messages: Option<Vec<Message>>,
    pub verdict: String,
    pub assertions: Vec<AssertionResult>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub id: String,
    pub verdict: String,
    pub severity: String,
    pub expected: Option<Value>,
    pub actual: Option<Value>,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub event_id: String,
    pub run_id: String,
    pub scenario_id: String,
    pub case_id: Option<String>,
    pub attempt_id: Option<String>,
    pub at_ms: u64,
    pub kind: String,
    pub provider: String,
    pub message: String,
}

pub fn load_contract(path: &Path) -> Result<Contract> {
    if fs::metadata(path)?.len() > MAX_CONFIG_BYTES {
        bail!("configuration exceeds the 8 MiB size limit")
    }
    let raw = fs::read_to_string(path)?;
    let document: Value = if path
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        serde_json::from_str(&raw)?
    } else {
        serde_yaml::from_str(&raw)?
    };
    let config_schema: Value =
        serde_json::from_str(include_str!("../schemas/contract.schema.json"))?;
    let validator = jsonschema::validator_for(&config_schema)?;
    if let Some(error) = validator.iter_errors(&document).next() {
        bail!(
            "configuration schema error at {}: {}",
            error.instance_path,
            error
        )
    }
    let mut c: Contract = if path.extension().is_some_and(|x| x == "json") {
        serde_json::from_str(&raw)?
    } else {
        serde_yaml::from_str(&raw)?
    };
    c.source_digest = Some(format!("sha256:{:x}", sha2::Sha256::digest(raw.as_bytes())));
    validate_contract(&c)?;
    let root = path.parent().unwrap_or(Path::new("."));
    for p in c.providers.values_mut() {
        if let Some(f) = &mut p.fixture
            && f.is_relative()
        {
            *f = root.join(&*f)
        }
    }
    for case in &mut c.cases {
        if let Some(f) = &mut case.output_schema
            && f.is_relative()
        {
            *f = root.join(&*f)
        }
    }
    for (id, provider) in &c.providers {
        if let Some(capabilities) = &provider.capabilities {
            for capability in capabilities.keys() {
                if !known_capability(capability) {
                    bail!("providers.{id}.capabilities: unsupported capability '{capability}'")
                }
            }
        }
        if provider.kind == "mock" {
            let fixture = provider.fixture.as_ref().with_context(|| {
                format!("providers.{id}.fixture is required for mock providers")
            })?;
            if !fixture.is_file() {
                bail!(
                    "providers.{id}.fixture: file does not exist: {}",
                    fixture.display()
                )
            }
            if fs::metadata(fixture)?.len() > MAX_FIXTURE_BYTES {
                bail!("providers.{id}.fixture exceeds the 16 MiB limit")
            }
            let values: Value = serde_json::from_slice(
                &fs::read(fixture)
                    .with_context(|| format!("read mock fixture {}", fixture.display()))?,
            )
            .with_context(|| format!("providers.{id}.fixture is not valid JSON"))?;
            if !values.is_object() {
                bail!("providers.{id}.fixture: expected an object keyed by case ID")
            }
            for case in &c.cases {
                if let Some(response) = values.get(&case.id).or_else(|| values.get("default")) {
                    if !response.is_object() {
                        bail!("providers.{id}.fixture.{}: expected an object", case.id)
                    }
                    if response.get("error").is_none()
                        && response.get("response").is_none()
                        && response.get("content").is_none()
                    {
                        bail!(
                            "providers.{id}.fixture.{}: expected content, response, or error",
                            case.id
                        )
                    }
                    if response
                        .get("latency_ms")
                        .is_some_and(|latency| latency.as_u64().is_none())
                    {
                        bail!(
                            "providers.{id}.fixture.{}.latency_ms: expected a non-negative integer",
                            case.id
                        )
                    }
                    if response.get("cost_usd").is_some_and(|cost| {
                        cost.as_f64()
                            .is_none_or(|value| !value.is_finite() || value < 0.0)
                    }) {
                        bail!(
                            "providers.{id}.fixture.{}.cost_usd: expected a finite non-negative number",
                            case.id
                        )
                    }
                } else {
                    bail!(
                        "providers.{id}.fixture: missing fixture entry for case {}",
                        case.id
                    )
                }
            }
        } else if !matches!(provider.kind.as_str(), "openai_compatible" | "ollama") {
            bail!(
                "providers.{id}.kind: unsupported adapter '{}'; supported kinds are mock, openai_compatible, ollama",
                provider.kind
            )
        } else if provider.kind == "openai_compatible" {
            let base = provider.base_url.as_deref().with_context(|| {
                format!("providers.{id}.base_url is required for openai_compatible")
            })?;
            crate::provider::validate_endpoint(base)
                .with_context(|| format!("providers.{id}.base_url is invalid"))?;
            if provider.model.as_deref().is_none_or(str::is_empty) {
                bail!("providers.{id}.model is required for openai_compatible")
            }
        } else if provider.kind == "ollama" {
            if provider.model.as_deref().is_none_or(str::is_empty) {
                bail!("providers.{id}.model is required for ollama")
            }
            if let Some(base) = provider.base_url.as_deref() {
                crate::provider::validate_endpoint(base)
                    .with_context(|| format!("providers.{id}.base_url is invalid"))?;
                let parsed = reqwest::Url::parse(base)?;
                if !matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1")) {
                    bail!("providers.{id}.base_url for ollama must be loopback")
                }
            }
        }
    }
    for case in &c.cases {
        for capability in &case.requires_capabilities {
            if !known_capability(capability) {
                bail!(
                    "cases.{}.requires_capabilities: unsupported capability '{capability}'",
                    case.id
                )
            }
        }
        let message_bytes: usize = case
            .messages
            .iter()
            .map(|message| message.content.len())
            .sum();
        if message_bytes > 1024 * 1024 {
            bail!(
                "cases.{}.messages exceed the 1 MiB input-size limit",
                case.id
            )
        }
        if let Some(path) = &case.output_schema {
            if !path.is_file() {
                bail!(
                    "cases.{}.output_schema: file does not exist: {}",
                    case.id,
                    path.display()
                )
            }
            let schema: Value = serde_json::from_slice(
                &fs::read(path).with_context(|| format!("read schema {}", path.display()))?,
            )?;
            jsonschema::validator_for(&schema)
                .with_context(|| format!("cases.{}.output_schema is invalid", case.id))?;
        }
    }
    Ok(c)
}

fn known_capability(capability: &str) -> bool {
    matches!(
        capability,
        "structured_json"
            | "json_schema"
            | "tool_calling"
            | "usage_reporting"
            | "token_counting"
            | "streaming"
    )
}
pub fn validate_contract(c: &Contract) -> Result<()> {
    if c.version != 1 {
        bail!("version: expected 1")
    };
    if c.name.trim().is_empty() {
        bail!("name: must not be empty")
    };
    if c.providers.is_empty() {
        bail!("providers: at least one required")
    };
    if c.cases.is_empty() {
        bail!("cases: at least one case is required")
    }
    if c.scenarios.is_empty() {
        bail!("scenarios: at least one scenario is required")
    }
    if c.defaults.max_total_attempts == 0 || c.defaults.max_total_attempts > 20 {
        bail!("defaults.max_total_attempts: expected 1..=20")
    };
    if c.defaults.max_retries_per_provider > 10 {
        bail!("defaults.max_retries_per_provider: must be at most 10")
    }
    if c.defaults.retry_backoff_ms > 60_000 {
        bail!("defaults.retry_backoff_ms: maximum is 60000 ms")
    }
    if c.defaults.max_retries_per_provider > 10 {
        bail!("defaults.max_retries_per_provider: must be at most 10")
    }
    let retry_categories = [
        "rate_limited",
        "timeout",
        "connection_failure",
        "provider_unavailable",
        "internal_server_error",
        "context_window_exceeded",
        "invalid_provider_response",
    ];
    if c.defaults
        .fallback_on
        .iter()
        .any(|category| !retry_categories.contains(&category.as_str()))
    {
        bail!("defaults.fallback_on: contains an unsupported failure category")
    }
    if c.defaults.timeout_ms == 0 {
        bail!("defaults.timeout_ms: must be greater than zero")
    }
    if c.defaults.overall_deadline_ms == Some(0) {
        bail!("defaults.overall_deadline_ms: must be greater than zero")
    }
    if c.cost
        .as_ref()
        .is_some_and(|b| !b.max_total_usd.is_finite() || b.max_total_usd < 0.0)
    {
        bail!("cost.max_total_usd: must be a finite non-negative number")
    }
    if c.defaults.max_cost_usd < 0.0
        || (!c.defaults.max_cost_usd.is_finite() && c.defaults.max_cost_usd != f64::INFINITY)
    {
        bail!("defaults.max_cost_usd: must be non-negative")
    }
    if c.latency
        .as_ref()
        .is_some_and(|budget| budget.max_end_to_end_ms == 0)
    {
        bail!("latency.max_end_to_end_ms: must be greater than zero")
    }
    for (id, provider) in &c.providers {
        if let Some(capabilities) = &provider.capabilities {
            for capability in capabilities.keys() {
                if !known_capability(capability) {
                    bail!("providers.{id}.capabilities: unsupported capability '{capability}'")
                }
            }
        }
        if provider
            .timeout_ms
            .is_some_and(|timeout| timeout == 0 || timeout > 300_000)
        {
            bail!("providers.{id}.timeout_ms: expected 1..=300000")
        }
        if provider.temperature.is_some_and(|temperature| {
            !temperature.is_finite() || !(0.0..=2.0).contains(&temperature)
        }) {
            bail!("providers.{id}.temperature: expected a value from 0 to 2")
        }
        if provider
            .top_p
            .is_some_and(|top_p| !top_p.is_finite() || top_p <= 0.0 || top_p > 1.0)
        {
            bail!("providers.{id}.top_p: expected a value in (0, 1]")
        }
        if provider.max_output_tokens == Some(0) {
            bail!("providers.{id}.max_output_tokens: must be greater than zero")
        }
    }
    let ids: std::collections::HashSet<_> = c.cases.iter().map(|x| x.id.as_str()).collect();
    if ids.len() != c.cases.len() {
        bail!("cases: duplicate IDs")
    };
    if c.cases.iter().any(|case| case.id.trim().is_empty()) {
        bail!("cases[].id: must not be empty")
    }
    let scenario_ids: std::collections::HashSet<_> = c
        .scenarios
        .iter()
        .map(|scenario| scenario.id.as_str())
        .collect();
    if scenario_ids.len() != c.scenarios.len() {
        bail!("scenarios: duplicate IDs")
    }
    if c.scenarios
        .iter()
        .any(|scenario| scenario.id.trim().is_empty())
    {
        bail!("scenarios[].id: must not be empty")
    }
    for s in &c.scenarios {
        if !c.providers.contains_key(&s.primary) {
            bail!("scenarios.{}.primary: unknown provider {}", s.id, s.primary)
        }
        for f in &s.fallbacks {
            if !c.providers.contains_key(f) {
                bail!("scenarios.{}.fallbacks: unknown provider {}", s.id, f)
            }
        }
        for id in &s.cases {
            if !ids.contains(id.as_str()) {
                bail!("scenarios.{}.cases: unknown case {}", s.id, id)
            }
        }
        for injection in &s.injections {
            if !c.providers.contains_key(&injection.provider) {
                bail!(
                    "scenarios.{}.injections: unknown provider {}",
                    s.id,
                    injection.provider
                )
            }
            if !matches!(
                injection.fault.as_str(),
                "rate_limit"
                    | "timeout"
                    | "connection_failure"
                    | "provider_unavailable"
                    | "internal_server_error"
                    | "context_window_exceeded"
                    | "invalid_request"
                    | "authentication_failure"
                    | "permission_denied"
                    | "unsupported_capability"
                    | "invalid_provider_response"
                    | "invalid_json"
                    | "stream_failure"
            ) {
                bail!(
                    "scenarios.{}.injections: unsupported fault {}",
                    s.id,
                    injection.fault
                )
            }
            if injection
                .status
                .is_some_and(|status| !(100..=599).contains(&status))
            {
                bail!(
                    "scenarios.{}.injections.status: expected an HTTP status from 100 to 599",
                    s.id
                )
            }
            if injection.times.is_some_and(|times| times > 20) {
                bail!("scenarios.{}.injections.times: maximum is 20", s.id)
            }
            if injection.times == Some(0) {
                bail!("scenarios.{}.injections.times must be at least one", s.id)
            }
        }
    }
    for case in &c.cases {
        let mut required_capabilities = std::collections::HashSet::new();
        for capability in &case.requires_capabilities {
            if !known_capability(capability) {
                bail!(
                    "cases.{}.requires_capabilities: unsupported capability '{capability}'",
                    case.id
                )
            }
            if !required_capabilities.insert(capability) {
                bail!(
                    "cases.{}.requires_capabilities: duplicate capability '{capability}'",
                    case.id
                )
            }
        }
        let mut assertion_ids = std::collections::HashSet::new();
        for assertion in &case.assertions {
            if assertion.id.trim().is_empty() {
                bail!("cases.{}.assertions[].id: must not be empty", case.id)
            }
            if !assertion_ids.insert(&assertion.id) {
                bail!(
                    "cases.{}.assertions: duplicate id {}",
                    case.id,
                    assertion.id
                )
            }
            if let Some(pointer) = &assertion.path
                && !pointer.is_empty()
                && !pointer.starts_with('/')
            {
                bail!(
                    "cases.{}.assertions.{}.path: must be a JSON Pointer",
                    case.id,
                    assertion.id
                )
            }
            if assertion
                .path
                .as_deref()
                .into_iter()
                .chain(
                    [assertion.if_path.as_deref(), assertion.then_path.as_deref()]
                        .into_iter()
                        .flatten(),
                )
                .any(|pointer| !valid_json_pointer(pointer))
            {
                bail!(
                    "cases.{}.assertions.{}: invalid JSON Pointer",
                    case.id,
                    assertion.id
                )
            }
            if !matches!(
                assertion.severity.as_str(),
                "" | "critical" | "warning" | "info"
            ) {
                bail!(
                    "cases.{}.assertions.{}.severity: expected critical, warning, or info",
                    case.id,
                    assertion.id
                )
            }
            if assertion.kind == "number_range"
                && (assertion.min.is_none() && assertion.max.is_none()
                    || assertion
                        .min
                        .zip(assertion.max)
                        .is_some_and(|(min, max)| min > max))
            {
                bail!(
                    "cases.{}.assertions.{}: number_range requires ordered min/max bounds",
                    case.id,
                    assertion.id
                )
            }
            if matches!(assertion.kind.as_str(), "tool_called" | "tool_not_called")
                && assertion.tool.as_deref().is_none_or(str::is_empty)
            {
                bail!(
                    "cases.{}.assertions.{}.tool is required",
                    case.id,
                    assertion.id
                )
            }
            if matches!(
                assertion.kind.as_str(),
                "json_equals"
                    | "equals"
                    | "not_equals"
                    | "one_of"
                    | "none_of"
                    | "required"
                    | "exists"
                    | "forbidden"
                    | "absent"
                    | "number_range"
                    | "less_than"
                    | "greater_than"
                    | "string_equals"
                    | "contains"
                    | "not_contains"
                    | "array_length"
                    | "matches_regex"
            ) && assertion.path.is_none()
            {
                bail!(
                    "cases.{}.assertions.{}.path is required",
                    case.id,
                    assertion.id
                )
            }
            if assertion.kind == "one_of" && assertion.values.is_empty() {
                bail!(
                    "cases.{}.assertions.{}.values must not be empty",
                    case.id,
                    assertion.id
                )
            }
            if assertion.kind == "none_of" && assertion.forbidden_values.is_empty() {
                bail!(
                    "cases.{}.assertions.{}.forbidden_values must not be empty",
                    case.id,
                    assertion.id
                )
            }
            if assertion
                .min_items
                .zip(assertion.max_items)
                .is_some_and(|(min, max)| min > max)
            {
                bail!(
                    "cases.{}.assertions.{}: min_items cannot exceed max_items",
                    case.id,
                    assertion.id
                )
            }
            match assertion.kind.as_str() {
                "json_equals" | "equals" | "not_equals" | "one_of" | "none_of" | "exists"
                | "required" | "absent" | "forbidden" | "number_range" | "less_than"
                | "greater_than" | "string_equals" | "contains" | "not_contains"
                | "array_length" | "matches_regex" | "manual_review" | "tool_called"
                | "tool_not_called" | "conditional_equals" => {}
                other => bail!(
                    "cases.{}.assertions.{}.type: unsupported operator {}",
                    case.id,
                    assertion.id,
                    other
                ),
            }
            if assertion.kind == "matches_regex" {
                let pattern = assertion.pattern.as_deref().context(format!(
                    "cases.{}.assertions.{}.pattern is required",
                    case.id, assertion.id
                ))?;
                regex::Regex::new(pattern).with_context(|| {
                    format!(
                        "cases.{}.assertions.{}.pattern is invalid",
                        case.id, assertion.id
                    )
                })?;
            }
            if assertion.kind == "conditional_equals"
                && (assertion.if_path.is_none()
                    || assertion.if_equals.is_none()
                    || assertion.then_path.is_none()
                    || assertion.then_equals.is_none())
            {
                bail!(
                    "cases.{}.assertions.{}: conditional_equals requires if_path, then_path, and then_equals",
                    case.id,
                    assertion.id
                )
            }
        }
        let mut tool_names = std::collections::HashSet::new();
        for tool in &case.tools {
            if !tool_names.insert(&tool.name) {
                bail!("cases.{}.tools: duplicate tool {}", case.id, tool.name)
            }
            jsonschema::validator_for(&tool.parameters).with_context(|| {
                format!(
                    "cases.{}.tools.{}.parameters is not a valid JSON Schema",
                    case.id, tool.name
                )
            })?;
        }
    }
    Ok(())
}

pub fn evaluate_response(
    case: &Case,
    response: &Value,
    schema: Option<&Value>,
) -> Vec<AssertionResult> {
    let body = response
        .get("content")
        .cloned()
        .unwrap_or_else(|| response.clone());
    let mut out = Vec::new();
    if let Some(schema) = schema {
        let valid = jsonschema::validator_for(schema)
            .map(|v| v.validate(&body).is_ok())
            .unwrap_or(false);
        out.push(AssertionResult {
            id: "output-schema".into(),
            verdict: if valid { "passed" } else { "failed" }.into(),
            severity: "critical".into(),
            expected: None,
            actual: Some(body.clone()),
            message: if valid {
                "Response matches JSON Schema"
            } else {
                "Response does not match JSON Schema or schema is invalid"
            }
            .into(),
        })
    }
    for a in &case.assertions {
        out.push(eval_assertion(a, &body, response))
    }
    let tool_schemas: std::collections::HashMap<_, _> = case
        .tools
        .iter()
        .map(|tool| (tool.name.as_str(), &tool.parameters))
        .collect();
    if let Some(calls) = response.get("tool_calls").and_then(Value::as_array) {
        for (index, call) in calls.iter().enumerate() {
            let name = call
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<missing-name>");
            let arguments = call.get("arguments").cloned().unwrap_or(Value::Null);
            let valid = tool_schemas.get(name).is_some_and(|schema| {
                jsonschema::validator_for(schema)
                    .is_ok_and(|validator| validator.validate(&arguments).is_ok())
            });
            out.push(AssertionResult {
                id: format!("tool-arguments-{name}-{index}"),
                verdict: if valid { "passed" } else { "failed" }.into(),
                severity: "critical".into(),
                expected: tool_schemas.get(name).map(|s| (*s).clone()),
                actual: Some(arguments),
                message: if valid {
                    format!("Arguments for tool '{name}' match its schema")
                } else if tool_schemas.contains_key(name) {
                    format!("Arguments for tool '{name}' do not match its schema")
                } else {
                    format!("Tool '{name}' is not declared by this case")
                },
            });
        }
    }
    out
}
fn evaluate_capability_requirements(
    case: &Case,
    declared: Option<&BTreeMap<String, bool>>,
) -> Vec<AssertionResult> {
    case.requires_capabilities
        .iter()
        .map(|capability| {
            let supported = declared.and_then(|items| items.get(capability));
            let (verdict, message) = match supported {
                Some(true) => ("passed", "Provider declares this capability"),
                Some(false) => ("failed", "Provider declares this capability unsupported"),
                None => (
                    "inconclusive",
                    "Provider capability is unknown; declare it explicitly to evaluate this requirement",
                ),
            };
            AssertionResult {
                id: format!("capability-{capability}"),
                verdict: verdict.into(),
                severity: "critical".into(),
                expected: Some(Value::Bool(true)),
                actual: supported.copied().map(Value::Bool),
                message: message.into(),
            }
        })
        .collect()
}
fn eval_assertion(a: &Assertion, body: &Value, response: &Value) -> AssertionResult {
    let actual = a.path.as_deref().and_then(|p| body.pointer(p));
    let calls = response
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (ok, msg, got) = match a.kind.as_str() {
        "json_equals" | "equals" => (
            actual == a.expected.as_ref(),
            "Value equality",
            actual.cloned(),
        ),
        "not_equals" => (
            actual != a.expected.as_ref(),
            "Value inequality",
            actual.cloned(),
        ),
        "one_of" => (
            actual.is_some_and(|v| a.values.contains(v)),
            "Allowed value",
            actual.cloned(),
        ),
        "none_of" => (
            actual.is_some_and(|v| !a.forbidden_values.contains(v)),
            "Value is not forbidden",
            actual.cloned(),
        ),
        "exists" | "required" => (actual.is_some(), "Required field", actual.cloned()),
        "absent" | "forbidden" => (actual.is_none(), "Forbidden field", actual.cloned()),
        "number_range" => {
            let n = actual.and_then(Value::as_f64);
            (
                n.is_some_and(|n| a.min.is_none_or(|m| n >= m) && a.max.is_none_or(|m| n <= m)),
                "Number is within range",
                actual.cloned(),
            )
        }
        "less_than" | "greater_than" => {
            let n = actual.and_then(Value::as_f64);
            let bound = a
                .expected
                .as_ref()
                .and_then(Value::as_f64)
                .or(a.max)
                .or(a.min);
            let ok = n
                .zip(bound)
                .is_some_and(|(n, b)| if a.kind == "less_than" { n < b } else { n > b });
            (
                ok,
                if a.kind == "less_than" {
                    "Number is below limit"
                } else {
                    "Number is above limit"
                },
                actual.cloned(),
            )
        }
        "string_equals" => {
            let left = actual.and_then(Value::as_str);
            let right = a.expected.as_ref().and_then(Value::as_str);
            let sensitive = a.case_sensitive.unwrap_or(true);
            let ok = left.zip(right).is_some_and(|(l, r)| {
                if sensitive {
                    l == r
                } else {
                    l.eq_ignore_ascii_case(r)
                }
            });
            (ok, "String equality", actual.cloned())
        }
        "contains" | "not_contains" => {
            let has = actual.is_some_and(|v| match v {
                Value::Array(xs) => a.expected.as_ref().is_some_and(|e| xs.contains(e)),
                Value::String(s) => a
                    .expected
                    .as_ref()
                    .and_then(Value::as_str)
                    .is_some_and(|x| s.contains(x)),
                _ => false,
            });
            (
                if a.kind == "contains" { has } else { !has },
                "Container membership",
                actual.cloned(),
            )
        }
        "array_length" => {
            let len = actual.and_then(Value::as_array).map(Vec::len);
            (
                len.is_some_and(|n| {
                    a.min_items.is_none_or(|m| n >= m) && a.max_items.is_none_or(|m| n <= m)
                }),
                "Array length is within bounds",
                actual.cloned(),
            )
        }
        "matches_regex" => {
            let matched = actual
                .and_then(Value::as_str)
                .zip(a.pattern.as_deref())
                .is_some_and(|(s, p)| regex::Regex::new(p).is_ok_and(|re| re.is_match(s)));
            (
                matched,
                "String matches safe regular expression",
                actual.cloned(),
            )
        }
        "conditional_equals" => {
            let condition = a.if_path.as_deref().and_then(|p| body.pointer(p));
            let applies = condition == a.if_equals.as_ref();
            let target = a.then_path.as_deref().and_then(|p| body.pointer(p));
            (
                !applies || target == a.then_equals.as_ref(),
                "Conditional policy",
                target.cloned(),
            )
        }
        "manual_review" => {
            return AssertionResult {
                id: a.id.clone(),
                verdict: "inconclusive".into(),
                severity: if a.severity.is_empty() {
                    "critical".into()
                } else {
                    a.severity.clone()
                },
                expected: a.expected.clone(),
                actual: None,
                message: "Requires human review; not automatically evaluated".into(),
            };
        }
        "tool_called" => (
            calls
                .iter()
                .any(|x| x.get("name").and_then(Value::as_str) == a.tool.as_deref()),
            "Required tool call",
            Some(Value::Array(calls.clone())),
        ),
        "tool_not_called" => (
            !calls
                .iter()
                .any(|x| x.get("name").and_then(Value::as_str) == a.tool.as_deref()),
            "Prohibited tool call",
            Some(Value::Array(calls.clone())),
        ),
        _ => {
            return AssertionResult {
                id: a.id.clone(),
                verdict: "inconclusive".into(),
                severity: a.severity.clone(),
                expected: a.expected.clone(),
                actual: actual.cloned(),
                message: format!("Unsupported assertion type '{}'", a.kind),
            };
        }
    };
    AssertionResult {
        id: a.id.clone(),
        verdict: if ok { "passed" } else { "failed" }.into(),
        severity: if a.severity.is_empty() {
            "critical".into()
        } else {
            a.severity.clone()
        },
        expected: a.expected.clone(),
        actual: got,
        message: msg.into(),
    }
}

fn fixture(p: &ProviderConfig, case: &str) -> Result<(Value, u64, Option<f64>, Option<u64>)> {
    if p.kind != "mock" {
        bail!(
            "provider kind '{}' is not supported by this runner (mock only)",
            p.kind
        )
    }
    let path = p
        .fixture
        .as_ref()
        .context("mock provider requires fixture")?;
    let v: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("read fixture {}", path.display()))?,
    )?;
    let x = v
        .get(case)
        .or_else(|| v.get("default"))
        .context(format!("fixture missing case '{case}'"))?
        .clone();
    Ok((
        x.clone(),
        x.get("latency_ms").and_then(Value::as_u64).unwrap_or(0),
        x.get("cost_usd").and_then(Value::as_f64),
        x.get("retry_after_ms").and_then(Value::as_u64),
    ))
}
fn classify_error(error: &str, status: Option<u16>) -> (&'static str, bool) {
    match (error, status) {
        ("rate_limit", _) | (_, Some(429)) => ("rate_limited", true),
        ("timeout", _) => ("timeout", true),
        ("connection_failure", _) => ("connection_failure", true),
        ("provider_unavailable", _) | (_, Some(503)) => ("provider_unavailable", true),
        ("internal_server_error", _) | (_, Some(500)) => ("internal_server_error", true),
        ("context_window_exceeded", _) => ("context_window_exceeded", true),
        ("authentication_failure", _) | (_, Some(401)) => ("authentication_failure", false),
        ("permission_denied", _) | (_, Some(403)) => ("permission_denied", false),
        ("invalid_request", _) | (_, Some(400)) => ("invalid_request", false),
        ("unsupported_capability", _) => ("unsupported_capability", false),
        ("invalid_provider_response", _) | ("invalid_json", _) => {
            ("invalid_provider_response", true)
        }
        ("stream_failure", _) => ("stream_failure", true),
        _ => ("unknown_provider_error", false),
    }
}
fn stored_response(response: &Value, include_content: bool) -> Value {
    if include_content {
        return response.clone();
    }
    let calls=response.get("tool_calls").and_then(Value::as_array).map(|calls|calls.iter().map(|call|serde_json::json!({"id":call.get("id"),"name":call.get("name"),"arguments":"[redacted]"})).collect::<Vec<_>>());
    serde_json::json!({"content":"[redacted]","tool_calls":calls})
}
pub fn execute(c: &Contract, include_content: bool) -> Result<Run> {
    let mut expanded = c.clone();
    expanded.scenarios = c
        .scenarios
        .iter()
        .flat_map(|scenario| {
            let repetitions = scenario.repetitions.clamp(1, 20);
            (0..repetitions).map(move |index| {
                let mut item = scenario.clone();
                item.repetitions = 0;
                if repetitions > 1 {
                    item.id = format!("{} [run {}]", item.id, index + 1)
                }
                item
            })
        })
        .collect();
    execute_once(&expanded, include_content)
}

fn execute_once(c: &Contract, include_content: bool) -> Result<Run> {
    let run_id = Uuid::new_v4().to_string();
    let started_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let serialized = serde_json::to_vec(c)?;
    let config_digest = c
        .source_digest
        .clone()
        .unwrap_or_else(|| format!("sha256:{:x}", sha2::Sha256::digest(serialized)));
    let mut clients = BTreeMap::new();
    for (id, config) in &c.providers {
        if config.kind == "openai_compatible" {
            clients.insert(
                id.clone(),
                provider::build_client(config, c.defaults.timeout_ms)?,
            );
        }
    }
    let mut python_bridge = if c.providers.values().any(|config| config.kind == "ollama") {
        Some(python_bridge::PythonBridge::start(
            std::time::Duration::from_millis(c.defaults.timeout_ms.saturating_add(1000)),
        )?)
    } else {
        None
    };
    let mut scenarios = Vec::new();
    for s in &c.scenarios {
        let case_ids = if s.cases.is_empty() {
            c.cases.iter().map(|x| x.id.clone()).collect()
        } else {
            s.cases.clone()
        };
        let mut attempts = vec![];
        let mut case_results = vec![];
        let mut events = vec![];
        let mut elapsed_ms = 0u64;
        events.push(Event {
            event_id: Uuid::new_v4().to_string(),
            run_id: run_id.clone(),
            scenario_id: s.id.clone(),
            case_id: None,
            attempt_id: None,
            at_ms: 0,
            kind: "scenario_started".into(),
            provider: s.primary.clone(),
            message: s.description.clone(),
        });
        for cid in case_ids {
            let attempt_start = attempts.len();
            let case = c
                .cases
                .iter()
                .find(|x| x.id == cid)
                .context("validated case missing")?;
            events.push(Event {
                event_id: Uuid::new_v4().to_string(),
                run_id: run_id.clone(),
                scenario_id: s.id.clone(),
                case_id: Some(cid.clone()),
                attempt_id: None,
                at_ms: elapsed_ms,
                kind: "case_started".into(),
                provider: s.primary.clone(),
                message: case.description.clone(),
            });
            let mut providers = vec![s.primary.clone()];
            providers.extend(s.fallbacks.clone());
            let mut chain = Vec::new();
            for provider in providers {
                for retry_index in 0..=c.defaults.max_retries_per_provider {
                    chain.push((provider.clone(), retry_index));
                }
            }
            chain.truncate(c.defaults.max_total_attempts);
            let mut final_case = None;
            let mut provider_attempts: BTreeMap<String, usize> = BTreeMap::new();
            for (idx, (pid, retry_index)) in chain.iter().enumerate() {
                if c.defaults
                    .overall_deadline_ms
                    .is_some_and(|deadline| elapsed_ms >= deadline)
                {
                    events.push(Event {
                        event_id: Uuid::new_v4().to_string(),
                        run_id: run_id.clone(),
                        scenario_id: s.id.clone(),
                        case_id: Some(cid.clone()),
                        attempt_id: None,
                        at_ms: elapsed_ms,
                        kind: "deadline_exhausted".into(),
                        provider: pid.clone(),
                        message: "Overall deadline exhausted before the next provider attempt"
                            .into(),
                    });
                    break;
                }
                let attempt_id = Uuid::new_v4().to_string();
                events.push(Event {
                    event_id: Uuid::new_v4().to_string(),
                    run_id: run_id.clone(),
                    scenario_id: s.id.clone(),
                    case_id: Some(cid.clone()),
                    attempt_id: Some(attempt_id.clone()),
                    at_ms: elapsed_ms,
                    kind: "attempt_started".into(),
                    provider: pid.clone(),
                    message: format!("Attempt {} for {cid}", idx + 1),
                });
                let attempt_number = provider_attempts.entry(pid.clone()).or_default();
                *attempt_number += 1;
                let configured_injection = s.injections.iter().find(|x| x.provider == *pid);
                let injection =
                    configured_injection.filter(|item| *attempt_number <= item.times.unwrap_or(1));
                let fault = injection.map(|x| x.fault.as_str());
                let timeout_ms = c.providers[pid].timeout_ms.unwrap_or(c.defaults.timeout_ms);
                let remaining_deadline = c
                    .defaults
                    .overall_deadline_ms
                    .map(|deadline| deadline.saturating_sub(elapsed_ms))
                    .unwrap_or(u64::MAX);
                let allowed_latency = timeout_ms.min(remaining_deadline);
                let (raw, latency, cost, provider_retry_after) = if let Some(f) = fault {
                    (
                        serde_json::json!({"error":f,"status":injection.and_then(|i|i.status)}),
                        injection
                            .and_then(|i| i.latency_ms)
                            .unwrap_or(if f == "timeout" { timeout_ms } else { 0 }),
                        Some(0.0),
                        injection.and_then(|i| i.retry_after_ms),
                    )
                } else if c.providers[pid].kind == "mock" {
                    let (raw, latency, cost, retry_after) = fixture(&c.providers[pid], &cid)?;
                    (raw, latency, cost, retry_after)
                } else if c.providers[pid].kind == "openai_compatible" {
                    let client = clients
                        .get(pid)
                        .context("provider HTTP client was not initialized")?;
                    let response = provider::call_openai_compatible(
                        client,
                        &c.providers[pid],
                        case,
                        allowed_latency,
                    )?;
                    (
                        response.raw,
                        response.latency_ms,
                        response.cost_usd,
                        response.retry_after_ms,
                    )
                } else if c.providers[pid].kind == "ollama" {
                    let bridge = python_bridge
                        .as_mut()
                        .context("Python adapter process was not initialized")?;
                    let response =
                        provider::call_ollama(bridge, &c.providers[pid], case, allowed_latency);
                    (
                        response.raw,
                        response.latency_ms,
                        response.cost_usd,
                        response.retry_after_ms,
                    )
                } else {
                    bail!("unsupported provider kind {}", c.providers[pid].kind)
                };
                let simulated_timeout = fault == Some("timeout") || latency > allowed_latency;
                let fail = if simulated_timeout {
                    Some("timeout".to_owned())
                } else {
                    fault
                        .map(str::to_owned)
                        .or_else(|| raw.get("error").and_then(Value::as_str).map(str::to_owned))
                };
                let response = if fail.is_none() {
                    Some(raw.get("response").cloned().unwrap_or_else(|| {
                        let mut x = raw.clone();
                        if let Some(content) = raw.get("content") {
                            x["content"] = content.clone()
                        }
                        x
                    }))
                } else {
                    None
                };
                let http_status = injection.and_then(|item| item.status).or_else(|| {
                    raw.get("status")
                        .and_then(Value::as_u64)
                        .and_then(|status| u16::try_from(status).ok())
                });
                let (classification, retryable) = fail
                    .as_deref()
                    .map(|error| classify_error(error, http_status))
                    .map(|(name, retryable)| (Some(name.to_owned()), retryable))
                    .unwrap_or((None, false));
                let schedules_retry = retryable
                    && chain
                        .get(idx + 1)
                        .is_some_and(|(next_provider, _)| next_provider == pid);
                let retry_metadata = injection
                    .and_then(|item| item.retry_after_ms)
                    .or(provider_retry_after);
                let retry_delay =
                    schedules_retry.then(|| retry_metadata.unwrap_or(c.defaults.retry_backoff_ms));
                let measured_latency = latency.min(allowed_latency);
                elapsed_ms = elapsed_ms.saturating_add(measured_latency);
                if let Some(delay) = retry_delay {
                    elapsed_ms = elapsed_ms.saturating_add(delay);
                }
                attempts.push(Attempt {
                    attempt_id: attempt_id.clone(),
                    provider: pid.clone(),
                    model: c.providers[pid].model.clone(),
                    capabilities: c.providers[pid].capabilities.clone(),
                    case_id: cid.clone(),
                    outcome: if fail.is_some() {
                        "failed"
                    } else {
                        "responded"
                    }
                    .into(),
                    failure: classification.clone(),
                    classification: classification.clone(),
                    http_status,
                    retryable,
                    retry_after_ms: retry_metadata,
                    failure_stage: fail.as_ref().map(|_| {
                        if c.providers[pid].kind == "mock" {
                            "mock_provider"
                        } else {
                            "http_adapter"
                        }
                        .into()
                    }),
                    latency_ms: measured_latency,
                    cost_usd: cost,
                    usage: raw.get("usage").cloned(),
                    response: response
                        .as_ref()
                        .map(|r| stored_response(r, include_content)),
                });
                if fail.is_some() {
                    let category = classification
                        .as_deref()
                        .unwrap_or("unknown_provider_error");
                    if injection.is_some() {
                        events.push(Event {
                            event_id: Uuid::new_v4().to_string(),
                            run_id: run_id.clone(),
                            scenario_id: s.id.clone(),
                            case_id: Some(cid.clone()),
                            attempt_id: Some(attempt_id.clone()),
                            at_ms: elapsed_ms,
                            kind: "fault_injected".into(),
                            provider: pid.clone(),
                            message: format!(
                                "Injected {}",
                                injection
                                    .map(|item| item.fault.as_str())
                                    .unwrap_or("provider fault")
                            ),
                        });
                    }
                    events.push(Event {
                        event_id: Uuid::new_v4().to_string(),
                        run_id: run_id.clone(),
                        scenario_id: s.id.clone(),
                        case_id: Some(cid.clone()),
                        attempt_id: Some(attempt_id.clone()),
                        at_ms: elapsed_ms,
                        kind: "provider_failed".into(),
                        provider: pid.clone(),
                        message: format!("Provider failed: {category}"),
                    });
                    let next = chain.get(idx + 1);
                    let next_is_retry = next.is_some_and(|(next_provider, _)| next_provider == pid);
                    if retryable && next_is_retry {
                        let retry_delay = retry_delay.unwrap_or_default();
                        if c.providers[pid].kind != "mock" && retry_delay > 0 {
                            std::thread::sleep(std::time::Duration::from_millis(retry_delay));
                        }
                        events.push(Event {
                            event_id: Uuid::new_v4().to_string(),
                            run_id: run_id.clone(),
                            scenario_id: s.id.clone(),
                            case_id: Some(cid.clone()),
                            attempt_id: Some(attempt_id.clone()),
                            at_ms: elapsed_ms,
                            kind: "retry_scheduled".into(),
                            provider: pid.clone(),
                            message: format!(
                                "Retry {} scheduled after {category}; delay {retry_delay} ms",
                                retry_index + 1
                            ),
                        })
                    } else if retryable
                        && next.is_some()
                        && c.defaults
                            .fallback_on
                            .iter()
                            .any(|allowed| allowed == category)
                    {
                        events.push(Event {
                            event_id: Uuid::new_v4().to_string(),
                            run_id: run_id.clone(),
                            scenario_id: s.id.clone(),
                            case_id: Some(cid.clone()),
                            attempt_id: Some(attempt_id.clone()),
                            at_ms: elapsed_ms,
                            kind: "fallback_selected".into(),
                            provider: next.unwrap().0.clone(),
                            message: format!("Fallback selected after {category}"),
                        })
                    }
                    if retryable && next_is_retry {
                        continue;
                    }
                    if retryable
                        && next.is_some()
                        && c.defaults
                            .fallback_on
                            .iter()
                            .any(|allowed| allowed == category)
                    {
                        continue;
                    }
                    break;
                }
                let response = response.unwrap();
                events.push(Event {
                    event_id: Uuid::new_v4().to_string(),
                    run_id: run_id.clone(),
                    scenario_id: s.id.clone(),
                    case_id: Some(cid.clone()),
                    attempt_id: Some(attempt_id.clone()),
                    at_ms: elapsed_ms,
                    kind: "provider_response_received".into(),
                    provider: pid.clone(),
                    message: "Provider returned a response; evaluating application contract".into(),
                });
                let schema = case
                    .output_schema
                    .as_ref()
                    .map(|p| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(p)?)?) })
                    .transpose()?;
                let mut assertions = evaluate_response(case, &response, schema.as_ref());
                assertions.extend(evaluate_capability_requirements(
                    case,
                    c.providers[pid].capabilities.as_ref(),
                ));
                if !include_content {
                    for assertion in &mut assertions {
                        assertion.actual = None;
                    }
                }
                let verdict = if assertions
                    .iter()
                    .any(|a| a.verdict == "failed" && a.severity == "critical")
                {
                    "failed"
                } else if assertions.iter().any(|a| a.verdict == "inconclusive") {
                    "inconclusive"
                } else {
                    "passed"
                };
                events.push(Event {
                    event_id: Uuid::new_v4().to_string(),
                    run_id: run_id.clone(),
                    scenario_id: s.id.clone(),
                    case_id: Some(cid.clone()),
                    attempt_id: Some(attempt_id),
                    at_ms: elapsed_ms,
                    kind: "contract_evaluated".into(),
                    provider: pid.clone(),
                    message: format!("Contract verdict: {verdict}"),
                });
                final_case = Some(CaseResult {
                    case_id: cid.clone(),
                    provider: Some(pid.clone()),
                    input_messages: include_content.then(|| case.messages.clone()),
                    verdict: verdict.into(),
                    assertions,
                });
                break;
            }
            let mut completed = final_case.unwrap_or(CaseResult {
                case_id: cid,
                provider: None,
                input_messages: None,
                verdict: "error".into(),
                assertions: vec![],
            });
            let case_attempts = &attempts[attempt_start..];
            let total_latency: u64 = case_attempts
                .iter()
                .map(|attempt| {
                    attempt
                        .latency_ms
                        .saturating_add(attempt.retry_after_ms.unwrap_or_default())
                })
                .sum();
            if let Some(budget) = &c.latency {
                completed.assertions.push(AssertionResult {
                    id: "latency-budget".into(),
                    verdict: if total_latency <= budget.max_end_to_end_ms {
                        "passed"
                    } else {
                        "failed"
                    }
                    .into(),
                    severity: "critical".into(),
                    expected: Some(serde_json::json!(budget.max_end_to_end_ms)),
                    actual: Some(serde_json::json!(total_latency)),
                    message: format!("End-to-end fallback-path latency: {total_latency} ms"),
                });
            }
            if let Some(budget) = &c.cost {
                let total_cost = case_attempts
                    .iter()
                    .try_fold(0.0, |sum, attempt| attempt.cost_usd.map(|cost| sum + cost));
                completed.assertions.push(match total_cost {
                    Some(total) => AssertionResult {
                        id: "cost-budget".into(),
                        verdict: if total <= budget.max_total_usd {
                            "passed"
                        } else {
                            "failed"
                        }
                        .into(),
                        severity: "critical".into(),
                        expected: Some(serde_json::json!(budget.max_total_usd)),
                        actual: Some(serde_json::json!(total)),
                        message: format!("Fallback-path cost: ${total:.6}"),
                    },
                    None => AssertionResult {
                        id: "cost-budget".into(),
                        verdict: "inconclusive".into(),
                        severity: "critical".into(),
                        expected: Some(serde_json::json!(budget.max_total_usd)),
                        actual: None,
                        message: "Usage cost is unavailable for one or more attempts".into(),
                    },
                });
            }
            if !include_content {
                for assertion in &mut completed.assertions {
                    assertion.actual = None;
                }
            }
            events.push(Event {
                event_id: Uuid::new_v4().to_string(),
                run_id: run_id.clone(),
                scenario_id: s.id.clone(),
                case_id: Some(completed.case_id.clone()),
                attempt_id: None,
                at_ms: elapsed_ms,
                kind: "budget_evaluated".into(),
                provider: completed.provider.clone().unwrap_or_default(),
                message: "Latency and cost constraints evaluated when configured".into(),
            });
            if completed
                .assertions
                .iter()
                .any(|assertion| assertion.verdict == "failed" && assertion.severity == "critical")
            {
                completed.verdict = "failed".into()
            } else if completed
                .assertions
                .iter()
                .any(|assertion| assertion.verdict == "inconclusive")
            {
                completed.verdict = "inconclusive".into()
            }
            case_results.push(completed)
        }
        let verdict = if case_results
            .iter()
            .any(|x| x.verdict == "failed" || x.verdict == "error")
        {
            "failed"
        } else if case_results.iter().any(|x| x.verdict == "inconclusive") {
            "inconclusive"
        } else {
            "passed"
        };
        events.push(Event {
            event_id: Uuid::new_v4().to_string(),
            run_id: run_id.clone(),
            scenario_id: s.id.clone(),
            case_id: None,
            attempt_id: None,
            at_ms: elapsed_ms,
            kind: "scenario_completed".into(),
            provider: String::new(),
            message: format!("Scenario verdict: {verdict}"),
        });
        scenarios.push(ScenarioResult {
            id: s.id.clone(),
            description: s.description.clone(),
            verdict: verdict.into(),
            attempts,
            cases: case_results,
            events,
        })
    }
    if let Some(first) = scenarios.first_mut() {
        first.events.insert(
            0,
            Event {
                event_id: Uuid::new_v4().to_string(),
                run_id: run_id.clone(),
                scenario_id: first.id.clone(),
                case_id: None,
                attempt_id: None,
                at_ms: 0,
                kind: "run_started".into(),
                provider: String::new(),
                message: "Contract run started".into(),
            },
        );
    }
    let scenario_count = scenarios.len();
    if let Some(last) = scenarios.last_mut() {
        let elapsed = last
            .events
            .iter()
            .map(|event| event.at_ms)
            .max()
            .unwrap_or(0);
        last.events.push(Event {
            event_id: Uuid::new_v4().to_string(),
            run_id: run_id.clone(),
            scenario_id: last.id.clone(),
            case_id: None,
            attempt_id: None,
            at_ms: elapsed,
            kind: "run_completed".into(),
            provider: String::new(),
            message: format!("Contract run completed with {scenario_count} scenarios"),
        });
    }
    Ok(Run {
        schema_version: RESULT_SCHEMA_VERSION,
        run_id,
        contract: c.name.clone(),
        tool_version: env!("CARGO_PKG_VERSION").into(),
        config_digest,
        started_at_unix_ms,
        environment: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        scenarios,
    })
}

pub fn write_artifacts(run: &Run, out: &Path) -> Result<()> {
    fs::create_dir_all(out)?;
    fs::write(out.join("run.json"), serde_json::to_vec_pretty(run)?)?;
    let mut repeats: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    for scenario in &run.scenarios {
        let name = scenario
            .id
            .split_once(" [run ")
            .map(|(name, _)| name)
            .unwrap_or(&scenario.id)
            .to_owned();
        let totals = repeats.entry(name).or_default();
        totals.0 += 1;
        match scenario.verdict.as_str() {
            "passed" => totals.1 += 1,
            "failed" | "error" => totals.2 += 1,
            _ => totals.3 += 1,
        }
    }
    let repeated_summary=repeats.into_iter().filter(|(_,counts)|counts.0>1).map(|(scenario,(runs,passed,failed,inconclusive))|serde_json::json!({"scenario":scenario,"runs":runs,"passed":passed,"failed":failed,"inconclusive":inconclusive,"observed_sample_pass_rate":(passed as f64)/(runs as f64),"label":"observed sample pass rate; not a reliability guarantee"})).collect::<Vec<_>>();
    let summary = serde_json::json!({"schema_version":RESULT_SCHEMA_VERSION,"run_id":run.run_id,"contract":run.contract,"repetitions":repeated_summary,"scenarios":run.scenarios.iter().map(|s|serde_json::json!({"id":s.id,"verdict":s.verdict,"cases":s.cases.len(),"attempts":s.attempts.len()})).collect::<Vec<_>>()});
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    fs::write(out.join("junit.xml"), junit(run))?;
    fs::create_dir_all(out.join("report/assets"))?;
    fs::write(out.join("report/index.html"), html_report(run)?)?;
    Ok(())
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn junit(r: &Run) -> String {
    let n: usize = r.scenarios.iter().map(|s| s.cases.len()).sum();
    let f: usize = r
        .scenarios
        .iter()
        .flat_map(|s| &s.cases)
        .filter(|c| c.verdict == "failed")
        .count();
    let errors: usize = r
        .scenarios
        .iter()
        .flat_map(|s| &s.cases)
        .filter(|case| case.verdict == "error")
        .count();
    let skipped: usize = r
        .scenarios
        .iter()
        .flat_map(|s| &s.cases)
        .filter(|case| case.verdict == "inconclusive")
        .count();
    let mut x = format!(
        "<?xml version=\"1.0\"?><testsuite name=\"LLM Fallback Compatibility Tester\" tests=\"{n}\" failures=\"{f}\" errors=\"{errors}\" skipped=\"{skipped}\">"
    );
    for s in &r.scenarios {
        for c in &s.cases {
            x.push_str(&format!(
                "<testcase classname=\"{}\" name=\"{}\">",
                esc(&s.id),
                esc(&c.case_id)
            ));
            if c.verdict == "failed" {
                x.push_str("<failure message=\"contract failed\"/>")
            } else if c.verdict == "error" {
                x.push_str("<error message=\"provider attempts exhausted\"/>")
            } else if c.verdict == "inconclusive" {
                x.push_str("<skipped message=\"required check was inconclusive\"/>")
            }
            x.push_str("</testcase>")
        }
    }
    x.push_str("</testsuite>");
    x
}
fn html_report(r: &Run) -> Result<String> {
    let passed = r.scenarios.iter().filter(|s| s.verdict == "passed").count();
    let failed = r
        .scenarios
        .iter()
        .filter(|s| s.verdict == "failed" || s.verdict == "error")
        .count();
    let inconclusive = r
        .scenarios
        .iter()
        .filter(|s| s.verdict == "inconclusive")
        .count();
    let attempts: usize = r.scenarios.iter().map(|s| s.attempts.len()).sum();
    let latency_ms: u64 = r
        .scenarios
        .iter()
        .flat_map(|s| &s.attempts)
        .map(|a| a.latency_ms)
        .sum();
    let cost: f64 = r
        .scenarios
        .iter()
        .flat_map(|s| &s.attempts)
        .filter_map(|a| a.cost_usd)
        .sum();
    let providers: std::collections::BTreeSet<_> = r
        .scenarios
        .iter()
        .flat_map(|s| &s.attempts)
        .map(|a| a.provider.as_str())
        .collect();
    let mut matrix = String::new();
    let mut details = String::new();
    for scenario in &r.scenarios {
        for case in &scenario.cases {
            matrix.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{}</td></tr>",
                esc(&scenario.id),
                esc(&case.case_id),
                esc(case.provider.as_deref().unwrap_or("unavailable")),
                esc(&case.verdict),
                esc(&case.verdict.to_uppercase())
            ));
        }
        let attempt_cards=scenario.attempts.iter().enumerate().map(|(index,attempt)|format!("<li class=\"attempt {}\"><span class=\"step\">{}</span><div><b>{}</b> <span class=\"muted\">{}</span><p>{}</p></div><div class=\"metrics\">{} ms<br/>{}</div></li>",if attempt.outcome=="failed"{"failed"}else{"passed"},index+1,esc(&attempt.provider),esc(attempt.model.as_deref().unwrap_or("model unspecified")),esc(attempt.classification.as_deref().or(attempt.failure.as_deref()).unwrap_or("Provider responded")),attempt.latency_ms,attempt.cost_usd.map(|v|format!("${v:.6}")).unwrap_or_else(||"cost unknown".into()))).collect::<String>();
        let event_rows = scenario
            .events
            .iter()
            .map(|event| {
                format!(
                    "<li><time>+{} ms</time><b>{}</b><span>{}</span></li>",
                    event.at_ms,
                    esc(&event.kind),
                    esc(&event.message)
                )
            })
            .collect::<String>();
        let assertion_cards = scenario.cases.iter().flat_map(|case| {
            let case_id=esc(&case.case_id);
            let provider=esc(case.provider.as_deref().unwrap_or("unavailable"));
            case.assertions.iter().map(move |assertion| format!(
                "<article class=\"assertion {}\"><div class=\"assertion-head\"><b>{}</b><span class=\"{}\">{}</span></div><p>{}</p><div class=\"values\"><div><small>EXPECTED</small><code>{}</code></div><div><small>ACTUAL</small><code>{}</code></div></div><small class=\"muted\">case {} · provider {}</small></article>",
                esc(&assertion.verdict),esc(&assertion.id),esc(&assertion.verdict),esc(&assertion.verdict.to_uppercase()),esc(&assertion.message),
                esc(&assertion.expected.as_ref().map(Value::to_string).unwrap_or_else(||"(not specified)".into())),
                esc(&assertion.actual.as_ref().map(Value::to_string).unwrap_or_else(||"redacted or unavailable".into())),case_id,provider))
        }).collect::<String>();
        details.push_str(&format!("<section id=\"scenario-{}\"><header class=\"scenario-head\"><div><p class=\"eyebrow\">SCENARIO</p><h2>{}</h2><p class=\"muted\">{}</p></div><strong class=\"verdict {}\">{}</strong></header><h3>Failover path</h3><ol class=\"attempts\">{}</ol><details><summary>Execution events</summary><ol class=\"events\">{}</ol></details><h3>Contract assertions</h3>{}</section>",esc(&scenario.id),esc(&scenario.id),esc(&scenario.description),esc(&scenario.verdict),esc(&scenario.verdict.to_uppercase()),attempt_cards,event_rows,assertion_cards));
    }
    Ok(format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta name="color-scheme" content="light dark"><title>LLM Fallback Compatibility Tester report · {}</title><style>
:root{{font-family:Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;color:#17212b;background:#f3f6f8;font-synthesis:none}}*{{box-sizing:border-box}}body{{margin:0}}main{{max-width:1180px;margin:0 auto;padding:32px 22px 70px}}header.hero{{border-bottom:1px solid #d8e0e7;padding-bottom:22px}}.eyebrow,small{{letter-spacing:.09em;font-size:11px;color:#657382}}h1{{font-size:27px;margin:4px 0}}h2{{margin:0;font-size:21px}}h3{{font-size:15px;margin:26px 0 10px}}.muted{{color:#667482}}.stats{{display:grid;grid-template-columns:repeat(auto-fit,minmax(145px,1fr));gap:10px;margin:20px 0}}.stat,section,.matrix{{background:#fff;border:1px solid #dce3e9;border-radius:12px}}.stat{{padding:13px 15px}}.stat b{{display:block;font-size:22px;margin-top:4px}}section{{padding:22px;margin:18px 0}}.scenario-head,.assertion-head{{display:flex;justify-content:space-between;align-items:flex-start;gap:16px}}.verdict,.assertion-head span{{font-size:12px;font-weight:700;text-transform:uppercase}}.passed{{color:#087443}}.failed,.error{{color:#b42318}}.inconclusive{{color:#805b00}}.attempts,.events{{padding:0;list-style:none}}.attempt{{display:flex;gap:12px;align-items:flex-start;border:1px solid #e1e7eb;border-radius:10px;padding:13px;margin:8px 0}}.attempt.failed{{border-left:4px solid #c33}}.attempt.passed{{border-left:4px solid #198754}}.step{{width:25px;height:25px;border-radius:50%;display:grid;place-items:center;background:#edf1f4;color:#17212b;flex:none}}.attempt p{{margin:5px 0 0}}.metrics{{margin-left:auto;text-align:right;white-space:nowrap;font-variant-numeric:tabular-nums}}details{{margin:10px 0;color:#3e4b57}}summary{{cursor:pointer}}.events li{{display:grid;grid-template-columns:75px 175px 1fr;gap:10px;padding:7px;border-bottom:1px solid #edf0f2;font-size:13px}}time{{font-variant-numeric:tabular-nums;color:#718090}}.assertion{{border:1px solid #e0e6eb;border-left:4px solid #8795a2;border-radius:8px;padding:12px;margin:9px 0}}.assertion.failed{{border-left-color:#bd3128}}.assertion.passed{{border-left-color:#16814d}}.assertion p{{margin:9px 0}}.values{{display:grid;grid-template-columns:1fr 1fr;gap:8px;margin:12px 0}}.values div{{background:#f5f7f9;padding:9px;border-radius:6px;min-width:0}}code{{display:block;white-space:pre-wrap;overflow-wrap:anywhere;margin-top:5px}}.matrix{{width:100%;border-collapse:collapse;overflow:hidden}}.matrix th,.matrix td{{padding:11px;text-align:left;border-bottom:1px solid #e6ebef}}.matrix th{{font-size:11px;letter-spacing:.08em;color:#657382}}@media(max-width:650px){{main{{padding:20px 12px}}section{{padding:15px}}.events li{{grid-template-columns:55px 1fr}}.events li span{{grid-column:2}}.values{{grid-template-columns:1fr}}.matrix th,.matrix td{{padding:8px 5px;font-size:12px}}}}@media(prefers-color-scheme:dark){{:root{{color:#e2e8ee;background:#101820}}.stat,section,.matrix{{background:#17232d;border-color:#293844}}.attempt,.assertion{{border-color:#31414d}}.values div{{background:#202d38}}.matrix th,.matrix td,.events li{{border-color:#293844}}.muted,small,.eyebrow,time{{color:#aab6c0}}}}
</style></head><body><main><header class="hero"><p class="eyebrow">BEHAVIORAL COMPATIBILITY RUN</p><h1>{}</h1><p class="muted">Run {} · {} · config {}</p><p class="muted">Environment: {} · Generated {}</p></header><div class="stats"><div class="stat">Scenarios<b>{}</b></div><div class="stat">Passed<b class="passed">{}</b></div><div class="stat">Failed<b class="failed">{}</b></div><div class="stat">Inconclusive<b class="inconclusive">{}</b></div><div class="stat">Providers<b>{}</b></div><div class="stat">Attempts / latency<b>{} / {} ms</b></div><div class="stat">Observed fixture cost<b>${:.6}</b></div></div><h2>Compatibility matrix</h2><table class="matrix"><thead><tr><th>SCENARIO</th><th>CASE</th><th>PROVIDER</th><th>VERDICT</th></tr></thead><tbody>{}</tbody></table><h2 style="margin-top:30px">Scenario investigation</h2>{}</main></body></html>"#,
        esc(&r.contract),
        esc(&r.contract),
        esc(&r.run_id),
        esc(&r.tool_version),
        esc(&r.config_digest),
        esc(&r.environment),
        r.started_at_unix_ms,
        r.scenarios.len(),
        passed,
        failed,
        inconclusive,
        providers.len(),
        attempts,
        latency_ms,
        cost,
        matrix,
        details
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrong_business_value_fails() {
        let a = Assertion {
            id: "category".into(),
            kind: "json_equals".into(),
            path: Some("/category".into()),
            expected: Some(Value::String("billing".into())),
            values: vec![],
            min: None,
            max: None,
            severity: "critical".into(),
            tool: None,
            ..Default::default()
        };
        let c = Case {
            id: "c".into(),
            description: String::new(),
            messages: vec![],
            output_schema: None,
            assertions: vec![a],
            tools: vec![],
            requires_capabilities: vec![],
        };
        assert_eq!(
            evaluate_response(&c, &serde_json::json!({"category":"refund"}), None)[0].verdict,
            "failed"
        )
    }
    #[test]
    fn unsupported_check_is_inconclusive() {
        let a = Assertion {
            id: "manual".into(),
            kind: "manual_review".into(),
            path: None,
            expected: None,
            values: vec![],
            min: None,
            max: None,
            severity: "warning".into(),
            tool: None,
            ..Default::default()
        };
        assert_eq!(
            eval_assertion(&a, &Value::Null, &Value::Null).verdict,
            "inconclusive"
        )
    }

    #[test]
    fn required_capabilities_are_explicit_and_unknown_is_inconclusive() {
        let case = Case {
            id: "capability-case".into(),
            description: String::new(),
            messages: vec![],
            output_schema: None,
            assertions: vec![],
            tools: vec![],
            requires_capabilities: vec!["tool_calling".into()],
        };
        let supported = BTreeMap::from([("tool_calling".into(), true)]);
        let unsupported = BTreeMap::from([("tool_calling".into(), false)]);
        assert_eq!(
            evaluate_capability_requirements(&case, Some(&supported))[0].verdict,
            "passed"
        );
        assert_eq!(
            evaluate_capability_requirements(&case, Some(&unsupported))[0].verdict,
            "failed"
        );
        assert_eq!(
            evaluate_capability_requirements(&case, None)[0].verdict,
            "inconclusive"
        );
    }

    #[test]
    fn exact_assertions_cover_string_array_regex_and_conditional_rules() {
        let body = serde_json::json!({"label":"Billing","tags":["urgent","duplicate"],"amount":24,"approved":false});
        let make = |kind: &str, path: &str, expected: Value| Assertion {
            id: kind.into(),
            kind: kind.into(),
            path: Some(path.into()),
            expected: Some(expected),
            severity: "critical".into(),
            ..Default::default()
        };
        assert_eq!(
            eval_assertion(
                &Assertion {
                    case_sensitive: Some(false),
                    ..make("string_equals", "/label", serde_json::json!("billing"))
                },
                &body,
                &Value::Null
            )
            .verdict,
            "passed"
        );
        assert_eq!(
            eval_assertion(
                &make("contains", "/tags", serde_json::json!("urgent")),
                &body,
                &Value::Null
            )
            .verdict,
            "passed"
        );
        assert_eq!(
            eval_assertion(
                &Assertion {
                    pattern: Some("^Bill.*$".into()),
                    ..make("matches_regex", "/label", Value::Null)
                },
                &body,
                &Value::Null
            )
            .verdict,
            "passed"
        );
        assert_eq!(
            eval_assertion(
                &Assertion {
                    if_path: Some("/amount".into()),
                    if_equals: Some(serde_json::json!(24)),
                    then_path: Some("/approved".into()),
                    then_equals: Some(Value::Bool(true)),
                    ..make("conditional_equals", "", Value::Null)
                },
                &body,
                &Value::Null
            )
            .verdict,
            "failed"
        );
    }

    #[test]
    fn tool_arguments_are_validated_against_tool_schema() {
        let case = Case {
            id: "tool".into(),
            description: String::new(),
            messages: vec![],
            output_schema: None,
            assertions: vec![],
            tools: vec![ToolDefinition {
                name: "create_ticket".into(),
                description: String::new(),
                parameters: serde_json::json!({"type":"object","required":["priority"],"properties":{"priority":{"type":"string","enum":["low","high"]}},"additionalProperties":false}),
            }],
            requires_capabilities: vec![],
        };
        let response = serde_json::json!({"content":{},"tool_calls":[{"name":"create_ticket","arguments":{"priority":"urgent"}}]});
        assert_eq!(
            evaluate_response(&case, &response, None)
                .iter()
                .find(|a| a.id.starts_with("tool-arguments"))
                .unwrap()
                .verdict,
            "failed"
        );
    }

    #[test]
    fn flagship_run_has_real_safe_and_unsafe_verdicts_and_budgets() {
        let config =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/support-triage/contract.yaml");
        let contract = load_contract(&config).unwrap();
        let run = execute(&contract, true).unwrap();
        assert_eq!(
            run.scenarios
                .iter()
                .find(|s| s.id == "safe-fallback")
                .unwrap()
                .verdict,
            "passed"
        );
        let unsafe_case = run
            .scenarios
            .iter()
            .find(|s| s.id == "unsafe-fallback")
            .unwrap();
        assert_eq!(unsafe_case.verdict, "failed");
        assert!(
            unsafe_case
                .attempts
                .iter()
                .any(|attempt| attempt.http_status == Some(429)
                    && attempt.classification.as_deref() == Some("rate_limited"))
        );
        assert!(
            unsafe_case.cases[0]
                .assertions
                .iter()
                .any(|assertion| assertion.id == "cost-budget" && assertion.verdict == "passed")
        );
        assert!(unsafe_case.cases[0].input_messages.is_some());
        let redacted = execute(&contract, false).unwrap();
        assert!(redacted.scenarios[1].cases[0].input_messages.is_none());
        assert_eq!(
            redacted.scenarios[1].attempts[1].response.as_ref().unwrap()["content"],
            "[redacted]"
        );
    }

    #[test]
    fn authentication_failure_is_not_fallback_eligible() {
        assert_eq!(
            classify_error("authentication_failure", Some(401)),
            ("authentication_failure", false)
        );
    }

    #[test]
    fn retry_budget_is_bounded_and_records_retry_transition() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/support-triage/contract.yaml");
        let mut contract = load_contract(&path).unwrap();
        contract.defaults.max_retries_per_provider = 1;
        contract.defaults.max_total_attempts = 2;
        contract.scenarios[0].injections[0].times = Some(1);
        let run = execute(&contract, true).unwrap();
        let safe = &run.scenarios[0];
        assert_eq!(safe.attempts.len(), 2);
        assert!(
            safe.attempts
                .iter()
                .all(|attempt| attempt.provider == "primary")
        );
        assert!(
            safe.events
                .iter()
                .any(|event| event.kind == "retry_scheduled")
        );
    }
}
