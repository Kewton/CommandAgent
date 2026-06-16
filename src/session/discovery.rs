use crate::session::store::SessionError;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEntry {
    pub id: String,
    pub path: PathBuf,
}

pub fn list_session_entries(root: &Path) -> Result<Vec<SessionEntry>, SessionError> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(root).map_err(|err| SessionError::Io {
        path: root.to_path_buf(),
        message: err.to_string(),
    })? {
        let entry = entry.map_err(|err| SessionError::Io {
            path: root.to_path_buf(),
            message: err.to_string(),
        })?;
        let path = entry.path();
        if !path.join("session.json").is_file() {
            continue;
        }
        let Some(id) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        entries.push(SessionEntry {
            id: id.to_string(),
            path,
        });
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_session_dirs() {
        let root = temp_workspace("discovery");
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a/session.json"), "{}").unwrap();
        fs::create_dir_all(root.join("b")).unwrap();

        let entries = list_session_entries(&root).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "a");
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-session-discovery-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
