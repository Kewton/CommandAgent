//! Deterministic setup-artifact validation before setup recovery.
//!
//! The validator only checks local setup artifacts. It does not install
//! dependencies, infer package versions, or rewrite manifests.

use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetupArtifactViolation {
    pub(crate) path: String,
    pub(crate) reason_code: String,
    pub(crate) diagnostic: String,
}

pub(crate) fn validate_npm_manifest(cwd: &Path) -> Option<SetupArtifactViolation> {
    let path = cwd.join("package.json");
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Some(SetupArtifactViolation {
                path: "package.json".to_string(),
                reason_code: "setup_manifest_missing".to_string(),
                diagnostic: "package.json is missing before dependency setup".to_string(),
            });
        }
        Err(err) => {
            return Some(SetupArtifactViolation {
                path: "package.json".to_string(),
                reason_code: "setup_manifest_unreadable".to_string(),
                diagnostic: format!("package.json could not be read: {err}"),
            });
        }
    };
    validate_package_json_content(&content)
}

#[allow(dead_code)]
pub(crate) fn validate_rust_manifest(cwd: &Path) -> Option<SetupArtifactViolation> {
    let path = cwd.join("Cargo.toml");
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Some(SetupArtifactViolation {
                path: "Cargo.toml".to_string(),
                reason_code: "setup_manifest_missing".to_string(),
                diagnostic: "Cargo.toml is missing before Rust build/test setup".to_string(),
            });
        }
        Err(err) => {
            return Some(SetupArtifactViolation {
                path: "Cargo.toml".to_string(),
                reason_code: "setup_manifest_unreadable".to_string(),
                diagnostic: format!("Cargo.toml could not be read: {err}"),
            });
        }
    };
    validate_cargo_toml_content(&content)
}

#[allow(dead_code)]
pub(crate) fn validate_python_manifest(cwd: &Path) -> Option<SetupArtifactViolation> {
    let pyproject = cwd.join("pyproject.toml");
    let requirements = cwd.join("requirements.txt");
    if pyproject.exists() {
        return match fs::read_to_string(&pyproject) {
            Ok(content) => validate_pyproject_toml_content(&content),
            Err(err) => Some(SetupArtifactViolation {
                path: "pyproject.toml".to_string(),
                reason_code: "setup_manifest_unreadable".to_string(),
                diagnostic: format!("pyproject.toml could not be read: {err}"),
            }),
        };
    }
    if requirements.exists() {
        return match fs::read_to_string(&requirements) {
            Ok(content) => validate_requirements_content(&content),
            Err(err) => Some(SetupArtifactViolation {
                path: "requirements.txt".to_string(),
                reason_code: "setup_manifest_unreadable".to_string(),
                diagnostic: format!("requirements.txt could not be read: {err}"),
            }),
        };
    }
    Some(SetupArtifactViolation {
        path: "pyproject.toml|requirements.txt".to_string(),
        reason_code: "setup_manifest_missing".to_string(),
        diagnostic: "Python setup manifest is missing before dependency setup".to_string(),
    })
}

fn validate_package_json_content(content: &str) -> Option<SetupArtifactViolation> {
    if content.trim().is_empty() {
        return Some(SetupArtifactViolation {
            path: "package.json".to_string(),
            reason_code: "setup_manifest_empty".to_string(),
            diagnostic: "package.json is empty before dependency setup".to_string(),
        });
    }
    let parsed: Value = match serde_json::from_str(content) {
        Ok(value) => value,
        Err(err) => {
            return Some(SetupArtifactViolation {
                path: "package.json".to_string(),
                reason_code: "setup_manifest_invalid_json".to_string(),
                diagnostic: format!("package.json is invalid JSON: {err}"),
            });
        }
    };
    let Some(object) = parsed.as_object() else {
        return Some(SetupArtifactViolation {
            path: "package.json".to_string(),
            reason_code: "setup_manifest_not_object".to_string(),
            diagnostic: "package.json must be a JSON object".to_string(),
        });
    };
    for field in ["scripts", "dependencies", "devDependencies"] {
        if let Some(value) = object.get(field)
            && !value.is_object()
        {
            return Some(SetupArtifactViolation {
                path: "package.json".to_string(),
                reason_code: format!("setup_manifest_{field}_not_object"),
                diagnostic: format!("package.json field `{field}` must be an object when present"),
            });
        }
    }
    None
}

#[allow(dead_code)]
fn validate_cargo_toml_content(content: &str) -> Option<SetupArtifactViolation> {
    if content.trim().is_empty() {
        return Some(SetupArtifactViolation {
            path: "Cargo.toml".to_string(),
            reason_code: "setup_manifest_empty".to_string(),
            diagnostic: "Cargo.toml is empty before Rust build/test setup".to_string(),
        });
    }
    if !content.lines().any(|line| {
        let line = line.trim();
        line == "[package]" || line == "[workspace]"
    }) {
        return Some(SetupArtifactViolation {
            path: "Cargo.toml".to_string(),
            reason_code: "setup_manifest_invalid_cargo_toml".to_string(),
            diagnostic: "Cargo.toml must contain [package] or [workspace]".to_string(),
        });
    }
    None
}

#[allow(dead_code)]
fn validate_pyproject_toml_content(content: &str) -> Option<SetupArtifactViolation> {
    if content.trim().is_empty() {
        return Some(SetupArtifactViolation {
            path: "pyproject.toml".to_string(),
            reason_code: "setup_manifest_empty".to_string(),
            diagnostic: "pyproject.toml is empty before Python dependency setup".to_string(),
        });
    }
    if !content.lines().any(|line| {
        let line = line.trim();
        line == "[project]" || line == "[build-system]" || line.starts_with("[tool.")
    }) {
        return Some(SetupArtifactViolation {
            path: "pyproject.toml".to_string(),
            reason_code: "setup_manifest_invalid_pyproject_toml".to_string(),
            diagnostic:
                "pyproject.toml must contain [project], [build-system], or [tool.*] section"
                    .to_string(),
        });
    }
    None
}

#[allow(dead_code)]
fn validate_requirements_content(content: &str) -> Option<SetupArtifactViolation> {
    if content
        .lines()
        .map(str::trim)
        .all(|line| line.is_empty() || line.starts_with('#'))
    {
        return Some(SetupArtifactViolation {
            path: "requirements.txt".to_string(),
            reason_code: "setup_manifest_empty".to_string(),
            diagnostic: "requirements.txt has no dependency entries".to_string(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn valid_package_json_passes_setup_artifact_validation() {
        assert!(
            validate_package_json_content(
                r#"{"scripts":{"build":"next build"},"dependencies":{"next":"14.2.0"}}"#
            )
            .is_none()
        );
    }

    #[test]
    fn invalid_package_json_is_manifest_repair_evidence() {
        let violation = validate_package_json_content(r#"{"scripts":"next build"}"#).unwrap();

        assert_eq!(violation.path, "package.json");
        assert_eq!(violation.reason_code, "setup_manifest_scripts_not_object");
        assert!(violation.diagnostic.contains("scripts"));
    }

    #[test]
    fn missing_package_json_is_setup_artifact_violation() {
        let root = temp_workspace("missing-package-json");
        let violation = validate_npm_manifest(&root).unwrap();

        assert_eq!(violation.path, "package.json");
        assert_eq!(violation.reason_code, "setup_manifest_missing");
    }

    #[test]
    fn rust_manifest_requires_package_or_workspace_section() {
        assert!(validate_cargo_toml_content("[package]\nname = \"x\"").is_none());

        let violation = validate_cargo_toml_content("[dependencies]\nserde = \"1\"").unwrap();

        assert_eq!(violation.path, "Cargo.toml");
        assert_eq!(violation.reason_code, "setup_manifest_invalid_cargo_toml");
    }

    #[test]
    fn python_manifest_accepts_pyproject_or_requirements() {
        let root = temp_workspace("python-manifest");
        fs::write(root.join("pyproject.toml"), "[project]\nname = \"x\"").unwrap();

        assert!(validate_python_manifest(&root).is_none());
    }

    #[test]
    fn python_missing_manifest_is_setup_artifact_violation() {
        let root = temp_workspace("missing-python-manifest");
        let violation = validate_python_manifest(&root).unwrap();

        assert_eq!(violation.path, "pyproject.toml|requirements.txt");
        assert_eq!(violation.reason_code, "setup_manifest_missing");
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("commandagent-setup-validation-{name}-{nanos}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }
}
