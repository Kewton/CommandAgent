use crate::safety::path_guard::PathGuard;
use crate::tools::{ToolError, ToolResult};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchOptions {
    pub include_hidden: bool,
    pub limit: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            limit: 200,
        }
    }
}

pub struct GlobTool<'a> {
    guard: &'a PathGuard,
}

impl<'a> GlobTool<'a> {
    pub fn new(guard: &'a PathGuard) -> Self {
        Self { guard }
    }

    pub fn glob(&self, pattern: &str, options: SearchOptions) -> ToolResult<Vec<PathBuf>> {
        let mut results = Vec::new();
        walk_files(
            self.guard.root(),
            self.guard.root(),
            options,
            &mut |relative| {
                let rel = relative.to_string_lossy().replace('\\', "/");
                if wildcard_match(pattern, &rel) {
                    results.push(relative.to_path_buf());
                }
                Ok(results.len() < options.limit)
            },
        )?;
        results.sort();
        Ok(results)
    }
}

pub(crate) fn walk_files<F>(
    root: &Path,
    dir: &Path,
    options: SearchOptions,
    visit: &mut F,
) -> ToolResult<()>
where
    F: FnMut(&Path) -> ToolResult<bool>,
{
    let mut entries = fs::read_dir(dir)
        .map_err(|err| ToolError::Io {
            path: dir.to_path_buf(),
            message: err.to_string(),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| ToolError::Io {
            path: dir.to_path_buf(),
            message: err.to_string(),
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !options.include_hidden && name.starts_with('.') {
            continue;
        }

        let file_type = entry.file_type().map_err(|err| ToolError::Io {
            path: path.clone(),
            message: err.to_string(),
        })?;
        if file_type.is_dir() {
            walk_files(root, &path, options, visit)?;
        } else if file_type.is_file() {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            if !visit(relative)? {
                return Ok(());
            }
        }
    }

    Ok(())
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern == text;
    }

    let mut remaining = text;
    if !pattern.starts_with('*') {
        let Some(first) = parts.first() else {
            return false;
        };
        if !remaining.starts_with(first) {
            return false;
        }
        remaining = &remaining[first.len()..];
    }

    for part in parts
        .iter()
        .skip(usize::from(!pattern.starts_with('*')))
        .filter(|part| !part.is_empty())
    {
        let Some(index) = remaining.find(part) else {
            return false;
        };
        remaining = &remaining[index + part.len()..];
    }

    if !pattern.ends_with('*') {
        let Some(last) = parts.last() else {
            return false;
        };
        return text.ends_with(last);
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_matching_files_and_skips_hidden_by_default() {
        let root = temp_workspace("glob");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        fs::write(root.join(".hidden/secret.rs"), "").unwrap();
        let guard = PathGuard::new(&root).unwrap();

        let matches = GlobTool::new(&guard)
            .glob("*.rs", SearchOptions::default())
            .unwrap();

        assert_eq!(matches, vec![PathBuf::from("src/main.rs")]);
    }

    #[test]
    fn respects_limit() {
        let root = temp_workspace("limit");
        fs::write(root.join("a.txt"), "").unwrap();
        fs::write(root.join("b.txt"), "").unwrap();
        let guard = PathGuard::new(&root).unwrap();

        let matches = GlobTool::new(&guard)
            .glob(
                "*.txt",
                SearchOptions {
                    include_hidden: false,
                    limit: 1,
                },
            )
            .unwrap();

        assert_eq!(matches.len(), 1);
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("commandagent-glob-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
