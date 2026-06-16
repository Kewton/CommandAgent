use crate::safety::path_guard::PathGuard;
use crate::tools::{ToolError, ToolResult};
use crate::util::file_classify::is_likely_text_path;
use std::fs;
use std::path::Path;

pub struct EditTool<'a> {
    guard: &'a PathGuard,
}

impl<'a> EditTool<'a> {
    pub fn new(guard: &'a PathGuard) -> Self {
        Self { guard }
    }

    pub fn replace_once(&self, path: impl AsRef<Path>, old: &str, new: &str) -> ToolResult<()> {
        let resolved = self.guard.resolve(path)?;
        if !is_likely_text_path(&resolved) {
            return Err(ToolError::BinaryFile { path: resolved });
        }

        let content = fs::read_to_string(&resolved).map_err(|err| ToolError::Io {
            path: resolved.clone(),
            message: err.to_string(),
        })?;
        let count = content.matches(old).count();
        if count == 0 {
            return Err(ToolError::EditMatchNotFound);
        }
        if count > 1 {
            return Err(ToolError::EditMatchAmbiguous { count });
        }

        let edited = content.replacen(old, new, 1);
        fs::write(&resolved, edited).map_err(|err| ToolError::Io {
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
    fn replaces_single_match() {
        let root = temp_workspace("single");
        fs::write(root.join("file.txt"), "hello old").unwrap();
        let guard = PathGuard::new(&root).unwrap();

        EditTool::new(&guard)
            .replace_once("file.txt", "old", "new")
            .unwrap();

        assert_eq!(
            fs::read_to_string(root.join("file.txt")).unwrap(),
            "hello new"
        );
    }

    #[test]
    fn rejects_ambiguous_match() {
        let root = temp_workspace("ambiguous");
        fs::write(root.join("file.txt"), "old old").unwrap();
        let guard = PathGuard::new(&root).unwrap();

        let err = EditTool::new(&guard)
            .replace_once("file.txt", "old", "new")
            .unwrap_err();

        assert_eq!(err, ToolError::EditMatchAmbiguous { count: 2 });
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("commandagent-edit-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
