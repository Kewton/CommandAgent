use crate::planner::capability_catalog::ResolvedCapability;
use crate::planner::profile::ProfileFixReproducerSuggestion;
use crate::planner::profile_manifest::{CheckBinding, nextjs_manifest};

pub(crate) fn suggestion_for(goal: &str) -> Option<ProfileFixReproducerSuggestion> {
    contract_hook_suggestion(goal).or_else(|| build_suggestion(goal))
}

fn contract_hook_suggestion(goal: &str) -> Option<ProfileFixReproducerSuggestion> {
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
    (!matched.is_empty()).then(|| ProfileFixReproducerSuggestion {
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

fn build_suggestion(goal: &str) -> Option<ProfileFixReproducerSuggestion> {
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
    Some(ProfileFixReproducerSuggestion {
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
        ResolvedCapability::CommandCheck(_)
        | ResolvedCapability::Internal(_)
        | ResolvedCapability::Probe(_) => None,
    }
}
