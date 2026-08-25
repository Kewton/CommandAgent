//! Typed validation for executable Recovery Plan YAML.

use std::fmt;
use std::path::Path;

use super::ultra_plan::{UltraPlan, parse_ultra_plan, render_ultra_plan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryPlanValidationError {
    Missing,
    Unreadable,
    Parse,
    Roundtrip,
    NeedsReview,
}

impl RecoveryPlanValidationError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Missing => "recovery_yaml_missing",
            Self::Unreadable => "recovery_yaml_unreadable",
            Self::Parse => "recovery_yaml_parse_failed",
            Self::Roundtrip => "recovery_yaml_roundtrip_mismatch",
            Self::NeedsReview => "recovery_needs_review",
        }
    }
}

impl fmt::Display for RecoveryPlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

pub(crate) fn validate(path: &Path) -> Result<UltraPlan, RecoveryPlanValidationError> {
    if !path.is_file() {
        return Err(RecoveryPlanValidationError::Missing);
    }
    let text =
        std::fs::read_to_string(path).map_err(|_| RecoveryPlanValidationError::Unreadable)?;
    if needs_review(&text) {
        return Err(RecoveryPlanValidationError::NeedsReview);
    }
    let parsed = parse_ultra_plan(&text).map_err(|_| RecoveryPlanValidationError::Parse)?;
    let reparsed = parse_ultra_plan(&render_ultra_plan(&parsed))
        .map_err(|_| RecoveryPlanValidationError::Roundtrip)?;
    if reparsed != parsed {
        return Err(RecoveryPlanValidationError::Roundtrip);
    }
    Ok(parsed)
}

fn needs_review(text: &str) -> bool {
    text.lines()
        .any(|line| line.trim() == "recovery_needs_review: true")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_marker_is_a_typed_rejection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recovery.yaml");
        std::fs::write(
            &path,
            "recovery_needs_review: true\ngoal: x\nphases:\n  - id: x\n    prompt: x\n",
        )
        .unwrap();
        assert_eq!(
            validate(&path),
            Err(RecoveryPlanValidationError::NeedsReview)
        );
    }
}
