use crate::safety::path_guard::PathGuard;
use crate::tools::{ToolError, ToolResult};
use std::fs;
use std::path::Path;

pub struct WriteTool<'a> {
    guard: &'a PathGuard,
}

impl<'a> WriteTool<'a> {
    pub fn new(guard: &'a PathGuard) -> Self {
        Self { guard }
    }

    pub fn write(&self, path: impl AsRef<Path>, content: &str) -> ToolResult<()> {
        let resolved = self.guard.resolve(path)?;
        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent).map_err(|err| ToolError::Io {
                path: parent.to_path_buf(),
                message: err.to_string(),
            })?;
        }
        fs::write(&resolved, content).map_err(|err| ToolError::Io {
            path: resolved,
            message: err.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::path_guard::PathGuard;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn creates_parent_directories() {
        let root = temp_workspace("parents");
        let guard = PathGuard::new(&root).unwrap();

        WriteTool::new(&guard)
            .write("nested/deep/file.txt", "created")
            .unwrap();

        assert_eq!(
            fs::read_to_string(root.join("nested/deep/file.txt")).unwrap(),
            "created"
        );
    }

    #[test]
    fn rejects_path_escape() {
        let root = temp_workspace("escape");
        let guard = PathGuard::new(&root).unwrap();

        let err = WriteTool::new(&guard)
            .write("../outside.txt", "nope")
            .unwrap_err();

        assert!(matches!(err, ToolError::Path(_)));
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-write-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
