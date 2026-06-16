use crate::providers::ToolCallMode;
use crate::util::workspace_paths::state_dir;
use serde::Serialize;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct LlmIoLogger {
    path: PathBuf,
}

impl LlmIoLogger {
    pub fn new(cwd: impl AsRef<Path>) -> Result<Self, LogError> {
        let dir = state_dir(cwd.as_ref()).join("logs");
        fs::create_dir_all(&dir).map_err(|err| LogError::Io {
            path: dir.clone(),
            message: err.to_string(),
        })?;
        Ok(Self {
            path: dir.join("llm-io.jsonl"),
        })
    }

    pub fn append(&self, event: &LlmIoEvent) -> Result<(), LogError> {
        let mut event = event.clone();
        mask_secrets(&mut event.payload);
        let line = serde_json::to_string(&event).map_err(|err| LogError::Json(err.to_string()))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|err| LogError::Io {
                path: self.path.clone(),
                message: err.to_string(),
            })?;
        writeln!(file, "{}", line).map_err(|err| LogError::Io {
            path: self.path.clone(),
            message: err.to_string(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LlmIoEvent {
    pub ts_ms: u128,
    pub kind: LlmIoKind,
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planner_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planner_model: Option<String>,
    pub tool_call_mode: ToolCallModeForLog,
    pub payload: Value,
}

impl LlmIoEvent {
    pub fn new(
        kind: LlmIoKind,
        provider: impl Into<String>,
        model: impl Into<String>,
        tool_call_mode: ToolCallMode,
        payload: Value,
    ) -> Self {
        Self {
            ts_ms: now_ms(),
            kind,
            provider: provider.into(),
            model: model.into(),
            planner_provider: None,
            planner_model: None,
            tool_call_mode: tool_call_mode.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmIoKind {
    Request,
    Response,
    Parser,
    ToolCall,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallModeForLog {
    Native,
    XmlFallback,
}

impl From<ToolCallMode> for ToolCallModeForLog {
    fn from(value: ToolCallMode) -> Self {
        match value {
            ToolCallMode::Native => Self::Native,
            ToolCallMode::XmlFallback => Self::XmlFallback,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogError {
    Io { path: PathBuf, message: String },
    Json(String),
}

impl std::fmt::Display for LogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{}: {}", path.display(), message),
            Self::Json(message) => write!(f, "log JSON error: {}", message),
        }
    }
}

impl std::error::Error for LogError {}

pub fn mask_secrets(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                if is_secret_key(key) {
                    *value = Value::String("[redacted]".to_string());
                } else {
                    mask_secrets(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                mask_secrets(value);
            }
        }
        _ => {}
    }
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("api_key")
        || key.contains("authorization")
        || key == "token"
        || key.ends_with("_token")
        || key == "bearer"
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn masks_nested_secret_values() {
        let mut value = json!({
            "headers": {
                "authorization": "Bearer secret",
                "x": "safe"
            },
            "api_key": "secret",
            "items": [{"access_token": "secret"}]
        });

        mask_secrets(&mut value);

        assert_eq!(value["headers"]["authorization"], "[redacted]");
        assert_eq!(value["api_key"], "[redacted]");
        assert_eq!(value["items"][0]["access_token"], "[redacted]");
        assert_eq!(value["headers"]["x"], "safe");
    }

    #[test]
    fn writes_jsonl_log() {
        let cwd = temp_workspace("jsonl");
        let logger = LlmIoLogger::new(&cwd).unwrap();
        let event = LlmIoEvent::new(
            LlmIoKind::Request,
            "openai",
            "gpt-5.4-mini",
            ToolCallMode::XmlFallback,
            json!({"authorization":"Bearer secret","body":"ok"}),
        );

        logger.append(&event).unwrap();

        let body = fs::read_to_string(logger.path()).unwrap();
        assert!(body.contains("\"kind\":\"request\""));
        assert!(body.contains("[redacted]"));
        assert!(!body.contains("Bearer secret"));
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-llm-io-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
