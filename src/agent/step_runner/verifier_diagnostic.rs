//! Deterministic verifier diagnostic payloads.
//!
//! This module classifies already-observed verifier output. It does not run
//! tools, decide retries, or add provider/model-specific behavior.

use crate::agent::step_runner::correction_evidence::failure_signature;
use crate::agent::step_runner::verify::VerificationFailure;

const MAX_TEXT_CHARS: usize = 360;
const MAX_LIST_ITEMS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerifierDiagnosticCode {
    PythonImportMissing,
    PythonAssertionMismatch,
    FastapiResponseMismatch,
    RustCompileError,
    RustTestAssertionMismatch,
    TypescriptTypeError,
    NextjsRouteNotIntegrated,
    NextjsEventHandlerBoundary,
    DependencyMissing,
    CommandNotFound,
    PortInUse,
    GeneratedTestWeakness,
    SelfReferentialVerifier,
    WeakSourceGrep,
    BlockedCommandPolicy,
    UnknownVerifierFailure,
}

impl VerifierDiagnosticCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PythonImportMissing => "python_import_missing",
            Self::PythonAssertionMismatch => "python_assertion_mismatch",
            Self::FastapiResponseMismatch => "fastapi_response_mismatch",
            Self::RustCompileError => "rust_compile_error",
            Self::RustTestAssertionMismatch => "rust_test_assertion_mismatch",
            Self::TypescriptTypeError => "typescript_type_error",
            Self::NextjsRouteNotIntegrated => "nextjs_route_not_integrated",
            Self::NextjsEventHandlerBoundary => "nextjs_event_handler_boundary",
            Self::DependencyMissing => "dependency_missing",
            Self::CommandNotFound => "command_not_found",
            Self::PortInUse => "port_in_use",
            Self::GeneratedTestWeakness => "generated_test_weakness",
            Self::SelfReferentialVerifier => "self_referential_verifier",
            Self::WeakSourceGrep => "weak_source_grep",
            Self::BlockedCommandPolicy => "blocked_command_policy",
            Self::UnknownVerifierFailure => "unknown_verifier_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifierDiagnosticPayload {
    pub(crate) diagnostic_code: VerifierDiagnosticCode,
    pub(crate) failure_kind: String,
    pub(crate) failure_signature: String,
    pub(crate) command: String,
    pub(crate) exit_status: Option<String>,
    pub(crate) diagnostic_excerpt: String,
    pub(crate) source_excerpt: Option<String>,
    pub(crate) observed_expected_pairs: Vec<String>,
    pub(crate) affected_cases: Vec<String>,
    pub(crate) candidate_artifacts: Vec<String>,
    pub(crate) source_of_truth: String,
    pub(crate) preferred_repair_role: String,
    pub(crate) weak_verifier_reason: Option<String>,
    pub(crate) admitted_cluster_targets: Vec<String>,
    pub(crate) confidence: String,
}

impl VerifierDiagnosticPayload {
    pub(crate) fn from_failure(
        failure: &VerificationFailure,
        base_candidates: &[String],
        admitted_target: Option<&str>,
    ) -> Self {
        let text = combined_text(failure);
        let code = classify_diagnostic(failure, &text);
        let mut candidate_artifacts = base_candidates.to_vec();
        if let Some(source) = &failure.source_excerpt {
            push_unique(&mut candidate_artifacts, source.path.clone());
        }
        for path in source_like_paths(&text) {
            push_unique(&mut candidate_artifacts, path);
        }
        truncate_list(&mut candidate_artifacts);

        let mut admitted_cluster_targets = Vec::new();
        if let Some(target) = admitted_target {
            push_unique(&mut admitted_cluster_targets, target.to_string());
        }
        truncate_list(&mut admitted_cluster_targets);

        let observed_expected_pairs = observed_expected_pairs(failure, &text);
        let affected_cases = affected_cases(failure, &text);
        let weak_verifier_reason = weak_verifier_reason(failure, code);
        let source_excerpt = failure.source_excerpt.as_ref().map(|source| {
            compact(&format!(
                "{}:{} {}",
                source.path, source.line, source.excerpt
            ))
        });
        let source_of_truth = source_of_truth(code).to_string();
        let preferred_repair_role = preferred_repair_role(code).to_string();
        let failure_kind = failure_kind(code).to_string();
        let exit_status = failure
            .reason
            .strip_prefix("command_failed:")
            .map(str::to_string);
        let signature = failure_signature([
            "verifier_diagnostic",
            failure.command.as_str(),
            code.as_str(),
            admitted_target.unwrap_or(""),
            failure.reason.as_str(),
        ]);
        let confidence = match code {
            VerifierDiagnosticCode::UnknownVerifierFailure => "unknown_bounded",
            VerifierDiagnosticCode::WeakSourceGrep
            | VerifierDiagnosticCode::SelfReferentialVerifier
            | VerifierDiagnosticCode::GeneratedTestWeakness => "heuristic_bounded",
            _ => "deterministic",
        }
        .to_string();

        Self {
            diagnostic_code: code,
            failure_kind,
            failure_signature: signature,
            command: failure.command.clone(),
            exit_status,
            diagnostic_excerpt: bounded(&diagnostic_excerpt(failure, &text)),
            source_excerpt,
            observed_expected_pairs,
            affected_cases,
            candidate_artifacts,
            source_of_truth,
            preferred_repair_role,
            weak_verifier_reason,
            admitted_cluster_targets,
            confidence,
        }
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("diagnostic_code={}", self.diagnostic_code.as_str()),
            format!("failure_kind={}", compact(&self.failure_kind)),
            format!("failure_signature={}", compact(&self.failure_signature)),
            format!("command={}", compact(&self.command)),
            format!("source_of_truth={}", compact(&self.source_of_truth)),
            format!(
                "preferred_repair_role={}",
                compact(&self.preferred_repair_role)
            ),
            format!("confidence={}", compact(&self.confidence)),
        ];
        if let Some(status) = &self.exit_status {
            lines.push(format!("exit_status={}", compact(status)));
        }
        if !self.diagnostic_excerpt.trim().is_empty() {
            lines.push(format!(
                "diagnostic_excerpt={}",
                compact(&self.diagnostic_excerpt)
            ));
        }
        if let Some(source) = &self.source_excerpt {
            lines.push(format!("source_excerpt={}", compact(source)));
        }
        if !self.observed_expected_pairs.is_empty() {
            lines.push(format!(
                "observed_expected={}",
                self.observed_expected_pairs.join("|")
            ));
        }
        if !self.affected_cases.is_empty() {
            lines.push(format!("affected_cases={}", self.affected_cases.join("|")));
        }
        if !self.candidate_artifacts.is_empty() {
            lines.push(format!(
                "candidate_artifacts={}",
                self.candidate_artifacts.join("|")
            ));
        }
        if !self.admitted_cluster_targets.is_empty() {
            lines.push(format!(
                "admitted_cluster_targets={}",
                self.admitted_cluster_targets.join("|")
            ));
        }
        if let Some(reason) = &self.weak_verifier_reason {
            lines.push(format!("weak_verifier_reason={}", compact(reason)));
        }
        lines
    }

    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        let mut fields = vec![
            format!("diagnostic_code={}", self.diagnostic_code.as_str()),
            format!("diagnostic_failure_kind={}", compact(&self.failure_kind)),
            format!("source_of_truth={}", compact(&self.source_of_truth)),
            format!("failure_signature={}", compact(&self.failure_signature)),
            format!("command={}", compact(&self.command)),
            format!(
                "preferred_repair_role={}",
                compact(&self.preferred_repair_role)
            ),
            format!(
                "unknown_diagnostic_count={}",
                if self.diagnostic_code == VerifierDiagnosticCode::UnknownVerifierFailure {
                    1
                } else {
                    0
                }
            ),
        ];
        if !self.observed_expected_pairs.is_empty() {
            fields.push(format!(
                "observed_expected={}",
                report_list(&self.observed_expected_pairs)
            ));
        }
        if !self.affected_cases.is_empty() {
            fields.push(format!(
                "affected_cases={}",
                report_list(&self.affected_cases)
            ));
        }
        if !self.candidate_artifacts.is_empty() {
            fields.push(format!(
                "candidate_artifacts={}",
                report_list(&self.candidate_artifacts)
            ));
        }
        if let Some(reason) = &self.weak_verifier_reason {
            fields.push(format!("weak_verifier_reason={}", compact(reason)));
        }
        if !self.admitted_cluster_targets.is_empty() {
            fields.push(format!(
                "admitted_cluster_targets={}",
                self.admitted_cluster_targets.join("|")
            ));
        }
        fields
    }
}

fn classify_diagnostic(failure: &VerificationFailure, text: &str) -> VerifierDiagnosticCode {
    let lower = text.to_ascii_lowercase();
    let command = failure.command.trim().to_ascii_lowercase();
    if failure.reason == "dependency_missing" || lower.contains("dependency_missing") {
        return VerifierDiagnosticCode::DependencyMissing;
    }
    if failure.reason.starts_with("blocked:") {
        return VerifierDiagnosticCode::BlockedCommandPolicy;
    }
    if command_is_weak_source_grep(&failure.command) {
        return VerifierDiagnosticCode::WeakSourceGrep;
    }
    if command_is_self_referential_generated_test(&failure.command) {
        return VerifierDiagnosticCode::SelfReferentialVerifier;
    }
    if lower.contains("eaddrinuse") || lower.contains("address already in use") {
        return VerifierDiagnosticCode::PortInUse;
    }
    if lower.contains("command not found") || lower.contains("not found:") {
        return VerifierDiagnosticCode::CommandNotFound;
    }
    if lower.contains("cannot find module")
        || lower.contains("module not found: can't resolve")
        || lower.contains("module not found: cannot resolve")
    {
        return VerifierDiagnosticCode::DependencyMissing;
    }
    if lower.contains("modulenotfounderror") || lower.contains("no module named") {
        return VerifierDiagnosticCode::PythonImportMissing;
    }
    if lower.contains("event handlers cannot be passed to client component props") {
        return VerifierDiagnosticCode::NextjsEventHandlerBoundary;
    }
    if lower.contains("nextjs_route_not_integrated")
        || lower.contains("route_not_integrated")
        || lower.contains("not imported by selected route")
    {
        return VerifierDiagnosticCode::NextjsRouteNotIntegrated;
    }
    if lower.contains("type error") || lower.contains("typescript") && lower.contains("error") {
        return VerifierDiagnosticCode::TypescriptTypeError;
    }
    if command.contains("cargo") && (lower.contains("error[") || lower.contains("--> src/")) {
        return VerifierDiagnosticCode::RustCompileError;
    }
    if command.contains("cargo test") && lower.contains("assert") {
        return VerifierDiagnosticCode::RustTestAssertionMismatch;
    }
    if command.contains("pytest")
        && (lower.contains("response")
            || lower.contains("status_code")
            || lower.contains("fastapi")
            || lower.contains("testclient"))
        && (lower.contains("assert") || lower.contains("expected"))
    {
        return VerifierDiagnosticCode::FastapiResponseMismatch;
    }
    if lower.contains("assertionerror") || lower.contains("e assert") || lower.contains("assert ") {
        return VerifierDiagnosticCode::PythonAssertionMismatch;
    }
    if lower.contains("generated test") && lower.contains("weak") {
        return VerifierDiagnosticCode::GeneratedTestWeakness;
    }
    VerifierDiagnosticCode::UnknownVerifierFailure
}

fn failure_kind(code: VerifierDiagnosticCode) -> &'static str {
    match code {
        VerifierDiagnosticCode::PythonImportMissing => "import_missing",
        VerifierDiagnosticCode::PythonAssertionMismatch
        | VerifierDiagnosticCode::FastapiResponseMismatch
        | VerifierDiagnosticCode::RustTestAssertionMismatch => "assertion_mismatch",
        VerifierDiagnosticCode::RustCompileError
        | VerifierDiagnosticCode::TypescriptTypeError
        | VerifierDiagnosticCode::NextjsEventHandlerBoundary => "compile_or_type_error",
        VerifierDiagnosticCode::NextjsRouteNotIntegrated => "route_integration_failure",
        VerifierDiagnosticCode::DependencyMissing => "dependency_missing",
        VerifierDiagnosticCode::CommandNotFound
        | VerifierDiagnosticCode::BlockedCommandPolicy
        | VerifierDiagnosticCode::WeakSourceGrep
        | VerifierDiagnosticCode::SelfReferentialVerifier
        | VerifierDiagnosticCode::GeneratedTestWeakness => "verifier_contract_failure",
        VerifierDiagnosticCode::PortInUse => "dev_server_port_conflict",
        VerifierDiagnosticCode::UnknownVerifierFailure => "unknown_verifier_failure",
    }
}

fn source_of_truth(code: VerifierDiagnosticCode) -> &'static str {
    match code {
        VerifierDiagnosticCode::WeakSourceGrep
        | VerifierDiagnosticCode::SelfReferentialVerifier
        | VerifierDiagnosticCode::GeneratedTestWeakness
        | VerifierDiagnosticCode::BlockedCommandPolicy
        | VerifierDiagnosticCode::CommandNotFound => "verifier_contract",
        VerifierDiagnosticCode::DependencyMissing => "setup_manifest_and_dependency_diagnostic",
        VerifierDiagnosticCode::NextjsRouteNotIntegrated => "profile_contract",
        VerifierDiagnosticCode::PortInUse => "dev_server_contract",
        _ => "original_verifier_diagnostic",
    }
}

fn preferred_repair_role(code: VerifierDiagnosticCode) -> &'static str {
    match code {
        VerifierDiagnosticCode::DependencyMissing => "setup",
        VerifierDiagnosticCode::WeakSourceGrep
        | VerifierDiagnosticCode::SelfReferentialVerifier
        | VerifierDiagnosticCode::GeneratedTestWeakness
        | VerifierDiagnosticCode::BlockedCommandPolicy
        | VerifierDiagnosticCode::CommandNotFound => "verifier_contract",
        VerifierDiagnosticCode::NextjsRouteNotIntegrated => "route_integration",
        VerifierDiagnosticCode::PortInUse => "dev_server",
        VerifierDiagnosticCode::RustTestAssertionMismatch
        | VerifierDiagnosticCode::PythonAssertionMismatch => "implementation",
        _ => "implementation",
    }
}

fn weak_verifier_reason(
    failure: &VerificationFailure,
    code: VerifierDiagnosticCode,
) -> Option<String> {
    match code {
        VerifierDiagnosticCode::WeakSourceGrep => {
            Some("source_grep_verifies_text_not_behavior".to_string())
        }
        VerifierDiagnosticCode::SelfReferentialVerifier => {
            Some("verifier_only_checks_generated_test_artifact".to_string())
        }
        VerifierDiagnosticCode::GeneratedTestWeakness => {
            Some("generated_test_weakness".to_string())
        }
        VerifierDiagnosticCode::BlockedCommandPolicy => Some(compact(&failure.reason)),
        _ => None,
    }
}

fn observed_expected_pairs(failure: &VerificationFailure, text: &str) -> Vec<String> {
    let mut pairs = Vec::new();
    for marker in ["observed=", "expected="] {
        if text.contains(marker) {
            push_unique(&mut pairs, bounded(&compact(text)));
            return pairs;
        }
    }
    if let Some((left, right)) = text.split_once("!=") {
        push_unique(
            &mut pairs,
            format!("observed={} expected={}", tail(left), head(right)),
        );
    } else if text.to_ascii_lowercase().contains("assert") {
        push_unique(
            &mut pairs,
            format!(
                "observed={} expected=verifier passes",
                bounded(&compact(text))
            ),
        );
    } else {
        push_unique(
            &mut pairs,
            format!(
                "observed={} expected={} verifier passes",
                bounded(&diagnostic_excerpt(failure, text)),
                bounded(&failure.command)
            ),
        );
    }
    truncate_list(&mut pairs);
    pairs
}

fn affected_cases(failure: &VerificationFailure, text: &str) -> Vec<String> {
    let mut cases = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("FAILED ") {
            if let Some(case) = trimmed.split_whitespace().nth(1) {
                push_unique(&mut cases, clean_case(case));
            }
        } else if trimmed.contains("::test_") && !trimmed.contains(' ') {
            push_unique(&mut cases, clean_case(trimmed));
        }
    }
    if cases.is_empty() && !failure.command.trim().is_empty() {
        push_unique(&mut cases, failure.command.clone());
    }
    truncate_list(&mut cases);
    cases
}

fn diagnostic_excerpt(failure: &VerificationFailure, text: &str) -> String {
    if !failure.diagnostic_excerpt.trim().is_empty() {
        failure.diagnostic_excerpt.clone()
    } else if !failure.stderr_excerpt.trim().is_empty() {
        failure.stderr_excerpt.clone()
    } else if !failure.stdout_excerpt.trim().is_empty() {
        failure.stdout_excerpt.clone()
    } else {
        text.to_string()
    }
}

fn combined_text(failure: &VerificationFailure) -> String {
    [
        failure.reason.as_str(),
        failure.diagnostic_excerpt.as_str(),
        failure.stderr_excerpt.as_str(),
        failure.stdout_excerpt.as_str(),
    ]
    .join("\n")
}

fn source_like_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for raw in text.split_whitespace() {
        let value = raw
            .trim_matches(|ch: char| {
                matches!(
                    ch,
                    '.' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '\'' | '"' | '`'
                )
            })
            .trim_start_matches("./");
        if is_source_like_path(value) {
            push_unique(&mut paths, value.to_string());
        } else if let Some((path, _)) = value.split_once(':')
            && is_source_like_path(path)
        {
            push_unique(&mut paths, path.to_string());
        }
    }
    truncate_list(&mut paths);
    paths
}

fn is_source_like_path(value: &str) -> bool {
    !value.starts_with('/')
        && !value.starts_with("node_modules/")
        && !value.contains("/node_modules/")
        && !value.starts_with(".next/")
        && !value.contains("/.next/")
        && value.contains('/')
        && matches!(
            value.rsplit('.').next(),
            Some("ts" | "tsx" | "js" | "jsx" | "rs" | "py")
        )
}

pub(crate) fn command_is_weak_source_grep(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    if !(lower.starts_with("grep -q ") || lower.starts_with("rg -q ")) {
        return false;
    }
    command
        .split_whitespace()
        .last()
        .map(|path| {
            let path = path.trim_matches(|ch| ch == '\'' || ch == '"');
            matches!(
                path.rsplit('.').next(),
                Some("ts" | "tsx" | "js" | "jsx" | "rs" | "py")
            )
        })
        .unwrap_or(false)
}

fn command_is_self_referential_generated_test(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    (lower.starts_with("test -f ") || lower.starts_with("grep -q "))
        && (lower.contains("tests/") || lower.contains("_test.rs") || lower.contains("test_"))
}

fn clean_case(value: &str) -> String {
    value
        .trim_matches(|ch: char| matches!(ch, '\'' | '"' | ',' | ';' | ':'))
        .to_string()
}

fn tail(value: &str) -> String {
    let compacted = compact(value);
    compacted
        .split_whitespace()
        .last()
        .unwrap_or(compacted.as_str())
        .to_string()
}

fn head(value: &str) -> String {
    compact(value)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !values.contains(&value) {
        values.push(value);
    }
}

fn truncate_list(values: &mut Vec<String>) {
    values.truncate(MAX_LIST_ITEMS);
}

fn bounded(value: &str) -> String {
    let compacted = compact(value);
    compacted.chars().take(MAX_TEXT_CHARS).collect()
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn report_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| compact(value).replace(' ', "_"))
        .collect::<Vec<_>>()
        .join("|")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(command: &str, reason: &str, diagnostic: &str) -> VerificationFailure {
        VerificationFailure {
            command: command.to_string(),
            reason: reason.to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            diagnostic_excerpt: diagnostic.to_string(),
            source_excerpt: None,
        }
    }

    #[test]
    fn classifies_python_import_missing() {
        let payload = VerifierDiagnosticPayload::from_failure(
            &failure(
                "pytest tests/test_app.py",
                "command_failed:1",
                "ModuleNotFoundError: No module named 'app.main'",
            ),
            &[],
            None,
        );

        assert_eq!(
            payload.diagnostic_code,
            VerifierDiagnosticCode::PythonImportMissing
        );
        assert_eq!(payload.preferred_repair_role, "implementation");
        assert!(
            payload
                .render_lines()
                .iter()
                .any(|line| { line == "diagnostic_code=python_import_missing" })
        );
    }

    #[test]
    fn classifies_rust_compile_error() {
        let payload = VerifierDiagnosticPayload::from_failure(
            &failure(
                "cargo check",
                "command_failed:101",
                "error[E0425]: cannot find value `x` in this scope\n --> src/main.rs:3:5",
            ),
            &[],
            None,
        );

        assert_eq!(
            payload.diagnostic_code,
            VerifierDiagnosticCode::RustCompileError
        );
        assert!(
            payload
                .candidate_artifacts
                .contains(&"src/main.rs".to_string())
        );
    }

    #[test]
    fn classifies_weak_source_grep() {
        let payload = VerifierDiagnosticPayload::from_failure(
            &failure("grep -q CommandAgent src/main.rs", "command_failed:1", ""),
            &["src/main.rs".to_string()],
            Some("src/main.rs"),
        );

        assert_eq!(
            payload.diagnostic_code,
            VerifierDiagnosticCode::WeakSourceGrep
        );
        assert_eq!(payload.preferred_repair_role, "verifier_contract");
        assert_eq!(
            payload.weak_verifier_reason.as_deref(),
            Some("source_grep_verifies_text_not_behavior")
        );
    }

    #[test]
    fn unknown_failure_remains_classified() {
        let payload = VerifierDiagnosticPayload::from_failure(
            &failure("custom check", "command_failed:1", "unexpected failure"),
            &[],
            None,
        );

        assert_eq!(
            payload.diagnostic_code,
            VerifierDiagnosticCode::UnknownVerifierFailure
        );
        assert_eq!(payload.confidence, "unknown_bounded");
        assert!(!payload.failure_signature.is_empty());
    }

    #[test]
    fn eval_fields_include_semantic_diagnostic_payload() {
        let payload = VerifierDiagnosticPayload::from_failure(
            &failure(
                "cargo test",
                "command_failed:101",
                "thread 'tests::it_works' panicked at src/lib.rs:4:5: assertion `left == right` failed left: 1 right: 2",
            ),
            &["src/lib.rs".to_string()],
            Some("src/lib.rs"),
        );
        let fields = payload.eval_report_fields().join("\n");

        assert!(fields.contains("diagnostic_code=rust_test_assertion_mismatch"));
        assert!(fields.contains("diagnostic_failure_kind=assertion_mismatch"));
        assert!(fields.contains("source_of_truth=original_verifier_diagnostic"));
        assert!(fields.contains("preferred_repair_role=implementation"));
        assert!(fields.contains("candidate_artifacts=src/lib.rs"));
        assert!(fields.contains("unknown_diagnostic_count=0"));
    }

    #[test]
    fn classifies_common_command_not_found_output() {
        let payload = VerifierDiagnosticPayload::from_failure(
            &failure(
                "missing-tool --version",
                "command_failed:127",
                "zsh: command not found: missing-tool",
            ),
            &[],
            None,
        );

        assert_eq!(
            payload.diagnostic_code,
            VerifierDiagnosticCode::CommandNotFound
        );
        assert_eq!(payload.preferred_repair_role, "verifier_contract");
    }
}
