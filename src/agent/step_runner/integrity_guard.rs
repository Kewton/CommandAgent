//! Deterministic patch-integrity guards for repair attempts.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatchValidationOutcome {
    Accepted,
    Noop,
    Duplicate,
    TestWeakening,
    GeneratedTestUnsupportedAssertion,
    UnsupportedContractAssertion,
    SelfReferentialVerifier,
    WorsenedVerifier,
}

impl PatchValidationOutcome {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Noop => "noop",
            Self::Duplicate => "duplicate",
            Self::TestWeakening => "test_weakening",
            Self::GeneratedTestUnsupportedAssertion => "generated_test_unsupported_assertion",
            Self::UnsupportedContractAssertion => "unsupported_contract_assertion",
            Self::SelfReferentialVerifier => "self_referential_verifier",
            Self::WorsenedVerifier => "worsened_verifier",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchValidation {
    pub(crate) outcome: PatchValidationOutcome,
    pub(crate) path: Option<String>,
    pub(crate) reason: String,
}

impl PatchValidation {
    pub(crate) fn render_line(&self) -> String {
        format!(
            "outcome={} path={} reason={}",
            self.outcome.as_str(),
            self.path.as_deref().unwrap_or("none"),
            self.reason.split_whitespace().collect::<Vec<_>>().join(" ")
        )
    }
}

pub(crate) fn detect_test_weakening(path: &str, content: &str) -> Option<PatchValidation> {
    let test_like =
        path.contains("test") || path.ends_with("_test.rs") || path.ends_with(".spec.ts");
    if !test_like {
        return None;
    }
    let lowered = content.to_ascii_lowercase();
    if lowered.contains("assert!(true)")
        || lowered.contains("it.skip")
        || lowered.contains("describe.skip")
        || lowered.contains("#[ignore]")
    {
        return Some(PatchValidation {
            outcome: PatchValidationOutcome::TestWeakening,
            path: Some(path.to_string()),
            reason: "repair attempted to weaken or skip a test".to_string(),
        });
    }
    None
}

pub(crate) fn detect_generated_test_assertion_outside_contract(
    path: &str,
    content: &str,
    task_contract_terms: &[&str],
) -> Option<PatchValidation> {
    if !is_test_like(path) {
        return None;
    }
    let lowered = content.to_ascii_lowercase();
    let generated = lowered.contains("generated")
        || lowered.contains("commandagent")
        || lowered.contains("auto-generated");
    if !generated || task_contract_terms.is_empty() {
        return None;
    }
    let has_contract_term = task_contract_terms
        .iter()
        .any(|term| lowered.contains(&term.to_ascii_lowercase()));
    if has_contract_term {
        return None;
    }
    Some(PatchValidation {
        outcome: PatchValidationOutcome::GeneratedTestUnsupportedAssertion,
        path: Some(path.to_string()),
        reason: "generated test assertion is not anchored to the task contract".to_string(),
    })
}

pub(crate) fn detect_unsupported_contract_assertion(
    path: &str,
    content: &str,
    supported_terms: &[&str],
) -> Option<PatchValidation> {
    if !is_test_like(path) || supported_terms.is_empty() {
        return None;
    }
    let lowered = content.to_ascii_lowercase();
    let asserts = lowered.contains("assert")
        || lowered.contains("expect(")
        || lowered.contains("should")
        || lowered.contains("pytest");
    if !asserts {
        return None;
    }
    let supported = supported_terms
        .iter()
        .any(|term| lowered.contains(&term.to_ascii_lowercase()));
    if supported {
        return None;
    }
    Some(PatchValidation {
        outcome: PatchValidationOutcome::UnsupportedContractAssertion,
        path: Some(path.to_string()),
        reason: "test assertion is outside the supported task contract terms".to_string(),
    })
}

pub(crate) fn detect_self_referential_verifier(
    verifier_command: &str,
    verifier_artifact_path: &str,
) -> Option<PatchValidation> {
    let command = verifier_command.trim();
    let artifact = verifier_artifact_path.trim();
    if artifact.is_empty() {
        return None;
    }
    if command.contains(artifact) {
        return Some(PatchValidation {
            outcome: PatchValidationOutcome::SelfReferentialVerifier,
            path: Some(artifact.to_string()),
            reason: "verifier command depends on the verifier artifact it is meant to validate"
                .to_string(),
        });
    }
    None
}

fn is_test_like(path: &str) -> bool {
    path.contains("test")
        || path.ends_with("_test.rs")
        || path.ends_with(".spec.ts")
        || path.ends_with(".test.ts")
        || path.ends_with(".spec.tsx")
        || path.ends_with(".test.tsx")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_test_weakening_markers() {
        let validation = detect_test_weakening("tests/app_test.rs", "#[ignore]\nfn test() {}")
            .expect("test weakening should be detected");

        assert_eq!(validation.outcome, PatchValidationOutcome::TestWeakening);
        assert!(validation.render_line().contains("test_weakening"));
    }

    #[test]
    fn generated_test_cannot_assert_behavior_outside_task_contract() {
        let validation = detect_generated_test_assertion_outside_contract(
            "tests/test_app.py",
            "# generated by CommandAgent\nassert dashboard.total == 10",
            &["login"],
        )
        .expect("generated assertion outside task contract should be detected");

        assert_eq!(
            validation.outcome,
            PatchValidationOutcome::GeneratedTestUnsupportedAssertion
        );
    }

    #[test]
    fn unsupported_contract_assertion_is_filtered() {
        let validation = detect_unsupported_contract_assertion(
            "tests/test_app.py",
            "def test_dashboard():\n    assert dashboard.total == 10",
            &["login"],
        )
        .expect("unsupported assertion should be detected");

        assert_eq!(
            validation.outcome,
            PatchValidationOutcome::UnsupportedContractAssertion
        );
    }

    #[test]
    fn detects_self_referential_verifier_commands() {
        let validation = detect_self_referential_verifier(
            "python scripts/verify.py scripts/verify.py",
            "scripts/verify.py",
        )
        .expect("self-referential verifier should be detected");

        assert_eq!(
            validation.outcome,
            PatchValidationOutcome::SelfReferentialVerifier
        );
    }
}
