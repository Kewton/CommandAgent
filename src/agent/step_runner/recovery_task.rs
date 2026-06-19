//! Clear repair-task contracts derived from deterministic failure evidence.
//!
//! A recovery task contract is rendered into existing bounded repair prompts.
//! It does not grant retry authority, select a new workflow, or execute tools.

use crate::agent::step_runner::correction_evidence::ContractEvidence;

const MAX_FIELD_CHARS: usize = 240;
const MAX_LIST_ITEMS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryExecutionEnvelope {
    ReadOnlyEvidence,
    FileMutationRepair,
    ToolProtocolCorrection,
}

impl RecoveryExecutionEnvelope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnlyEvidence => "read_only_evidence",
            Self::FileMutationRepair => "file_mutation_repair",
            Self::ToolProtocolCorrection => "tool_protocol_correction",
        }
    }

    fn tool_policy(self) -> &'static str {
        match self {
            Self::ReadOnlyEvidence => "read_only",
            Self::FileMutationRepair => "file_mutation_allowed",
            Self::ToolProtocolCorrection => "current_tool_protocol",
        }
    }

    fn evidence_requirement(self) -> &'static str {
        match self {
            Self::ReadOnlyEvidence => "repository_read_evidence",
            Self::FileMutationRepair => "file_change_or_explicit_blocker",
            Self::ToolProtocolCorrection => "valid_tool_call",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecoveryTaskContract {
    pub source: String,
    pub failed_step: Option<String>,
    pub contract_code: Option<String>,
    pub blocker: Option<String>,
    pub required_action: Option<String>,
    pub repair_target: Option<String>,
    pub candidate_artifacts: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub disallowed_actions: Vec<String>,
    pub success_check: Option<String>,
    pub evidence_signature: Option<String>,
    pub execution_envelope: Option<RecoveryExecutionEnvelope>,
}

impl RecoveryTaskContract {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            ..Self::default()
        }
    }

    pub fn from_contract_evidence(evidence: &ContractEvidence) -> Option<Self> {
        if !known_recovery_source(&evidence.guard)
            && evidence.required_action.is_none()
            && evidence.repair_focus.is_none()
        {
            return None;
        }

        let mut task = Self::new(evidence.guard.clone())
            .with_failed_step_opt(evidence.failed_step.clone())
            .with_contract_code_opt(contract_code(evidence))
            .with_blocker_opt(blocker(evidence))
            .with_required_action_opt(required_action(evidence))
            .with_repair_target_opt(
                evidence
                    .repair_target
                    .clone()
                    .or_else(|| evidence.target_path.clone()),
            )
            .with_candidate_artifacts(evidence.candidate_artifacts.clone())
            .with_success_check_opt(success_check(evidence))
            .with_evidence_signature_opt(evidence.failure_signature.clone())
            .with_execution_envelope_opt(execution_envelope(evidence));

        if evidence.guard == "tool_protocol"
            && let Some(tool) = evidence.tool.as_deref()
        {
            task = task.with_allowed_tool(tool);
        }
        for action in disallowed_actions(evidence) {
            task = task.with_disallowed_action(action);
        }

        if task.has_task_detail() {
            Some(task)
        } else {
            None
        }
    }

    pub fn with_failed_step(mut self, failed_step: impl Into<String>) -> Self {
        self.failed_step = Some(failed_step.into());
        self
    }

    pub fn with_contract_code(mut self, contract_code: impl Into<String>) -> Self {
        self.contract_code = Some(contract_code.into());
        self
    }

    pub fn with_blocker(mut self, blocker: impl Into<String>) -> Self {
        self.blocker = Some(blocker.into());
        self
    }

    pub fn with_required_action(mut self, required_action: impl Into<String>) -> Self {
        self.required_action = Some(required_action.into());
        self
    }

    pub fn with_repair_target(mut self, repair_target: impl Into<String>) -> Self {
        self.repair_target = Some(repair_target.into());
        self
    }

    pub fn with_candidate_artifacts<I, S>(mut self, candidate_artifacts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for artifact in candidate_artifacts {
            self = self.with_candidate_artifact(artifact);
        }
        self
    }

    pub fn with_candidate_artifact(mut self, candidate_artifact: impl Into<String>) -> Self {
        push_unique(&mut self.candidate_artifacts, candidate_artifact.into());
        self
    }

    pub fn with_allowed_tool(mut self, allowed_tool: impl Into<String>) -> Self {
        push_unique(&mut self.allowed_tools, allowed_tool.into());
        self
    }

    pub fn with_disallowed_action(mut self, disallowed_action: impl Into<String>) -> Self {
        push_unique(&mut self.disallowed_actions, disallowed_action.into());
        self
    }

    pub fn with_success_check(mut self, success_check: impl Into<String>) -> Self {
        self.success_check = Some(success_check.into());
        self
    }

    pub fn with_evidence_signature(mut self, evidence_signature: impl Into<String>) -> Self {
        self.evidence_signature = Some(evidence_signature.into());
        self
    }

    pub fn with_execution_envelope(mut self, envelope: RecoveryExecutionEnvelope) -> Self {
        self.execution_envelope = Some(envelope);
        self
    }

    pub fn render(&self) -> Option<String> {
        if self.source.trim().is_empty() || !self.has_task_detail() {
            return None;
        }

        let mut lines = Vec::new();
        push_field(&mut lines, "source", Some(&self.source));
        push_field(&mut lines, "failed_step", self.failed_step.as_deref());
        push_field(&mut lines, "contract_code", self.contract_code.as_deref());
        push_field(&mut lines, "blocker", self.blocker.as_deref());
        push_field(
            &mut lines,
            "required_action",
            self.required_action.as_deref(),
        );
        push_field(&mut lines, "repair_target", self.repair_target.as_deref());
        push_list(&mut lines, "candidate_artifacts", &self.candidate_artifacts);
        push_list(&mut lines, "allowed_tools", &self.allowed_tools);
        push_list(&mut lines, "disallowed_actions", &self.disallowed_actions);
        push_field(&mut lines, "success_check", self.success_check.as_deref());
        push_field(
            &mut lines,
            "evidence_signature",
            self.evidence_signature.as_deref(),
        );
        if let Some(envelope) = self.execution_envelope {
            push_field(&mut lines, "execution_envelope", Some(envelope.as_str()));
            push_field(&mut lines, "tool_policy", Some(envelope.tool_policy()));
            push_field(
                &mut lines,
                "evidence_requirement",
                Some(envelope.evidence_requirement()),
            );
        }
        Some(lines.join("\n"))
    }

    fn with_failed_step_opt(self, failed_step: Option<String>) -> Self {
        match failed_step {
            Some(value) => self.with_failed_step(value),
            None => self,
        }
    }

    fn with_contract_code_opt(self, contract_code: Option<String>) -> Self {
        match contract_code {
            Some(value) => self.with_contract_code(value),
            None => self,
        }
    }

    fn with_blocker_opt(self, blocker: Option<String>) -> Self {
        match blocker {
            Some(value) => self.with_blocker(value),
            None => self,
        }
    }

    fn with_required_action_opt(self, required_action: Option<String>) -> Self {
        match required_action {
            Some(value) => self.with_required_action(value),
            None => self,
        }
    }

    fn with_repair_target_opt(self, repair_target: Option<String>) -> Self {
        match repair_target {
            Some(value) => self.with_repair_target(value),
            None => self,
        }
    }

    fn with_success_check_opt(self, success_check: Option<String>) -> Self {
        match success_check {
            Some(value) => self.with_success_check(value),
            None => self,
        }
    }

    fn with_evidence_signature_opt(self, evidence_signature: Option<String>) -> Self {
        match evidence_signature {
            Some(value) => self.with_evidence_signature(value),
            None => self,
        }
    }

    fn with_execution_envelope_opt(self, envelope: Option<RecoveryExecutionEnvelope>) -> Self {
        match envelope {
            Some(value) => self.with_execution_envelope(value),
            None => self,
        }
    }

    fn has_task_detail(&self) -> bool {
        self.blocker.is_some()
            || self.required_action.is_some()
            || self.repair_target.is_some()
            || !self.candidate_artifacts.is_empty()
            || !self.allowed_tools.is_empty()
            || !self.disallowed_actions.is_empty()
            || self.success_check.is_some()
            || self.execution_envelope.is_some()
    }
}

pub fn recovery_execution_envelope(
    evidence: &[ContractEvidence],
) -> Option<RecoveryExecutionEnvelope> {
    let mut selected = None;
    for item in evidence {
        match execution_envelope(item) {
            Some(RecoveryExecutionEnvelope::ReadOnlyEvidence) => {
                return Some(RecoveryExecutionEnvelope::ReadOnlyEvidence);
            }
            Some(envelope) if selected.is_none() => selected = Some(envelope),
            _ => {}
        }
    }
    selected
}

fn known_recovery_source(source: &str) -> bool {
    matches!(
        source,
        "tool_protocol" | "step_policy" | "verifier" | "profile_verification"
    )
}

fn contract_code(evidence: &ContractEvidence) -> Option<String> {
    evidence
        .diagnostic_code
        .clone()
        .or_else(|| evidence.reason_code.clone())
        .or_else(|| evidence.violated_contract.clone())
}

fn blocker(evidence: &ContractEvidence) -> Option<String> {
    match evidence.guard.as_str() {
        "tool_protocol" => Some(format!(
            "Tool call violated schema{}",
            evidence
                .tool
                .as_deref()
                .map(|tool| format!(" for {tool}"))
                .unwrap_or_default()
        )),
        "step_policy" => Some(format!(
            "Step tool policy rejected{}",
            evidence
                .tool
                .as_deref()
                .map(|tool| format!(" {tool}"))
                .unwrap_or_default()
        )),
        "verifier" => Some(format!(
            "Verifier command failed{}",
            evidence
                .command
                .as_deref()
                .map(|command| format!(": {command}"))
                .unwrap_or_default()
        )),
        "profile_verification" => {
            let code = contract_code(evidence).unwrap_or_else(|| "profile contract".to_string());
            Some(format!("Profile verification failed: {code}"))
        }
        _ => evidence
            .diagnostic_code
            .as_deref()
            .or(evidence.reason_code.as_deref())
            .or(evidence.violated_contract.as_deref())
            .map(|code| format!("Contract rejected: {code}")),
    }
}

fn required_action(evidence: &ContractEvidence) -> Option<String> {
    if let Some(action) = evidence.required_action.clone() {
        return Some(action);
    }

    match evidence.guard.as_str() {
        "tool_protocol" => Some(format!(
            "Emit exactly one valid {} tool call with the required fields.",
            evidence.tool.as_deref().unwrap_or("tool")
        )),
        "step_policy" => Some(
            "Do not mutate in this step; move file changes into an explicit mutation-allowed create/edit/repair step."
                .to_string(),
        ),
        "verifier" => Some("Fix the original verifier failure before adding feature work.".to_string()),
        "profile_verification" => {
            if let Some(focus) = evidence.repair_focus.clone() {
                Some(focus)
            } else {
                Some("Fix the reported profile contract before adding feature work.".to_string())
            }
        }
        _ => evidence.repair_focus.clone(),
    }
}

fn success_check(evidence: &ContractEvidence) -> Option<String> {
    match evidence.guard.as_str() {
        "tool_protocol" => Some("tool schema validation".to_string()),
        "step_policy" => Some("step tool policy".to_string()),
        "verifier" => evidence.command.clone(),
        "profile_verification" => Some("profile verification".to_string()),
        _ => None,
    }
}

fn execution_envelope(evidence: &ContractEvidence) -> Option<RecoveryExecutionEnvelope> {
    match evidence.guard.as_str() {
        "step_policy" if contract_code(evidence).as_deref() == Some("read_only_step_mutation") => {
            Some(RecoveryExecutionEnvelope::ReadOnlyEvidence)
        }
        "tool_protocol" => Some(RecoveryExecutionEnvelope::ToolProtocolCorrection),
        "verifier" => Some(RecoveryExecutionEnvelope::FileMutationRepair),
        "profile_verification"
            if evidence.repair_target.is_some() || evidence.required_action.is_some() =>
        {
            Some(RecoveryExecutionEnvelope::FileMutationRepair)
        }
        _ => None,
    }
}

fn disallowed_actions(evidence: &ContractEvidence) -> Vec<String> {
    match evidence.guard.as_str() {
        "tool_protocol" => vec![
            "Do not answer in prose instead of a tool call.".to_string(),
            "Do not run dependency installation.".to_string(),
        ],
        "step_policy" => vec![
            "Do not use Write in a read-only step.".to_string(),
            "Do not use Edit in a read-only step.".to_string(),
            "Do not use mutating Bash in a read-only step.".to_string(),
        ],
        "verifier" => vec![
            "Do not change the verifier command to fake success.".to_string(),
            "Do not rewrite build scripts to bypass errors.".to_string(),
            "Do not run dependency setup except through the existing approved setup path."
                .to_string(),
        ],
        "profile_verification" => vec![
            "Do not add unrelated feature work before fixing the profile contract.".to_string(),
        ],
        _ => Vec::new(),
    }
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

fn push_unique(values: &mut Vec<String>, value: String) {
    let bounded = bounded_value(&value);
    if !bounded.trim().is_empty() && !values.iter().any(|existing| existing == &bounded) {
        values.push(bounded);
        if values.len() > MAX_LIST_ITEMS {
            values.truncate(MAX_LIST_ITEMS);
        }
    }
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
    fn renders_bounded_recovery_task() {
        let task = RecoveryTaskContract::new("verifier")
            .with_failed_step("verify-build")
            .with_contract_code("command_failed:1")
            .with_blocker("Verifier command failed: npm run build")
            .with_required_action("Fix the original verifier failure before adding feature work.")
            .with_repair_target("app/page.tsx")
            .with_candidate_artifacts((0..10).map(|index| format!("app/file-{index}.tsx")))
            .with_disallowed_action("Do not change the verifier command to fake success.")
            .with_success_check("npm run build")
            .with_evidence_signature("verifier|verify-build|npm run build|command_failed:1")
            .with_execution_envelope(RecoveryExecutionEnvelope::FileMutationRepair);

        let rendered = task.render().unwrap();

        assert!(rendered.contains("- source: verifier"));
        assert!(rendered.contains("- failed_step: verify-build"));
        assert!(rendered.contains("- required_action: Fix the original verifier failure"));
        assert!(rendered.contains("- repair_target: app/page.tsx"));
        assert!(rendered.contains("execution_envelope: file_mutation_repair"));
        assert!(rendered.contains("app/file-0.tsx"));
        assert!(rendered.contains("success_check: npm run build"));
        assert!(!rendered.contains("app/file-9.tsx"));
    }

    #[test]
    fn empty_task_does_not_render() {
        let task = RecoveryTaskContract::new("verifier");

        assert!(task.render().is_none());
    }

    #[test]
    fn unknown_evidence_without_action_does_not_make_task() {
        let evidence = ContractEvidence::new("unknown_guard").with_diagnostic("something failed");

        assert!(RecoveryTaskContract::from_contract_evidence(&evidence).is_none());
    }

    #[test]
    fn verifier_evidence_becomes_recovery_task() {
        let evidence = ContractEvidence::new("verifier")
            .with_failed_step("verify-build")
            .with_violated_contract("command_failed:1")
            .with_command("npm run build")
            .with_repair_target("app/page.tsx")
            .with_candidate_artifacts(vec!["app/page.tsx"])
            .with_failure_signature("verifier|verify-build|npm run build|command_failed:1");

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("blocker: Verifier command failed: npm run build"));
        assert!(rendered.contains("required_action: Fix the original verifier failure"));
        assert!(rendered.contains("repair_target: app/page.tsx"));
        assert!(rendered.contains("success_check: npm run build"));
        assert!(rendered.contains("execution_envelope: file_mutation_repair"));
        assert!(rendered.contains("Do not change the verifier command"));
    }

    #[test]
    fn step_policy_task_does_not_authorize_mutation() {
        let evidence = ContractEvidence::new("step_policy")
            .with_failed_step("inspect-source")
            .with_violated_contract("read_only_step_mutation")
            .with_tool("Write");

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("Step tool policy rejected Write"));
        assert!(rendered.contains("Do not mutate in this step"));
        assert!(rendered.contains("Do not use Write in a read-only step"));
        assert!(rendered.contains("success_check: step tool policy"));
        assert!(rendered.contains("execution_envelope: read_only_evidence"));
        assert!(rendered.contains("evidence_requirement: repository_read_evidence"));
    }

    #[test]
    fn profile_task_preserves_selected_route_target() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_failed_step("phase-ui")
            .with_violated_contract("nextjs_route_not_integrated")
            .with_repair_target("app/page.tsx")
            .with_candidate_artifacts(vec!["app/page.tsx", "app/hooks/useGame.ts"])
            .with_required_action(
                "edit app/page.tsx so it imports or references app/hooks/useGame.ts",
            );

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("Profile verification failed: nextjs_route_not_integrated"));
        assert!(rendered.contains("repair_target: app/page.tsx"));
        assert!(rendered.contains("candidate_artifacts: app/page.tsx, app/hooks/useGame.ts"));
        assert!(rendered.contains("success_check: profile verification"));
        assert!(rendered.contains("execution_envelope: file_mutation_repair"));
    }

    #[test]
    fn envelope_selection_prefers_read_only_evidence() {
        let evidence = vec![
            ContractEvidence::new("verifier")
                .with_violated_contract("command_failed:1")
                .with_command("npm run build"),
            ContractEvidence::new("step_policy")
                .with_violated_contract("read_only_step_mutation")
                .with_tool("Write"),
        ];

        assert_eq!(
            recovery_execution_envelope(&evidence),
            Some(RecoveryExecutionEnvelope::ReadOnlyEvidence)
        );
    }
}
