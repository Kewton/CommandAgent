//! Structured facts from deterministic guards.
//!
//! Evidence is rendered into existing bounded correction or repair paths. It
//! must not carry retry state, target authority, semantic guesses, sidecar
//! results, memory references, or provider policy.

const MAX_FIELD_CHARS: usize = 240;
const MAX_LIST_ITEMS: usize = 8;

pub type PlanCorrectionEvidence = ContractEvidence;

/// Bounded data produced by a deterministic contract guard.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContractEvidence {
    pub guard: String,
    pub failed_step: Option<String>,
    pub violated_contract: Option<String>,
    pub reason_code: Option<String>,
    pub failure_signature: Option<String>,
    pub failure_kind: Option<String>,
    pub diagnostic_code: Option<String>,
    pub command: Option<String>,
    pub tool: Option<String>,
    pub target_field: Option<String>,
    pub target_path: Option<String>,
    pub candidate_artifacts: Vec<String>,
    pub repair_target: Option<String>,
    pub required_fields: Vec<String>,
    pub required_literals: Vec<String>,
    pub missing_literals: Vec<String>,
    pub required_paths: Vec<String>,
    pub missing_paths: Vec<String>,
    pub affected_cases: Vec<String>,
    pub observed_expected_pairs: Vec<String>,
    pub rejected_value: Option<String>,
    pub required_action: Option<String>,
    pub related_source_excerpt: Option<String>,
    pub prior_attempts: Vec<String>,
    pub repair_attempt_ledger: Vec<String>,
    pub repair_focus: Option<String>,
    pub diagnostic: Option<String>,
}

impl ContractEvidence {
    pub fn new(guard: impl Into<String>) -> Self {
        Self {
            guard: guard.into(),
            ..Self::default()
        }
    }

    pub fn with_failed_step(mut self, failed_step: impl Into<String>) -> Self {
        self.failed_step = Some(failed_step.into());
        self
    }

    pub fn with_violated_contract(mut self, violated_contract: impl Into<String>) -> Self {
        self.violated_contract = Some(violated_contract.into());
        self
    }

    pub fn with_reason_code(mut self, reason_code: impl Into<String>) -> Self {
        self.reason_code = Some(reason_code.into());
        self
    }

    pub fn with_failure_signature(mut self, failure_signature: impl Into<String>) -> Self {
        self.failure_signature = Some(failure_signature.into());
        self
    }

    pub fn with_failure_kind(mut self, failure_kind: impl Into<String>) -> Self {
        self.failure_kind = Some(failure_kind.into());
        self
    }

    pub fn with_diagnostic_code(mut self, diagnostic_code: impl Into<String>) -> Self {
        self.diagnostic_code = Some(diagnostic_code.into());
        self
    }

    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    pub fn with_target_field(mut self, target_field: impl Into<String>) -> Self {
        self.target_field = Some(target_field.into());
        self
    }

    pub fn with_target_path(mut self, target_path: impl Into<String>) -> Self {
        self.target_path = Some(target_path.into());
        self
    }

    pub fn with_candidate_artifacts<I, S>(mut self, candidate_artifacts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.candidate_artifacts = collect_values(candidate_artifacts);
        self
    }

    pub fn with_repair_target(mut self, repair_target: impl Into<String>) -> Self {
        self.repair_target = Some(repair_target.into());
        self
    }

    pub fn with_required_fields<I, S>(mut self, required_fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_fields = collect_values(required_fields);
        self
    }

    pub fn with_required_literals<I, S>(mut self, required_literals: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_literals = collect_values(required_literals);
        self
    }

    pub fn with_missing_literals<I, S>(mut self, missing_literals: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.missing_literals = collect_values(missing_literals);
        self
    }

    pub fn with_required_paths<I, S>(mut self, required_paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_paths = collect_values(required_paths);
        self
    }

    pub fn with_missing_paths<I, S>(mut self, missing_paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.missing_paths = collect_values(missing_paths);
        self
    }

    pub fn with_affected_cases<I, S>(mut self, affected_cases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.affected_cases = collect_values(affected_cases);
        self
    }

    pub fn with_observed_expected_pairs<I, S>(mut self, observed_expected_pairs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.observed_expected_pairs = collect_values(observed_expected_pairs);
        self
    }

    pub fn with_rejected_value(mut self, rejected_value: impl Into<String>) -> Self {
        self.rejected_value = Some(rejected_value.into());
        self
    }

    pub fn with_required_action(mut self, required_action: impl Into<String>) -> Self {
        self.required_action = Some(required_action.into());
        self
    }

    pub fn with_related_source_excerpt(
        mut self,
        related_source_excerpt: impl Into<String>,
    ) -> Self {
        self.related_source_excerpt = Some(related_source_excerpt.into());
        self
    }

    pub fn with_prior_attempts<I, S>(mut self, prior_attempts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.prior_attempts = collect_values(prior_attempts);
        self
    }

    pub fn with_repair_attempt_ledger<I, S>(mut self, repair_attempt_ledger: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.repair_attempt_ledger = collect_values(repair_attempt_ledger);
        self
    }

    pub fn with_repair_focus(mut self, repair_focus: impl Into<String>) -> Self {
        self.repair_focus = Some(repair_focus.into());
        self
    }

    pub fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        self.diagnostic = Some(diagnostic.into());
        self
    }

    pub fn render(&self) -> Option<String> {
        if self.guard.trim().is_empty() && self.is_empty_without_guard() {
            return None;
        }

        let mut lines = vec!["Contract correction evidence:".to_string()];
        push_field(&mut lines, "guard", Some(&self.guard));
        push_field(&mut lines, "failed_step", self.failed_step.as_deref());
        push_field(
            &mut lines,
            "violated_contract",
            self.violated_contract.as_deref(),
        );
        push_field(&mut lines, "reason_code", self.reason_code.as_deref());
        push_field(
            &mut lines,
            "failure_signature",
            self.failure_signature.as_deref(),
        );
        push_field(&mut lines, "failure_kind", self.failure_kind.as_deref());
        push_field(
            &mut lines,
            "diagnostic_code",
            self.diagnostic_code.as_deref(),
        );
        push_field(&mut lines, "command", self.command.as_deref());
        push_field(&mut lines, "tool", self.tool.as_deref());
        push_field(&mut lines, "target_field", self.target_field.as_deref());
        push_field(&mut lines, "target_path", self.target_path.as_deref());
        push_list(&mut lines, "candidate_artifacts", &self.candidate_artifacts);
        push_field(&mut lines, "repair_target", self.repair_target.as_deref());
        push_list(&mut lines, "required_fields", &self.required_fields);
        push_list(&mut lines, "required_literals", &self.required_literals);
        push_list(&mut lines, "missing_literals", &self.missing_literals);
        push_list(&mut lines, "required_paths", &self.required_paths);
        push_list(&mut lines, "missing_paths", &self.missing_paths);
        push_list(&mut lines, "affected_cases", &self.affected_cases);
        push_list(
            &mut lines,
            "observed_expected_pairs",
            &self.observed_expected_pairs,
        );
        push_field(&mut lines, "rejected_value", self.rejected_value.as_deref());
        push_field(
            &mut lines,
            "required_action",
            self.required_action.as_deref(),
        );
        push_field(
            &mut lines,
            "related_source_excerpt",
            self.related_source_excerpt.as_deref(),
        );
        push_list(&mut lines, "prior_attempts", &self.prior_attempts);
        push_list(
            &mut lines,
            "repair_attempt_ledger",
            &self.repair_attempt_ledger,
        );
        push_field(&mut lines, "repair_focus", self.repair_focus.as_deref());
        push_field(&mut lines, "diagnostic", self.diagnostic.as_deref());
        Some(lines.join("\n"))
    }

    fn is_empty_without_guard(&self) -> bool {
        self.failed_step.is_none()
            && self.violated_contract.is_none()
            && self.reason_code.is_none()
            && self.failure_signature.is_none()
            && self.failure_kind.is_none()
            && self.diagnostic_code.is_none()
            && self.command.is_none()
            && self.tool.is_none()
            && self.target_field.is_none()
            && self.target_path.is_none()
            && self.candidate_artifacts.is_empty()
            && self.repair_target.is_none()
            && self.required_fields.is_empty()
            && self.required_literals.is_empty()
            && self.missing_literals.is_empty()
            && self.required_paths.is_empty()
            && self.missing_paths.is_empty()
            && self.affected_cases.is_empty()
            && self.observed_expected_pairs.is_empty()
            && self.rejected_value.is_none()
            && self.required_action.is_none()
            && self.related_source_excerpt.is_none()
            && self.prior_attempts.is_empty()
            && self.repair_attempt_ledger.is_empty()
            && self.repair_focus.is_none()
            && self.diagnostic.is_none()
    }
}

pub fn failure_signature<I, S>(parts: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let normalized = parts
        .into_iter()
        .filter_map(|part| {
            let value = bounded_value(part.as_ref());
            if value.trim().is_empty() {
                None
            } else {
                Some(value)
            }
        })
        .collect::<Vec<_>>()
        .join("|");
    bounded_value(&normalized)
}

fn collect_values<I, S>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    values.into_iter().map(Into::into).collect()
}

fn push_field(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    if value.trim().is_empty() {
        return;
    }
    lines.push(format!("- {key}: {}", bounded_value(value)));
}

fn push_list(lines: &mut Vec<String>, key: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    let mut rendered = values
        .iter()
        .take(MAX_LIST_ITEMS)
        .map(|value| bounded_value(value))
        .collect::<Vec<_>>();
    if values.len() > MAX_LIST_ITEMS {
        rendered.push(format!("... ({} more)", values.len() - MAX_LIST_ITEMS));
    }
    lines.push(format!("- {key}: {}", rendered.join(", ")));
}

fn bounded_value(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::new();
    for ch in normalized.chars().take(MAX_FIELD_CHARS) {
        out.push(ch);
    }
    if normalized.chars().count() > MAX_FIELD_CHARS {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_correction_evidence_renders_missing_literals() {
        let evidence = ContractEvidence::new("plan_lint.profile_obligations")
            .with_failed_step("create-package-json")
            .with_violated_contract("nextjs_dependencies_required")
            .with_target_field("instruction")
            .with_required_literals(vec![
                "next".to_string(),
                "react".to_string(),
                "react-dom".to_string(),
            ])
            .with_missing_literals(vec!["react-dom".to_string()])
            .with_required_action(
                "include these exact literals in the corrected package.json step instruction"
                    .to_string(),
            );

        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("Contract correction evidence"));
        assert!(rendered.contains("- guard: plan_lint.profile_obligations"));
        assert!(rendered.contains("- failed_step: create-package-json"));
        assert!(rendered.contains("- violated_contract: nextjs_dependencies_required"));
        assert!(rendered.contains("- required_literals: next, react, react-dom"));
        assert!(rendered.contains("- missing_literals: react-dom"));
    }

    #[test]
    fn contract_evidence_alias_keeps_plan_correction_name_usable() {
        let evidence = PlanCorrectionEvidence::new("plan_lint.profile_obligations")
            .with_failed_step("create-package-json")
            .with_required_paths(vec!["package.json"])
            .with_missing_paths(vec!["package.json"])
            .with_diagnostic("missing expected path");

        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("- failed_step: create-package-json"));
        assert!(rendered.contains("- required_paths: package.json"));
        assert!(rendered.contains("- missing_paths: package.json"));
        assert!(rendered.contains("- diagnostic: missing expected path"));
    }

    #[test]
    fn contract_evidence_renders_runtime_fields() {
        let evidence = ContractEvidence::new("tool_protocol")
            .with_failed_step("create-game-canvas")
            .with_violated_contract("tool_args_missing_required_field")
            .with_reason_code("tool_args_missing_required_field")
            .with_failure_signature("tool_protocol|create-game-canvas|Write|path")
            .with_failure_kind("tool_protocol_error")
            .with_diagnostic_code("tool_args_missing_required_field")
            .with_command("npm run build")
            .with_tool("Write")
            .with_target_field("path")
            .with_target_path("src/components/GameCanvas.tsx")
            .with_candidate_artifacts(vec!["src/components/GameCanvas.tsx"])
            .with_repair_target("src/components/GameCanvas.tsx")
            .with_required_fields(vec!["path", "content"])
            .with_observed_expected_pairs(vec![
                "observed=missing path; expected=Write.path".to_string(),
            ])
            .with_related_source_excerpt("src/components/GameCanvas.tsx:1\n>1: broken")
            .with_prior_attempts(vec!["attempt 1: same signature"])
            .with_repair_attempt_ledger(vec![
                "repair attempt 1: tool_protocol|create-game-canvas|Write|path",
            ])
            .with_repair_focus("emit valid Write call for src/components/GameCanvas.tsx")
            .with_diagnostic("Write missing path");

        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("- guard: tool_protocol"));
        assert!(rendered.contains("- failed_step: create-game-canvas"));
        assert!(rendered.contains("- violated_contract: tool_args_missing_required_field"));
        assert!(rendered.contains("- reason_code: tool_args_missing_required_field"));
        assert!(
            rendered.contains("- failure_signature: tool_protocol|create-game-canvas|Write|path")
        );
        assert!(rendered.contains("- failure_kind: tool_protocol_error"));
        assert!(rendered.contains("- diagnostic_code: tool_args_missing_required_field"));
        assert!(rendered.contains("- command: npm run build"));
        assert!(rendered.contains("- tool: Write"));
        assert!(rendered.contains("- target_field: path"));
        assert!(rendered.contains("- target_path: src/components/GameCanvas.tsx"));
        assert!(rendered.contains("- candidate_artifacts: src/components/GameCanvas.tsx"));
        assert!(rendered.contains("- repair_target: src/components/GameCanvas.tsx"));
        assert!(rendered.contains("- required_fields: path, content"));
        assert!(
            rendered
                .contains("- observed_expected_pairs: observed=missing path; expected=Write.path")
        );
        assert!(
            rendered
                .contains("- related_source_excerpt: src/components/GameCanvas.tsx:1 >1: broken")
        );
        assert!(rendered.contains("- prior_attempts: attempt 1: same signature"));
        assert!(rendered.contains(
            "- repair_attempt_ledger: repair attempt 1: tool_protocol|create-game-canvas|Write|path"
        ));
        assert!(rendered.contains("- repair_focus: emit valid Write call"));
    }

    #[test]
    fn plan_correction_evidence_bounds_long_values() {
        let evidence = ContractEvidence::new("plan_lint.profile_obligations")
            .with_diagnostic("x".repeat(MAX_FIELD_CHARS + 20))
            .with_required_literals(
                (0..(MAX_LIST_ITEMS + 2)).map(|index| format!("literal-{index}")),
            );

        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("..."));
        assert!(rendered.contains("2 more"));
        assert!(rendered.len() < 800);
    }

    #[test]
    fn failure_signature_is_stable_and_bounded() {
        let signature = failure_signature([
            " verifier ",
            "verify-build",
            "npm   run   build",
            "command_failed:1",
            "app/page.tsx",
        ]);

        assert_eq!(
            signature,
            "verifier|verify-build|npm run build|command_failed:1|app/page.tsx"
        );
        assert!(signature.len() <= MAX_FIELD_CHARS);
    }
}
