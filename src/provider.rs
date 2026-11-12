use crate::{Case, ProviderConfig, ToolDefinition};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{env, io::Read, time::Instant};
use uuid::Uuid;
const MAX_PROVIDER_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug)]
pub struct ProviderResponse {
    pub raw: Value,
    pub latency_ms: u64,
    pub cost_usd: Option<f64>,
    pub retry_after_ms: Option<u64>,
}

pub fn build_client(
    config: &ProviderConfig,
    default_timeout_ms: u64,
) -> Result<reqwest::blocking::Client> {
    let timeout = std::time::Duration::from_millis(
        config
            .timeout_ms
            .unwrap_or(default_timeout_ms)
            .clamp(1, 300_000),
    );
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("create provider HTTP client")
}

pub fn call_openai_compatible(
    client: &reqwest::blocking::Client,
    config: &ProviderConfig,
    case: &Case,
    timeout_ms: u64,
) -> Result<ProviderResponse> {
    let base = config
        .base_url
        .as_deref()
        .context("openai_compatible provider requires base_url")?;
    let model = config
        .model
        .as_deref()
        .context("openai_compatible provider requires model")?;
    validate_endpoint(base)?;
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let messages: Vec<Value> = case
        .messages
        .iter()
        .map(|message| json!({"role":message.role,"content":message.content}))
        .collect();
    let tools: Vec<Value> = case.tools.iter().map(openai_tool).collect();
    let mut body = json!({"model":model,"messages":messages,"stream":false});
    if let Some(temperature) = config.temperature {
        body["temperature"] = json!(temperature);
    }
    if let Some(top_p) = config.top_p {
        body["top_p"] = json!(top_p);
    }
    if let Some(max_tokens) = config.max_output_tokens {
        body["max_tokens"] = json!(max_tokens);
    }
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }
    if let Some(schema_path) = &case.output_schema {
        let schema: Value = serde_json::from_slice(
            &std::fs::read(schema_path)
                .with_context(|| format!("read response schema {}", schema_path.display()))?,
        )?;
        body["response_format"] = json!({"type":"json_schema","json_schema":{"name":"fallbackcontract_response","strict":true,"schema":schema}});
    }
    let mut request = client
        .post(url)
        .timeout(std::time::Duration::from_millis(timeout_ms.max(1)))
        .json(&body);
    if let Some(variable) = &config.api_key_env {
        let key = match env::var(variable) {
            Ok(key) => key,
            Err(_) => {
                return Ok(ProviderResponse {
                    raw: json!({"error":"authentication_failure"}),
                    latency_ms: 0,
                    cost_usd: None,
                    retry_after_ms: None,
                });
            }
        };
        request = request.bearer_auth(key);
    }
    let start = Instant::now();
    let response = match request.send() {
        Ok(response) => response,
        Err(error) => {
            let category = if error.is_timeout() {
                "timeout"
            } else if error.is_connect() {
                "connection_failure"
            } else {
                "unknown_provider_error"
            };
            return Ok(ProviderResponse {
                raw: json!({"error":category}),
                latency_ms: start.elapsed().as_millis().min(u64::MAX as u128) as u64,
                cost_usd: None,
                retry_after_ms: None,
            });
        }
    };
    let status = response.status();
    let retry_after_ms = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1000));
    let latency_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    if !status.is_success() {
        let status_code = status.as_u16();
        let category = match status_code {
            408 | 504 => "timeout",
            429 => "rate_limit",
            500 => "internal_server_error",
            502 | 503 => "provider_unavailable",
            401 => "authentication_failure",
            403 => "permission_denied",
            400 => "invalid_request",
            _ => "unknown_provider_error",
        };
        return Ok(ProviderResponse {
            raw: json!({"error":category,"status":status_code,"retry_after_ms":retry_after_ms}),
            latency_ms,
            cost_usd: None,
            retry_after_ms,
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PROVIDER_RESPONSE_BYTES)
    {
        return Ok(ProviderResponse {
            raw: json!({"error":"invalid_provider_response"}),
            latency_ms,
            cost_usd: None,
            retry_after_ms,
        });
    }
    let mut bytes = Vec::new();
    let read_result = response
        .take(MAX_PROVIDER_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes);
    if read_result.is_err() || bytes.len() as u64 > MAX_PROVIDER_RESPONSE_BYTES {
        return Ok(ProviderResponse {
            raw: json!({"error":"invalid_provider_response"}),
            latency_ms,
            cost_usd: None,
            retry_after_ms,
        });
    }
    let payload: Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => {
            return Ok(ProviderResponse {
                raw: json!({"error":"invalid_provider_response"}),
                latency_ms,
                cost_usd: None,
                retry_after_ms,
            });
        }
    };
    let message = payload.pointer("/choices/0/message");
    let Some(message) = message else {
        return Ok(ProviderResponse {
            raw: json!({"error":"invalid_provider_response"}),
            latency_ms,
            cost_usd: None,
            retry_after_ms,
        });
    };
    let text = message.get("content").and_then(Value::as_str).unwrap_or("");
    let content =
        serde_json::from_str::<Value>(text).unwrap_or_else(|_| Value::String(text.to_owned()));
    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| calls.iter().map(normalize_tool_call).collect::<Vec<_>>())
        .unwrap_or_default();
    let usage=payload.get("usage").map(|usage|json!({"input_tokens":usage.get("prompt_tokens"),"output_tokens":usage.get("completion_tokens")}));
    let normalized = json!({"provider":"openai_compatible","model":payload.get("model").and_then(Value::as_str).unwrap_or(model),"content":content,"tool_calls":tool_calls,"finish_reason":payload.pointer("/choices/0/finish_reason"),"usage":usage});
    Ok(ProviderResponse {
        raw: json!({"response":normalized,"usage":usage,"status":status.as_u16()}),
        latency_ms,
        cost_usd: None,
        retry_after_ms,
    })
}

pub fn call_ollama(
    bridge: &mut crate::python_bridge::PythonBridge,
    config: &ProviderConfig,
    case: &Case,
    timeout_ms: u64,
) -> ProviderResponse {
    let model = config.model.as_deref().unwrap_or("<missing-model>");
    let messages: Vec<Value> = case
        .messages
        .iter()
        .map(|message| json!({"role":message.role,"content":message.content}))
        .collect();
    let tools: Vec<Value> = case.tools.iter().map(openai_tool).collect();
    let format = case
        .output_schema
        .as_ref()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let effective_timeout = config
        .timeout_ms
        .unwrap_or(timeout_ms)
        .min(timeout_ms)
        .max(1);
    let mut request = json!({"id":Uuid::new_v4().to_string(),"provider":"ollama","model":model,"base_url":config.base_url.as_deref().unwrap_or("http://127.0.0.1:11434"),"timeout_ms":effective_timeout,"messages":messages,"tools":tools,"format":format,"options":{"temperature":config.temperature,"top_p":config.top_p,"num_predict":config.max_output_tokens}});
    if request["tools"].as_array().is_some_and(Vec::is_empty) {
        request.as_object_mut().unwrap().remove("tools");
    }
    let started = Instant::now();
    let result = bridge.request(request);
    let latency = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let response = match result {
        Ok(response) => response,
        Err(error) => {
            let category = if error.to_string().contains("timed out") {
                "timeout"
            } else {
                "unknown_provider_error"
            };
            return ProviderResponse {
                raw: json!({"error":category}),
                latency_ms: latency,
                cost_usd: None,
                retry_after_ms: None,
            };
        }
    };
    if response.get("error").is_some() {
        let category = response
            .get("error_classification")
            .and_then(Value::as_str)
            .unwrap_or("unknown_provider_error");
        return ProviderResponse {
            raw: json!({"error":category}),
            latency_ms: response
                .get("latency_ms")
                .and_then(Value::as_u64)
                .unwrap_or(latency),
            cost_usd: None,
            retry_after_ms: None,
        };
    }
    let text = response
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("");
    let content =
        serde_json::from_str::<Value>(text).unwrap_or_else(|_| Value::String(text.to_owned()));
    let tool_calls = response
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| calls.iter().map(normalize_tool_call).collect::<Vec<_>>())
        .unwrap_or_default();
    let normalized = json!({"provider":"ollama","model":response.get("model").and_then(Value::as_str).unwrap_or(model),"content":content,"tool_calls":tool_calls,"finish_reason":response.get("finish_reason"),"usage":response.get("usage")});
    ProviderResponse {
        raw: json!({"response":normalized,"usage":response.get("usage")}),
        latency_ms: response
            .get("latency_ms")
            .and_then(Value::as_u64)
            .unwrap_or(latency),
        cost_usd: None,
        retry_after_ms: None,
    }
}

fn openai_tool(tool: &ToolDefinition) -> Value {
    json!({"type":"function","function":{"name":tool.name,"description":tool.description,"parameters":tool.parameters}})
}
fn normalize_tool_call(call: &Value) -> Value {
    let function = call.get("function").unwrap_or(call);
    let arguments = function.get("arguments").cloned().unwrap_or(Value::Null);
    let arguments = match arguments {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        value => value,
    };
    json!({"id":call.get("id"),"name":function.get("name"),"arguments":arguments})
}

pub fn validate_endpoint(endpoint: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(endpoint).context("base_url must be an absolute URL")?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        bail!("base_url must use http/https and must not embed credentials or fragments")
    }
    let local = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if parsed.scheme() == "http" && !local {
        bail!("remote provider endpoints must use HTTPS; plain HTTP is allowed only on loopback")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_policy_allows_local_http_and_remote_https_only() {
        assert!(validate_endpoint("http://127.0.0.1:8000/v1").is_ok());
        assert!(validate_endpoint("https://api.example.test/v1").is_ok());
        assert!(validate_endpoint("http://api.example.test/v1").is_err());
        assert!(validate_endpoint("https://user:value@example.test/v1").is_err());
    }
    #[test]
    fn tool_call_arguments_are_normalized() {
        let call = json!({"id":"call-1","function":{"name":"get_customer","arguments":"{\"customer_id\":\"x\"}"}});
        let normalized = normalize_tool_call(&call);
        assert_eq!(normalized["name"], "get_customer");
        assert_eq!(normalized["arguments"]["customer_id"], "x");
    }

    fn mock_http_server(status: &str, response_body: &str, retry_after: Option<&str>) -> String {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let response_body = response_body.to_owned();
        let status = status.to_owned();
        let retry_after = retry_after.map(str::to_owned);
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = vec![0; 8192];
            let _ = stream.read(&mut request);
            let retry_header = retry_after
                .map(|value| format!("Retry-After: {value}\r\n"))
                .unwrap_or_default();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{retry_header}\r\n{response_body}",
                response_body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        format!("http://{address}/v1")
    }
    #[test]
    fn openai_compatible_response_is_normalized_without_a_remote_service() {
        let base = mock_http_server(
            "200 OK",
            r#"{"model":"fixture-model","choices":[{"finish_reason":"stop","message":{"content":"{\"ok\":true}"}}],"usage":{"prompt_tokens":3,"completion_tokens":2}}"#,
            None,
        );
        let config = ProviderConfig {
            kind: "openai_compatible".into(),
            fixture: None,
            base_url: Some(base),
            model: Some("fixture-model".into()),
            api_key_env: None,
            timeout_ms: Some(1000),
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            capabilities: None,
        };
        let case = Case {
            id: "case".into(),
            description: String::new(),
            messages: vec![crate::Message {
                role: "user".into(),
                content: "hello".into(),
            }],
            output_schema: None,
            assertions: vec![],
            tools: vec![],
            requires_capabilities: vec![],
        };
        let response =
            call_openai_compatible(&build_client(&config, 1000).unwrap(), &config, &case, 1000)
                .unwrap();
        assert_eq!(
            response.raw.pointer("/response/content/ok"),
            Some(&Value::Bool(true))
        );
        assert_eq!(response.raw.pointer("/usage/input_tokens"), Some(&json!(3)));
    }
    #[test]
    fn rate_limit_response_preserves_status_and_retry_after() {
        let base = mock_http_server(
            "429 Too Many Requests",
            r#"{"error":{"message":"private diagnostics"}}"#,
            Some("2"),
        );
        let config = ProviderConfig {
            kind: "openai_compatible".into(),
            fixture: None,
            base_url: Some(base),
            model: Some("fixture-model".into()),
            api_key_env: None,
            timeout_ms: Some(1000),
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            capabilities: None,
        };
        let case = Case {
            id: "case".into(),
            description: String::new(),
            messages: vec![],
            output_schema: None,
            assertions: vec![],
            tools: vec![],
            requires_capabilities: vec![],
        };
        let response =
            call_openai_compatible(&build_client(&config, 1000).unwrap(), &config, &case, 1000)
                .unwrap();
        assert_eq!(response.raw["error"], "rate_limit");
        assert_eq!(response.raw["status"], 429);
        assert_eq!(response.retry_after_ms, Some(2000));
        assert!(!response.raw.to_string().contains("private diagnostics"));
    }
}
