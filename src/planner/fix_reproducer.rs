use serde_json::json;

use crate::eval_events;
use crate::planner::adjudication::contract::is_fix_intent;
use crate::planner::capability_catalog::ResolvedCapability;
use crate::planner::profile::is_nextjs_profile;
use crate::planner::profile_manifest::{CheckBinding, nextjs_manifest};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReproducerSuggestion {
    pub(crate) basis: String,
    pub(crate) suggestion: String,
}

pub(crate) fn suggestion_for(plan: &UltraPlan) -> Option<ReproducerSuggestion> {
    if !is_fix_intent(&plan.intent) || !is_nextjs_profile(&plan.profile) {
        return None;
    }
    contract_hook_suggestion(&plan.goal).or_else(|| build_suggestion(&plan.goal))
}

pub(crate) fn attach_to_phase_prompt(
    plan: &UltraPlan,
    phase: &UltraPhase,
    eval_events_path: Option<&std::path::Path>,
    mut prompt: String,
) -> String {
    if plan.phases.first().is_none_or(|first| first.id != phase.id) {
        return prompt;
    }
    let Some(suggestion) = suggestion_for(plan) else {
        return prompt;
    };
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "fix_reproducer_suggested",
            "basis": suggestion.basis,
            "suggestion": suggestion.suggestion,
        }),
    );
    prompt.push_str(
        "\n\nFix contract section 8 reproducer suggestion (guidance, not enforcement):\n- basis: ",
    );
    prompt.push_str(&suggestion.basis);
    prompt.push_str("\n- canonical candidate: ");
    prompt.push_str(&suggestion.suggestion);
    prompt.push_str("\nUse this candidate when it represents the stated failure. A different deterministic R remains permitted; the F1 baseline gate remains authoritative.");
    prompt
}

fn contract_hook_suggestion(goal: &str) -> Option<ReproducerSuggestion> {
    let lower = goal.to_ascii_lowercase();
    let checks = nextjs_manifest().checks.get("contract-wiring")?;
    let mut matched = Vec::new();
    let mut bases = Vec::new();
    for check in checks {
        if check.id != "hook_attribute_present" {
            continue;
        }
        let attribute = param(check, "attribute")?;
        let token = format!("data-anvil-{attribute}");
        if !lower.contains(&token) || binding_value_conflicts(checks, check, &lower) {
            continue;
        }
        let value = param(check, "value")?;
        let path = param(check, "path")?;
        let command = shell_command(check)?;
        matched.push(format!(
            "profile_catalog:hook_attribute_present(attribute={attribute},value={value},path={path}) => {command}"
        ));
        let basis = if value.is_empty() {
            token
        } else {
            format!("{token}={value}")
        };
        if !bases.contains(&basis) {
            bases.push(basis);
        }
    }
    (!matched.is_empty()).then(|| ReproducerSuggestion {
        basis: format!("goal_contract_attribute:{}", bases.join(",")),
        suggestion: matched.join(" | "),
    })
}

fn binding_value_conflicts(checks: &[CheckBinding], check: &CheckBinding, goal: &str) -> bool {
    let Some(attribute) = param(check, "attribute") else {
        return true;
    };
    let value = param(check, "value").unwrap_or_default();
    if value.is_empty() {
        return false;
    }
    checks.iter().any(|candidate| {
        candidate.id == "hook_attribute_present"
            && param(candidate, "attribute") == Some(attribute)
            && param(candidate, "value").is_some_and(|other| other != value && goal.contains(other))
    })
}

fn build_suggestion(goal: &str) -> Option<ReproducerSuggestion> {
    let lower = goal.to_ascii_lowercase();
    let mentioned = [
        "npm run build",
        "next build",
        "build",
        "compile",
        "compiler",
        "compilation",
        "ビルド",
        "コンパイル",
        "コンパイラ",
    ]
    .iter()
    .any(|term| lower.contains(term));
    if !mentioned {
        return None;
    }
    let check = nextjs_manifest()
        .checks
        .get("build-verification")?
        .iter()
        .find(|check| check.id == "next_build_verify")?;
    Some(ReproducerSuggestion {
        basis: "goal_failure_kind:build_or_compile".to_string(),
        suggestion: format!(
            "profile_catalog:next_build_verify => {}",
            shell_command(check)?
        ),
    })
}

fn param<'a>(check: &'a CheckBinding, name: &str) -> Option<&'a str> {
    check.params.get(name)?.as_str()
}

fn shell_command(check: &CheckBinding) -> Option<String> {
    match crate::planner::capability_catalog::resolve(&check.id, &check.params).ok()? {
        ResolvedCapability::ShellCheck(command) => Some(command),
        ResolvedCapability::Internal(_) | ResolvedCapability::Probe(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::ultra_plan::UltraPhase;

    fn fix_plan(goal: &str) -> UltraPlan {
        UltraPlan {
            goal: goal.to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "fix".to_string(),
            phases: vec![UltraPhase {
                id: "reproduce-before".to_string(),
                prompt: "Reproduce the failure before repair.".to_string(),
            }],
        }
    }

    #[test]
    fn hook_goal_suggests_route_bound_catalog_check() {
        let suggestion = suggestion_for(&fix_plan(
            "このNext.jsアプリはリスタート操作の契約フック（data-anvil-action=\"restart\"）が欠落しており検証に失敗します。",
        ))
        .expect("hook suggestion");

        assert!(suggestion.basis.contains("data-anvil-action=restart"));
        assert!(suggestion.suggestion.contains("hook_attribute_present"));
        assert!(suggestion.suggestion.contains("attribute=action"));
        assert!(suggestion.suggestion.contains("value=restart"));
        assert!(suggestion.suggestion.contains("path=src/app/page.tsx"));
    }

    #[test]
    fn build_goal_suggests_catalog_build_oracle() {
        let suggestion = suggestion_for(&fix_plan(
            "このNext.jsプロジェクトは npm run build が失敗します。コンパイル原因を修正してください。",
        ))
        .expect("build suggestion");

        assert_eq!(suggestion.basis, "goal_failure_kind:build_or_compile");
        assert!(suggestion.suggestion.contains("next_build_verify"));
        assert!(suggestion.suggestion.contains("npm run build"));
    }

    #[test]
    fn goal_without_contract_or_failure_kind_keeps_legacy_behavior() {
        assert!(suggestion_for(&fix_plan("既存の不具合を修正してください。")).is_none());
    }

    #[test]
    fn create_intent_never_receives_fix_reproducer_guidance() {
        let mut plan = fix_plan("npm run build の失敗を修正してください。");
        plan.intent = "create".to_string();
        assert!(suggestion_for(&plan).is_none());
    }

    #[test]
    fn first_phase_prompt_records_the_suggestion_once() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let plan =
            fix_plan("このNext.jsアプリはdata-anvil-action=\"restart\"欠落で検証に失敗します。");

        let prompt = attach_to_phase_prompt(
            &plan,
            &plan.phases[0],
            Some(&events),
            "base prompt".to_string(),
        );

        assert!(prompt.contains("guidance, not enforcement"));
        assert!(prompt.contains("F1 baseline gate remains authoritative"));
        let event = std::fs::read_to_string(events).unwrap();
        assert_eq!(event.matches("fix_reproducer_suggested").count(), 1);
        assert!(event.contains(r#""basis":"goal_contract_attribute:data-anvil-action=restart""#));
        assert!(event.contains(r#""suggestion":"profile_catalog:hook_attribute_present"#));
    }
}
