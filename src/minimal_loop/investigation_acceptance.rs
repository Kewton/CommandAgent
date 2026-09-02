use std::path::Path;

use crate::planner::adjudication::investigate::{
    InvestigationAssurance, InvestigationBindingEvidence, InvestigationRunEvidence,
    evaluate_investigation_evidence,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InvestigationAcceptance {
    pub(crate) fully_bound: bool,
    pub(crate) reason: String,
}

pub(crate) fn evaluate_workspace(root: &Path) -> InvestigationAcceptance {
    let diagnosis_written = root.join("output/diagnosis.md").is_file();
    let run = match read_json::<InvestigationRunEvidence>(
        &root.join("evidence/investigation-run.json"),
    ) {
        Ok(value) => value,
        Err(reason) => return rejected(reason),
    };
    let binding = match read_json::<InvestigationBindingEvidence>(
        &root.join("evidence/investigation-binding.json"),
    ) {
        Ok(value) => value,
        Err(reason) => return rejected(reason),
    };
    let adjudication =
        evaluate_investigation_evidence(diagnosis_written, run.as_ref(), binding.as_ref());
    InvestigationAcceptance {
        fully_bound: adjudication.assurance == InvestigationAssurance::Full,
        reason: if adjudication.assurance == InvestigationAssurance::Full {
            "full".to_string()
        } else {
            adjudication.reason
        },
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|_| "investigation_evidence_unreadable".to_string())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "investigation_evidence_invalid_json".to_string())
}

fn rejected(reason: impl Into<String>) -> InvestigationAcceptance {
    InvestigationAcceptance {
        fully_bound: false,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimal_loop::evidence::verify_runtime_acceptance;
    use crate::planner::adjudication::contract::ProbeOutcome;
    use crate::planner::adjudication::investigate::{
        DiagnosisClaim, DiagnosisClaimKind, InvestigationBindingEvidence, InvestigationRunEvidence,
        write_investigation_evidence,
    };

    #[test]
    fn requires_a_full_bound_investigation() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            evaluate_workspace(root.path()),
            InvestigationAcceptance {
                fully_bound: false,
                reason: "diagnosis_not_written".to_string(),
            }
        );

        std::fs::create_dir_all(root.path().join("output")).unwrap();
        std::fs::write(
            root.path().join("output/diagnosis.md"),
            "The failure is bound to src/main.rs:1.\n",
        )
        .unwrap();
        let mut run = InvestigationRunEvidence::new(
            "python3 tools/probe.py fixture.json",
            1,
            ProbeOutcome::Failure,
        );
        run.executed = true;
        let binding = InvestigationBindingEvidence::new(vec![DiagnosisClaim {
            kind: DiagnosisClaimKind::FileLine,
            value: "src/main.rs:1".to_string(),
            subject_path: Some("src/main.rs".to_string()),
            line: Some(1),
            matched: true,
            nearest: None,
        }]);
        write_investigation_evidence(root.path(), &run, &binding).unwrap();

        assert_eq!(
            evaluate_workspace(root.path()),
            InvestigationAcceptance {
                fully_bound: true,
                reason: "full".to_string(),
            }
        );
    }

    #[test]
    fn rejects_an_unmatched_or_invalid_binding() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("output")).unwrap();
        std::fs::create_dir_all(root.path().join("evidence")).unwrap();
        std::fs::write(root.path().join("output/diagnosis.md"), "diagnosis\n").unwrap();
        std::fs::write(
            root.path().join("evidence/investigation-run.json"),
            "not json\n",
        )
        .unwrap();

        assert_eq!(
            evaluate_workspace(root.path()).reason,
            "investigation_evidence_invalid_json"
        );
    }

    #[test]
    fn obligation_rejects_a_diagnosis_without_bound_runtime_evidence() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("output")).unwrap();
        std::fs::write(root.path().join("output/diagnosis.md"), "diagnosis\n").unwrap();

        let report = verify_runtime_acceptance(
            root.path(),
            &[],
            &[],
            &["investigation_binding".to_string()],
            &["investigation_binding".to_string()],
            &["investigation".to_string()],
            &[],
        );

        assert!(!report.passed, "{report:?}");
        assert!(
            report
                .missing_evidence
                .contains(&"investigation_binding".to_string()),
            "{report:?}"
        );
        assert!(
            report
                .missing_obligations
                .contains(&"investigation".to_string()),
            "{report:?}"
        );
        assert_eq!(
            report.obligation_repair_targets[0].target_path,
            "output/diagnosis.md"
        );
    }

    #[test]
    fn obligation_accepts_only_full_existing_adjudication() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("output")).unwrap();
        std::fs::write(
            root.path().join("output/diagnosis.md"),
            "Bound to src/main.rs:1.\n",
        )
        .unwrap();
        let run = InvestigationRunEvidence::new("python3 repro.py", 1, ProbeOutcome::Failure);
        let binding = InvestigationBindingEvidence::new(vec![DiagnosisClaim {
            kind: DiagnosisClaimKind::FileLine,
            value: "src/main.rs:1".to_string(),
            subject_path: Some("src/main.rs".to_string()),
            line: Some(1),
            matched: true,
            nearest: None,
        }]);
        write_investigation_evidence(root.path(), &run, &binding).unwrap();

        let report = verify_runtime_acceptance(
            root.path(),
            &[],
            &[],
            &["investigation_binding".to_string()],
            &["investigation_binding".to_string()],
            &["investigation".to_string()],
            &[],
        );

        assert!(report.passed, "{report:?}");
        assert_eq!(
            report
                .evidence_tiers
                .get("investigation_binding")
                .map(String::as_str),
            Some("strong")
        );
    }
}
