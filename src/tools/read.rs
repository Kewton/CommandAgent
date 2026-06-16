use crate::safety::path_guard::PathGuard;
use crate::tools::{ToolError, ToolResult};
use crate::util::file_classify::is_likely_text_path;
use std::fs;
use std::path::Path;

pub struct ReadTool<'a> {
    guard: &'a PathGuard,
}

impl<'a> ReadTool<'a> {
    pub fn new(guard: &'a PathGuard) -> Self {
        Self { guard }
    }

    pub fn read(&self, path: impl AsRef<Path>) -> ToolResult<String> {
        let resolved = self.guard.resolve(path)?;
        if !is_likely_text_path(&resolved) {
            return Err(ToolError::BinaryFile { path: resolved });
        }

        fs::read_to_string(&resolved).map_err(|err| ToolError::Io {
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
    fn reads_text_file_inside_workspace() {
        let root = temp_workspace("read");
        fs::write(root.join("README.md"), "hello").unwrap();
        let guard = PathGuard::new(&root).unwrap();

        let text = ReadTool::new(&guard).read("README.md").unwrap();

        assert_eq!(text, "hello");
    }

    #[test]
    fn rejects_binary_extension() {
        let root = temp_workspace("binary");
        fs::write(root.join("image.png"), [0, 1, 2]).unwrap();
        let guard = PathGuard::new(&root).unwrap();

        let err = ReadTool::new(&guard).read("image.png").unwrap_err();

        assert!(matches!(err, ToolError::BinaryFile { .. }));
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("commandagent-read-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
