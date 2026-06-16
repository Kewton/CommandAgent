use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PathGuard {
    root: PathBuf,
}

impl PathGuard {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, PathGuardError> {
        let root = fs::canonicalize(root.as_ref()).map_err(|err| PathGuardError::Io {
            path: root.as_ref().to_path_buf(),
            message: err.to_string(),
        })?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve(&self, user_path: impl AsRef<Path>) -> Result<PathBuf, PathGuardError> {
        let user_path = user_path.as_ref();
        if user_path.as_os_str().is_empty() {
            return Err(PathGuardError::EmptyPath);
        }
        if has_parent_traversal(user_path) {
            return Err(PathGuardError::ParentTraversal {
                path: user_path.to_path_buf(),
            });
        }

        let candidate = if user_path.is_absolute() {
            user_path.to_path_buf()
        } else {
            self.root.join(user_path)
        };

        if candidate.exists() {
            let resolved = fs::canonicalize(&candidate).map_err(|err| PathGuardError::Io {
                path: candidate.clone(),
                message: err.to_string(),
            })?;
            if !resolved.starts_with(&self.root) {
                return Err(PathGuardError::OutsideWorkspace {
                    path: candidate,
                    root: self.root.clone(),
                });
            }
            return Ok(resolved);
        }

        let existing_parent = existing_parent(&candidate);
        let canonical_parent =
            fs::canonicalize(&existing_parent).map_err(|err| PathGuardError::Io {
                path: existing_parent.clone(),
                message: err.to_string(),
            })?;

        if !canonical_parent.starts_with(&self.root) {
            return Err(PathGuardError::OutsideWorkspace {
                path: candidate,
                root: self.root.clone(),
            });
        }

        let tail = candidate
            .strip_prefix(&existing_parent)
            .unwrap_or(Path::new(""));
        Ok(canonical_parent.join(tail))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathGuardError {
    EmptyPath,
    ParentTraversal { path: PathBuf },
    OutsideWorkspace { path: PathBuf, root: PathBuf },
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for PathGuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "empty path is not allowed"),
            Self::ParentTraversal { path } => {
                write!(f, "parent traversal is not allowed: {}", path.display())
            }
            Self::OutsideWorkspace { path, root } => write!(
                f,
                "path escapes workspace: {} is outside {}",
                path.display(),
                root.display()
            ),
            Self::Io { path, message } => write!(f, "{}: {}", path.display(), message),
        }
    }
}

impl std::error::Error for PathGuardError {}

fn has_parent_traversal(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn existing_parent(path: &Path) -> PathBuf {
    let mut cursor = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };

    while !cursor.exists() {
        if !cursor.pop() {
            break;
        }
    }

    cursor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_path_inside_workspace() {
        let root = temp_workspace("inside");
        let guard = PathGuard::new(&root).unwrap();

        let resolved = guard.resolve("src/main.rs").unwrap();

        assert_eq!(
            resolved,
            fs::canonicalize(&root).unwrap().join("src/main.rs")
        );
    }

    #[test]
    fn rejects_parent_traversal() {
        let root = temp_workspace("parent");
        let guard = PathGuard::new(&root).unwrap();

        let err = guard.resolve("../outside.txt").unwrap_err();

        assert!(matches!(err, PathGuardError::ParentTraversal { .. }));
    }

    #[test]
    fn rejects_absolute_path_outside_workspace() {
        let root = temp_workspace("absolute");
        let guard = PathGuard::new(&root).unwrap();

        let err = guard.resolve(std::env::temp_dir()).unwrap_err();

        assert!(matches!(err, PathGuardError::OutsideWorkspace { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = temp_workspace("symlink-root");
        let outside = temp_workspace("symlink-outside");
        symlink(&outside, root.join("link")).unwrap();
        let guard = PathGuard::new(&root).unwrap();

        let err = guard.resolve("link/file.txt").unwrap_err();

        assert!(matches!(err, PathGuardError::OutsideWorkspace { .. }));
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("commandagent-path-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("src")).unwrap();
        path
    }
}
