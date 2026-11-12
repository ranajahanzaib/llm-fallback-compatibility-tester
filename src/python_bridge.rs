use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};
use uuid::Uuid;

const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;

pub struct PythonBridge {
    child: Child,
    input: ChildStdin,
    output: Receiver<String>,
    timeout: Duration,
}
impl PythonBridge {
    pub fn start(timeout: Duration) -> Result<Self> {
        let mut child = Command::new("python3")
            .args(["-m", "fallbackcontract_adapters.protocol"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("start Python adapter; install python/fallbackcontract_adapters first")?;
        let input = child
            .stdin
            .take()
            .context("Python adapter stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("Python adapter stdout unavailable")?;
        let (sender, output) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match (&mut reader)
                    .take(MAX_LINE_BYTES as u64 + 1)
                    .read_line(&mut line)
                {
                    Ok(0) => break,
                    Ok(_) if line.len() <= MAX_LINE_BYTES => {
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {
                        let _ = sender.send(String::from(
                            "{\"error\":\"adapter response exceeded size limit\"}\n",
                        ));
                        break;
                    }
                    Err(_) => break,
                }
            }
        });
        let mut bridge = Self {
            child,
            input,
            output,
            timeout,
        };
        let hello = bridge.request(json!({"id":Uuid::new_v4().to_string(),"type":"hello"}))?;
        if hello.get("protocol_version").and_then(Value::as_u64) != Some(1) {
            bail!("Python adapter protocol version mismatch")
        }
        Ok(bridge)
    }
    pub fn request(&mut self, request: Value) -> Result<Value> {
        let id = request
            .get("id")
            .and_then(Value::as_str)
            .context("Python protocol request needs id")?
            .to_owned();
        let mut line = serde_json::to_vec(&request)?;
        line.push(b'\n');
        self.input
            .write_all(&line)
            .context("write Python adapter request")?;
        self.input.flush()?;
        let effective_timeout = request
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .map(Duration::from_millis)
            .unwrap_or(self.timeout);
        let response = match self.output.recv_timeout(effective_timeout) {
            Ok(response) => response,
            Err(error) => {
                let _ = self.child.kill();
                bail!("Python adapter response timed out or process exited: {error}")
            }
        };
        let response: Value = match serde_json::from_str(&response) {
            Ok(response) => response,
            Err(_) => {
                let _ = self.child.kill();
                bail!("Python adapter returned malformed JSONL")
            }
        };
        if response.get("id").and_then(Value::as_str) != Some(id.as_str()) {
            let _ = self.child.kill();
            bail!("Python adapter correlation ID mismatch")
        }
        Ok(response)
    }
}
impl Drop for PythonBridge {
    fn drop(&mut self) {
        let id = Uuid::new_v4().to_string();
        let mut line = serde_json::to_vec(&json!({"id":id,"type":"shutdown"})).unwrap_or_default();
        line.push(b'\n');
        let _ = self.input.write_all(&line);
        let _ = self.input.flush();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn line_limit_is_bounded() {
        assert_eq!(super::MAX_LINE_BYTES, 8 * 1024 * 1024);
    }
    #[test]
    fn subprocess_handshake_correlation_and_shutdown() {
        let available = std::process::Command::new("python3")
            .args(["-c", "import fallbackcontract_adapters.protocol"])
            .status()
            .is_ok_and(|status| status.success());
        if !available {
            return;
        }
        let mut bridge = super::PythonBridge::start(std::time::Duration::from_secs(2)).unwrap();
        let response = bridge
            .request(serde_json::json!({"id":"test-1","provider":"unknown"}))
            .unwrap();
        assert_eq!(response["id"], "test-1");
        assert!(response.get("error_classification").is_some());
    }
}
