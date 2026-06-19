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
    pub target_field: Option<String>,
    pub required_literals: Vec<String>,
    pub missing_literals: Vec<String>,
    pub required_paths: Vec<String>,
    pub missing_paths: Vec<String>,
    pub rejected_value: Option<String>,
    pub required_action: Option<String>,
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

    pub fn with_target_field(mut self, target_field: impl Into<String>) -> Self {
        self.target_field = Some(target_field.into());
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

    pub fn with_rejected_value(mut self, rejected_value: impl Into<String>) -> Self {
        self.rejected_value = Some(rejected_value.into());
        self
    }

    pub fn with_required_action(mut self, required_action: impl Into<String>) -> Self {
        self.required_action = Some(required_action.into());
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
        push_field(&mut lines, "target_field", self.target_field.as_deref());
        push_list(&mut lines, "required_literals", &self.required_literals);
        push_list(&mut lines, "missing_literals", &self.missing_literals);
        push_list(&mut lines, "required_paths", &self.required_paths);
        push_list(&mut lines, "missing_paths", &self.missing_paths);
        push_field(&mut lines, "rejected_value", self.rejected_value.as_deref());
        push_field(
            &mut lines,
            "required_action",
            self.required_action.as_deref(),
        );
        push_field(&mut lines, "diagnostic", self.diagnostic.as_deref());
        Some(lines.join("\n"))
    }

    fn is_empty_without_guard(&self) -> bool {
        self.failed_step.is_none()
            && self.violated_contract.is_none()
            && self.target_field.is_none()
            && self.required_literals.is_empty()
            && self.missing_literals.is_empty()
            && self.required_paths.is_empty()
            && self.missing_paths.is_empty()
            && self.rejected_value.is_none()
            && self.required_action.is_none()
            && self.diagnostic.is_none()
    }
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
}
