use super::{NormalizedVerifyCommand, VerifyCommandOracleRepair, VerifyCommandViolationKind};

const FALLBACK_ECHO_REASON: &str =
    "fallback_echo_stripped: trailing echo fallback masks the base command exit status";
const STDERR_SUPPRESSION_REASON: &str = "stderr_suppression_stripped: trailing 2>/dev/null suppresses verifier diagnostics; stderr is captured by the verifier";

pub(super) fn normalize_shared(command: &str) -> anyhow::Result<NormalizedVerifyCommand> {
    let diagnosis = super::diagnose_verify_command(command);
    if let Some(violation) = diagnosis.violation {
        if matches!(
            violation,
            VerifyCommandViolationKind::GrepDashPattern
                | VerifyCommandViolationKind::PackageJsonScriptGrep
                | VerifyCommandViolationKind::HookAttributeGrep
                | VerifyCommandViolationKind::SourceImplementationGrep
                | VerifyCommandViolationKind::OutputPipeStripped
                | VerifyCommandViolationKind::StderrMergeStripped
                | VerifyCommandViolationKind::ExitCodeEchoStripped
                | VerifyCommandViolationKind::FallbackTrueStripped
                | VerifyCommandViolationKind::FallbackEchoStripped
                | VerifyCommandViolationKind::StderrSuppressionStripped
                | VerifyCommandViolationKind::SuccessFailureEchoStripped
                | VerifyCommandViolationKind::WorkspaceCdNormalized
        ) {
            return Ok(NormalizedVerifyCommand::new(diagnosis.normalized));
        }
        anyhow::bail!(
            "{}",
            diagnosis
                .reason
                .unwrap_or_else(|| violation.message().to_string())
        );
    }
    Ok(NormalizedVerifyCommand::new(diagnosis.normalized))
}

pub(super) fn normalize(command: &str) -> Option<VerifyCommandOracleRepair> {
    if let Some(normalized) = strip_fallback_echo(command) {
        return Some(repair(
            normalized,
            FALLBACK_ECHO_REASON,
            "fallback_echo_stripped",
        ));
    }
    strip_stderr_suppression(command).map(|normalized| {
        repair(
            normalized,
            STDERR_SUPPRESSION_REASON,
            "stderr_suppression_stripped",
        )
    })
}

pub(super) fn violation_kind(kind: &str) -> Option<VerifyCommandViolationKind> {
    match kind {
        "fallback_echo_stripped" => Some(VerifyCommandViolationKind::FallbackEchoStripped),
        "stderr_suppression_stripped" => {
            Some(VerifyCommandViolationKind::StderrSuppressionStripped)
        }
        _ => None,
    }
}

fn repair(
    normalized: String,
    reason: &'static str,
    kind: &'static str,
) -> VerifyCommandOracleRepair {
    VerifyCommandOracleRepair {
        normalized,
        reason: reason.to_string(),
        kind,
    }
}

pub(super) fn strip_fallback_echo(command: &str) -> Option<String> {
    let trimmed = command.trim();
    let (base, fallback) = super::split_once_outside_quotes_sequence(trimmed, "||")?;
    if !super::is_plain_echo_command(fallback.trim()) {
        return None;
    }
    normalized_base(base)
}

pub(super) fn strip_stderr_suppression(command: &str) -> Option<String> {
    let trimmed = command.trim();
    let base = trailing_stderr_suppression_base(trimmed)?;
    normalized_base(base)
}

fn normalized_base(base: &str) -> Option<String> {
    let base = trailing_stderr_suppression_base(base)
        .unwrap_or(base)
        .trim();
    if base.is_empty() {
        return None;
    }
    Some(base.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn trailing_stderr_suppression_base(command: &str) -> Option<&str> {
    let trimmed = command.trim_end();
    let prefix = trimmed.strip_suffix("2>/dev/null")?;
    if prefix
        .chars()
        .next_back()
        .is_some_and(|ch| !ch.is_whitespace())
    {
        return None;
    }
    Some(prefix.trim_end())
}

#[cfg(test)]
mod tests {
    use super::super::{
        RuntimeCommandConnector, normalize_planner_verify_command,
        normalize_runtime_bash_command_for_boundary,
    };

    #[test]
    fn stderr_suppression_is_removed_in_declared_and_runtime_verify() {
        let command = "test -f output/inspection.json 2>/dev/null";
        assert_eq!(
            normalize_planner_verify_command(command).unwrap(),
            ["test -f output/inspection.json"]
        );
        let root = tempfile::tempdir().unwrap();
        let runtime = normalize_runtime_bash_command_for_boundary(command, root.path()).unwrap();
        assert_eq!(runtime.normalization_kind, "stderr_suppression_stripped");
        assert_eq!(runtime.normalized_command, "test -f output/inspection.json");
    }

    #[test]
    fn measured_echo_fallbacks_are_reduced_to_the_base_command() {
        assert_eq!(
            super::strip_fallback_echo("false || echo output directory missing").as_deref(),
            Some("false")
        );
        let fixture = std::fs::read_to_string(
            "tests/corpus/apps/test0715_data7_verify_rewrite/fixtures/runtime-rejections.jsonl",
        )
        .unwrap();
        let commands = fixture
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter_map(|event| event["command_summary"].as_str().map(str::to_string))
            .filter(|command| command.contains("|| echo"))
            .collect::<Vec<_>>();
        assert_eq!(commands.len(), 2);
        for command in &commands {
            let rewritten = super::strip_fallback_echo(command).unwrap();
            assert!(!rewritten.contains("2>/dev/null"));
            assert!(!rewritten.contains("|| echo"));
        }
        let command = commands
            .iter()
            .find(|command| !command.contains("<user>"))
            .unwrap();
        let declared = normalize_planner_verify_command(command).unwrap();
        assert_eq!(declared.len(), 1);
        {
            let root = tempfile::tempdir().unwrap();
            let runtime =
                normalize_runtime_bash_command_for_boundary(command, root.path()).unwrap();
            assert_eq!(runtime.normalization_kind, "fallback_echo_stripped");
            assert_eq!(runtime.normalized_command, declared[0]);
        }
    }

    #[test]
    fn measured_cd_and_pipe_rejection_reaches_runtime_split() {
        let fixture = std::fs::read_to_string(
            "tests/corpus/apps/test0715_data7_verify_rewrite/fixtures/runtime-rejections.jsonl",
        )
        .unwrap();
        let command = fixture
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter_map(|event| event["command_summary"].as_str().map(str::to_string))
            .find(|command| command.contains("wc -l data/sales.csv"))
            .unwrap();
        let (_, measured_suffix) = super::super::split_once_outside_quotes_sequence(&command, "&&")
            .expect("measured command has a workspace cd prefix");
        let root = tempfile::tempdir().unwrap();
        let command = format!("cd {} && {measured_suffix}", root.path().display());
        let runtime = normalize_runtime_bash_command_for_boundary(&command, root.path()).unwrap();
        assert_eq!(runtime.normalization_kind, "shell_control_split");
        assert_eq!(runtime.segments.len(), 3);
        assert_eq!(
            runtime.segments[1].connector,
            RuntimeCommandConnector::AndThen
        );
        assert!(!runtime.normalized_command.contains("head -30"));
    }

    #[test]
    fn stderr_stripping_exposes_and_split_in_both_verify_paths() {
        let command = "npm run build && test -f package.json 2>/dev/null";
        assert_eq!(
            normalize_planner_verify_command(command).unwrap(),
            ["npm run build", "test -f package.json"]
        );
        let root = tempfile::tempdir().unwrap();
        let runtime = normalize_runtime_bash_command_for_boundary(command, root.path()).unwrap();
        assert_eq!(runtime.normalization_kind, "shell_control_split");
        assert_eq!(runtime.segments.len(), 2);
        assert_eq!(
            runtime.segments[1].connector,
            RuntimeCommandConnector::AndThen
        );
    }

    #[test]
    fn file_writing_redirects_remain_rejected() {
        let command = "python3 pipeline/main.py > output/results.json";
        assert!(normalize_planner_verify_command(command).is_err());
        let root = tempfile::tempdir().unwrap();
        assert!(normalize_runtime_bash_command_for_boundary(command, root.path()).is_err());
    }
}
