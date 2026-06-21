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
