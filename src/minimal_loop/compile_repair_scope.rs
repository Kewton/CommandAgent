use std::path::{Path, PathBuf};

use crate::minimal_loop::build_verifier::CompileError;

pub const FOREIGN_PROJECT_COMPILE_ERROR: &str = "foreign_project_compile_error";

pub fn is_foreign(root: Option<&Path>, error: &CompileError) -> bool {
    error.route_bound == Some(false)
        && root.is_some_and(|root| belongs_to_nested_project(root, &error.path))
}

pub fn only_foreign(root: Option<&Path>, errors: &[CompileError]) -> bool {
    !errors.is_empty() && errors.iter().all(|error| is_foreign(root, error))
}

pub fn repairable(root: Option<&Path>, errors: &[CompileError]) -> Vec<CompileError> {
    errors
        .iter()
        .filter(|error| !is_foreign(root, error))
        .cloned()
        .collect()
}

fn belongs_to_nested_project(root: &Path, error_path: &str) -> bool {
    if !root.join("package.json").is_file() {
        return false;
    }
    let path = Path::new(error_path);
    let relative = if path.is_absolute() {
        let Ok(relative) = path.strip_prefix(root) else {
            return true;
        };
        relative
    } else {
        path
    };
    let mut ancestor = relative.parent().map(Path::to_path_buf);
    while let Some(candidate) = ancestor {
        if candidate.as_os_str().is_empty() {
            break;
        }
        if root.join(&candidate).join("package.json").is_file() {
            return true;
        }
        ancestor = candidate.parent().map(PathBuf::from);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(path: &str, route_bound: Option<bool>) -> CompileError {
        CompileError {
            path: path.to_string(),
            line: 1,
            column: 1,
            message: "Type error".to_string(),
            excerpt: String::new(),
            symbol: None,
            route_bound,
        }
    }

    #[test]
    fn only_explicitly_unbound_nested_project_errors_are_foreign() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("package.json"), "{}").unwrap();
        std::fs::create_dir_all(root.path().join("gui/components")).unwrap();
        std::fs::write(root.path().join("gui/package.json"), "{}").unwrap();

        assert!(only_foreign(
            Some(root.path()),
            &[error("gui/components/unused.tsx", Some(false))]
        ));
        assert!(!only_foreign(
            Some(root.path()),
            &[error("src/components/unused.tsx", Some(false))]
        ));
        assert!(!only_foreign(
            Some(root.path()),
            &[error("gui/components/unused.tsx", Some(true))]
        ));
        assert!(!only_foreign(
            Some(root.path()),
            &[error("gui/components/unused.tsx", None)]
        ));
        assert_eq!(
            repairable(
                Some(root.path()),
                &[
                    error("gui/components/unused.tsx", Some(false)),
                    error("src/components/unused.tsx", Some(false)),
                ]
            )
            .len(),
            1
        );
    }
}
