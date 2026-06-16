use crate::providers::{ChatMessage, ChatRole};
use crate::safety::path_guard::{PathGuard, PathGuardError};
use crate::util::workspace_paths::sessions_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct SessionStore {
    root: PathBuf,
    guard: PathGuard,
}

impl SessionStore {
    pub fn new(cwd: impl AsRef<Path>) -> Result<Self, SessionError> {
        let root = sessions_dir(cwd.as_ref());
        fs::create_dir_all(&root).map_err(|err| SessionError::Io {
            path: root.clone(),
            message: err.to_string(),
        })?;
        let guard = PathGuard::new(cwd.as_ref()).map_err(SessionError::Path)?;
        Ok(Self { root, guard })
    }

    pub fn create(&self) -> SessionSnapshot {
        let now = now_ms();
        SessionSnapshot {
            id: new_session_id(now),
            created_at_ms: now,
            updated_at_ms: now,
            messages: Vec::new(),
        }
    }

    pub fn save(&self, snapshot: &mut SessionSnapshot) -> Result<(), SessionError> {
        snapshot.updated_at_ms = now_ms();
        let dir = self.session_dir(&snapshot.id)?;
        fs::create_dir_all(&dir).map_err(|err| SessionError::Io {
            path: dir.clone(),
            message: err.to_string(),
        })?;
        let path = dir.join("session.json");
        let body = serde_json::to_string_pretty(snapshot)
            .map_err(|err| SessionError::Json(err.to_string()))?;
        fs::write(&path, body).map_err(|err| SessionError::Io {
            path,
            message: err.to_string(),
        })
    }

    pub fn load(&self, id: &str) -> Result<SessionSnapshot, SessionError> {
        let path = self.session_dir(id)?.join("session.json");
        let body = fs::read_to_string(&path).map_err(|err| SessionError::Io {
            path: path.clone(),
            message: err.to_string(),
        })?;
        serde_json::from_str(&body).map_err(|err| SessionError::Json(err.to_string()))
    }

    pub fn session_dir(&self, id: &str) -> Result<PathBuf, SessionError> {
        if !is_safe_session_id(id) {
            return Err(SessionError::InvalidId(id.to_string()));
        }
        self.guard
            .resolve(Path::new(".commandagent").join("sessions").join(id))
            .map_err(SessionError::Path)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub id: String,
    pub created_at_ms: u128,
    pub updated_at_ms: u128,
    pub messages: Vec<SessionMessage>,
}

impl SessionSnapshot {
    pub fn push(&mut self, role: SessionRole, content: impl Into<String>) {
        self.messages.push(SessionMessage {
            role,
            content: content.into(),
            name: None,
        });
    }

    pub fn as_chat_messages(&self) -> Vec<ChatMessage> {
        self.messages
            .iter()
            .map(|message| ChatMessage {
                role: message.role.into(),
                content: message.content.clone(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionMessage {
    pub role: SessionRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionRole {
    System,
    User,
    Assistant,
    Tool,
}

impl From<SessionRole> for ChatRole {
    fn from(value: SessionRole) -> Self {
        match value {
            SessionRole::System => Self::System,
            SessionRole::User => Self::User,
            SessionRole::Assistant => Self::Assistant,
            SessionRole::Tool => Self::Tool,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    Path(PathGuardError),
    Io { path: PathBuf, message: String },
    Json(String),
    InvalidId(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(err) => write!(f, "{}", err),
            Self::Io { path, message } => write!(f, "{}: {}", path.display(), message),
            Self::Json(message) => write!(f, "session JSON error: {}", message),
            Self::InvalidId(id) => write!(f, "invalid session id: {}", id),
        }
    }
}

impl std::error::Error for SessionError {}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn new_session_id(now: u128) -> String {
    format!("{:x}-{:x}", now, std::process::id())
}

fn is_safe_session_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_session() {
        let root = temp_workspace("save-load");
        let store = SessionStore::new(&root).unwrap();
        let mut snapshot = store.create();
        snapshot.push(SessionRole::User, "hello");
        snapshot.push(SessionRole::Assistant, "hi");

        store.save(&mut snapshot).unwrap();
        let loaded = store.load(&snapshot.id).unwrap();

        assert_eq!(loaded.id, snapshot.id);
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].content, "hello");
    }

    #[test]
    fn rejects_path_like_session_id() {
        let root = temp_workspace("invalid-id");
        let store = SessionStore::new(&root).unwrap();

        let err = store.load("../escape").unwrap_err();

        assert!(matches!(err, SessionError::InvalidId(_)));
    }

    #[test]
    fn converts_to_chat_messages() {
        let snapshot = SessionSnapshot {
            id: "s".to_string(),
            created_at_ms: 1,
            updated_at_ms: 1,
            messages: vec![SessionMessage {
                role: SessionRole::Assistant,
                content: "done".to_string(),
                name: None,
            }],
        };

        let chat = snapshot.as_chat_messages();

        assert_eq!(chat[0].role, ChatRole::Assistant);
        assert_eq!(chat[0].content, "done");
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-session-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
