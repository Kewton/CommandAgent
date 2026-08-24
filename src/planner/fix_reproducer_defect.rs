use crate::planner::adjudication::fix::{FixFailureClassification, ProbeOutcome};
use crate::tools::bash::BashOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FailureAssessment {
    pub(crate) classification: FixFailureClassification,
    pub(crate) error_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BeforePhaseOutcome {
    Confirmed,
    RebuildRequired { feedback: String },
}

pub(crate) fn classify(
    command: &str,
    outcome: ProbeOutcome,
    observation: Option<&BashOutcome>,
) -> FailureAssessment {
    let Some(observation) = observation.filter(|_| outcome == ProbeOutcome::Failure) else {
        return subject_failure();
    };
    let output = format!("{}\n{}", observation.stderr, observation.stdout);
    let error_kind = inline_python_syntax_error(command, &output);
    if let Some(error_kind) = error_kind {
        FailureAssessment {
            classification: FixFailureClassification::ReproducerDefect,
            error_kind: Some(error_kind.to_string()),
        }
    } else {
        subject_failure()
    }
}

pub(crate) fn rebuild_feedback(error_kind: &str) -> String {
    format!("再現コマンド自体が壊れている（{error_kind}）。対象を実際に評価するRを再構築せよ。")
}

pub(crate) fn rebuild_prompt(base: &str, feedback: &str) -> String {
    format!(
        "{base}\n\nFix reproducer reconstruction feedback:\n- {feedback}\n- F1 is not yet confirmed. Return one replacement verify step without changing the workspace."
    )
}

fn subject_failure() -> FailureAssessment {
    FailureAssessment {
        classification: FixFailureClassification::SubjectFailure,
        error_kind: None,
    }
}

fn inline_python_syntax_error<'a>(command: &str, output: &'a str) -> Option<&'a str> {
    let mut words = command.split_whitespace();
    let executable = words.next()?.rsplit('/').next()?;
    if !executable.starts_with("python") || words.next()? != "-c" || !output.contains("<string>") {
        return None;
    }
    output.lines().rev().find_map(|line| {
        let kind = line.trim().split_once(':')?.0;
        matches!(kind, "SyntaxError" | "IndentationError" | "TabError").then_some(kind)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::bash::BashOutcomeKind;

    const RUN6_COMMAND: &str = r#"python -c "import json\nd=json.load(open('output/results.json'))\nassert 'reconciliation' in d and 'values' in d""#;
    const RUN6_STDERR: &str = r#"  File "<string>", line 1
    import json\nd=json.load(open('output/results.json'))\nassert 'reconciliation' in d and 'values' in d
                ^
SyntaxError: unexpected character after line continuation character
"#;

    fn failed(stderr: &str) -> BashOutcome {
        BashOutcome {
            kind: BashOutcomeKind::CommandFailed,
            status: Some("exit status: 1".to_string()),
            stdout: String::new(),
            stderr: stderr.to_string(),
            elapsed_ms: 1,
            summary: "command failed".to_string(),
        }
    }

    #[test]
    fn run6_inline_syntax_error_is_reproducer_defect() {
        let assessment = classify(
            RUN6_COMMAND,
            ProbeOutcome::Failure,
            Some(&failed(RUN6_STDERR)),
        );

        assert_eq!(
            assessment.classification,
            FixFailureClassification::ReproducerDefect
        );
        assert_eq!(assessment.error_kind.as_deref(), Some("SyntaxError"));
        assert_eq!(
            rebuild_feedback("SyntaxError"),
            "再現コマンド自体が壊れている（SyntaxError）。対象を実際に評価するRを再構築せよ。"
        );
    }

    #[test]
    fn target_script_syntax_error_remains_subject_failure() {
        let stderr = "  File \"pipeline/main.py\", line 2\nSyntaxError: invalid syntax\n";
        let assessment = classify(
            "python3 -B pipeline/main.py",
            ProbeOutcome::Failure,
            Some(&failed(stderr)),
        );

        assert_eq!(
            assessment.classification,
            FixFailureClassification::SubjectFailure
        );
    }

    #[test]
    fn feedback_prompt_keeps_f1_unconfirmed_boundary_explicit() {
        let prompt = rebuild_prompt("base", &rebuild_feedback("SyntaxError"));

        assert!(prompt.contains("F1 is not yet confirmed"));
        assert!(prompt.contains("replacement verify step"));
        assert!(prompt.contains("without changing the workspace"));
    }
}
