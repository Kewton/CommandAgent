use crate::safety::path_guard::PathGuard;
use crate::tools::glob::{SearchOptions, walk_files};
use crate::tools::{ToolError, ToolResult};
use crate::util::file_classify::is_likely_text_path;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepMatch {
    pub path: PathBuf,
    pub line_number: usize,
    pub line: String,
}

pub struct GrepTool<'a> {
    guard: &'a PathGuard,
}

impl<'a> GrepTool<'a> {
    pub fn new(guard: &'a PathGuard) -> Self {
        Self { guard }
    }

    pub fn grep(&self, needle: &str, options: SearchOptions) -> ToolResult<Vec<GrepMatch>> {
        let mut matches = Vec::new();
        walk_files(
            self.guard.root(),
            self.guard.root(),
            options,
            &mut |relative| {
                if !is_likely_text_path(relative) {
                    return Ok(matches.len() < options.limit);
                }

                let full_path = self.guard.root().join(relative);
                let content = fs::read_to_string(&full_path).map_err(|err| ToolError::Io {
                    path: full_path,
                    message: err.to_string(),
                })?;
                for (idx, line) in content.lines().enumerate() {
                    if line.contains(needle) {
                        matches.push(GrepMatch {
                            path: relative.to_path_buf(),
                            line_number: idx + 1,
                            line: line.to_string(),
                        });
                        if matches.len() >= options.limit {
                            return Ok(false);
                        }
                    }
                }

                Ok(true)
            },
        )?;
        Ok(matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_text_matches() {
        let root = temp_workspace("grep");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/main.rs"),
            "fn main() {}\nlet target = true;\n",
        )
        .unwrap();
        let guard = PathGuard::new(&root).unwrap();

        let matches = GrepTool::new(&guard)
            .grep("target", SearchOptions::default())
            .unwrap();

        assert_eq!(
            matches,
            vec![GrepMatch {
                path: PathBuf::from("src/main.rs"),
                line_number: 2,
                line: "let target = true;".to_string(),
            }]
        );
    }

    #[test]
    fn skips_hidden_files_by_default() {
        let root = temp_workspace("hidden");
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join(".hidden/file.txt"), "target").unwrap();
        let guard = PathGuard::new(&root).unwrap();

        let matches = GrepTool::new(&guard)
            .grep("target", SearchOptions::default())
            .unwrap();

        assert!(matches.is_empty());
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("commandagent-grep-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
