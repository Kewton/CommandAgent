use std::path::Path;

use crate::planner::capability_catalog::ResolvedCapability;
use crate::planner::contract_attribute_repair::ContractAttributeIssue;
use crate::planner::profile::is_nextjs_profile;
use crate::planner::profile_manifest::{CheckBinding, nextjs_manifest};
use crate::planner::step_plan::{StepKind, StepPlan};
use crate::planner::ultra_plan::UltraPhase;

const CONTEXT_HEADING: &str = "Fix F1 profile contract predicate (runtime-bound):";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixContractPredicateContext {
    target_path: String,
    guidance: String,
}

impl FixContractPredicateContext {
    pub(crate) fn from_failed_reproducer(
        root: &Path,
        profile: &str,
        command: &str,
        eval_events_path: Option<&Path>,
    ) -> Option<Self> {
        let issue = matched_manifest_issue(profile, command)?;
        let guidance = crate::planner::contract_attribute_repair::guidance_for_issue(
            Some(root),
            &issue,
            eval_events_path,
        );
        Some(Self {
            target_path: issue.path,
            guidance,
        })
    }

    fn render(&self) -> String {
        format!(
            "{CONTEXT_HEADING}\n- capability: hook_attribute_present\n- write-pressure target: {} (selection_reason=contract_attribute)\n\n{}",
            self.target_path,
            self.guidance.trim(),
        )
    }
}

pub(crate) fn attach_to_phase_prompt(
    phase: &UltraPhase,
    context: Option<&FixContractPredicateContext>,
    mut prompt: String,
) -> String {
    if predicate_phase(phase)
        && let Some(context) = context
    {
        prompt.push_str("\n\n");
        prompt.push_str(&context.render());
    }
    prompt
}

pub(crate) fn bind_step_plan(
    phase: &UltraPhase,
    context: Option<&FixContractPredicateContext>,
    plan: &mut StepPlan,
) {
    let Some(context) = context.filter(|_| predicate_phase(phase)) else {
        return;
    };
    let guidance = context.render();
    for step in &mut plan.steps {
        if !matches!(step.step_kind(), StepKind::Inspect | StepKind::Implement) {
            continue;
        }
        if !step.instruction.contains(CONTEXT_HEADING) {
            step.instruction.push_str("\n\n");
            step.instruction.push_str(&guidance);
        }
        if step.step_kind() == StepKind::Implement
            && !step.expected_paths.contains(&context.target_path)
        {
            step.expected_paths.push(context.target_path.clone());
        }
    }
}

fn predicate_phase(phase: &UltraPhase) -> bool {
    matches!(phase.id.as_str(), "isolate-cause" | "repair")
}

fn matched_manifest_issue(profile: &str, command: &str) -> Option<ContractAttributeIssue> {
    if !is_nextjs_profile(profile) {
        return None;
    }
    nextjs_manifest()
        .checks
        .values()
        .flatten()
        .filter(|check| check.id == "hook_attribute_present")
        .find_map(|check| issue_for_matching_check(check, command))
}

fn issue_for_matching_check(check: &CheckBinding, command: &str) -> Option<ContractAttributeIssue> {
    let ResolvedCapability::ShellCheck(canonical) =
        crate::planner::capability_catalog::resolve(&check.id, &check.params).ok()?
    else {
        return None;
    };
    if command_key(command)? != command_key(&canonical)? {
        return None;
    }
    let attribute = param(check, "attribute")?;
    let value = param(check, "value")?;
    let path = param(check, "path")?;
    crate::tools::path_guard::validate_workspace_relative(path).ok()?;
    let attribute = match (attribute, value) {
        ("action", value) if !value.is_empty() => format!("data-anvil-action=\"{value}\""),
        ("state", "") => "data-anvil-state".to_string(),
        _ => return None,
    };
    Some(ContractAttributeIssue {
        attribute,
        path: path.to_string(),
    })
}

fn param<'a>(check: &'a CheckBinding, name: &str) -> Option<&'a str> {
    check.params.get(name)?.as_str()
}

fn command_key(command: &str) -> Option<String> {
    let script = command
        .trim()
        .strip_prefix("node -p '")?
        .strip_suffix('\'')?;
    let mut key = String::with_capacity(script.len());
    let mut quote = None;
    let mut escaped = false;
    for ch in script.chars() {
        if let Some(active_quote) = quote {
            key.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
        } else if matches!(ch, '"' | '\'' | '`') {
            quote = Some(ch);
            key.push(ch);
        } else if !ch.is_ascii_whitespace() {
            key.push(ch);
        }
    }
    quote.is_none().then_some(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::planner::repair_target_selection::RepairTargetSelectionReason;
    use crate::planner::step_plan::PlanStep;
    use crate::planner::ultra_plan::UltraPlan;
    use clap::Parser;

    fn restart_command() -> String {
        crate::planner::verify::hook_attribute_present_check_command(
            "action",
            "restart",
            "src/app/page.tsx",
        )
        .unwrap()
    }

    fn workspace() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/app")).unwrap();
        std::fs::write(
            root.path().join("src/app/page.tsx"),
            "export default function Page() {\n  return <button data-anvil-action=\"primary\">Start</button>;\n}\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn failed_manifest_predicate_builds_route_bound_guidance_and_event() {
        let root = workspace();
        let events = root.path().join("events.jsonl");

        let context = FixContractPredicateContext::from_failed_reproducer(
            root.path(),
            "nextjs",
            &restart_command(),
            Some(&events),
        )
        .expect("profile predicate context");
        let rendered = context.render();

        assert!(rendered.contains("src/app/page.tsx"));
        assert!(rendered.contains(r#"missing attribute: `data-anvil-action="restart"`"#));
        assert!(rendered.contains("Existing hook locations:"));
        assert!(rendered.contains("near line 2"));
        assert!(rendered.contains(r#"data-anvil-action="restart""#));
        let event = std::fs::read_to_string(events).unwrap();
        assert_eq!(
            event.matches("contract_attribute_repair_guidance").count(),
            1
        );
        assert!(event.contains(r#""path":"src/app/page.tsx""#));
    }

    #[test]
    fn harmless_command_whitespace_from_measured_run_keeps_manifest_lineage() {
        let root = workspace();
        let canonical = restart_command();
        let variant = canonical.replacen(")].some", ") ].some", 1);
        assert_ne!(variant, canonical);

        assert!(
            FixContractPredicateContext::from_failed_reproducer(
                root.path(),
                "nextjs",
                &variant,
                None,
            )
            .is_some()
        );
    }

    #[test]
    fn whitespace_inside_predicate_string_does_not_alias_manifest_lineage() {
        let root = workspace();
        let changed_path = restart_command().replacen("src/app/page.tsx", "src/app /page.tsx", 1);

        assert!(
            FixContractPredicateContext::from_failed_reproducer(
                root.path(),
                "nextjs",
                &changed_path,
                None,
            )
            .is_none()
        );
    }

    #[test]
    fn unrelated_or_non_nextjs_reproducer_does_not_gain_contract_context() {
        let root = workspace();
        assert!(
            FixContractPredicateContext::from_failed_reproducer(
                root.path(),
                "nextjs",
                "npm run build",
                None,
            )
            .is_none()
        );
        assert!(
            FixContractPredicateContext::from_failed_reproducer(
                root.path(),
                "data",
                &restart_command(),
                None,
            )
            .is_none()
        );
    }

    #[test]
    fn phase_two_guidance_targets_inspect_and_implement_but_not_verify() {
        let root = workspace();
        let context = FixContractPredicateContext::from_failed_reproducer(
            root.path(),
            "nextjs",
            &restart_command(),
            None,
        )
        .unwrap();
        let phase = UltraPhase {
            id: "isolate-cause".to_string(),
            prompt: "Isolate the route-bound predicate failure.".to_string(),
        };
        let prompt = attach_to_phase_prompt(&phase, Some(&context), "phase".to_string());
        assert!(prompt.contains("Contract attribute repair guidance"));
        assert!(prompt.contains("write-pressure target: src/app/page.tsx"));

        let mut plan = StepPlan {
            goal: "Repair the missing restart hook.".to_string(),
            steps: vec![
                PlanStep {
                    id: "inspect-page".to_string(),
                    kind: "inspect".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: "Inspect src/app/page.tsx.".to_string(),
                    expected_paths: Vec::new(),
                    verify: Vec::new(),
                },
                PlanStep {
                    id: "repair-page".to_string(),
                    kind: "repair".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: "Repair the missing hook.".to_string(),
                    expected_paths: Vec::new(),
                    verify: Vec::new(),
                },
                PlanStep {
                    id: "verify-hook".to_string(),
                    kind: "verify".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: "Run the route-bound predicate.".to_string(),
                    expected_paths: Vec::new(),
                    verify: vec![restart_command()],
                },
            ],
        };

        bind_step_plan(&phase, Some(&context), &mut plan);

        assert!(plan.steps[0].instruction.contains(CONTEXT_HEADING));
        assert!(plan.steps[1].instruction.contains(CONTEXT_HEADING));
        assert_eq!(plan.steps[1].expected_paths, ["src/app/page.tsx"]);
        assert!(!plan.steps[2].instruction.contains(CONTEXT_HEADING));
        assert!(!plan.steps[2].instruction.contains("write-pressure target"));
        let lint = crate::planner::lint::lint_step_plan_report(&plan);
        assert!(lint.is_pass(), "{}", lint.primary_message());
        let selection =
            crate::planner::fix_diagnostics::repair_target_from_prompt(&plan.steps[0].instruction)
                .expect("contract target");
        assert_eq!(selection.selected_targets, ["src/app/page.tsx"]);
        assert_eq!(
            selection.selection_reason,
            RepairTargetSelectionReason::ContractAttribute
        );
    }

    #[test]
    fn failed_hook_f1_binds_context_before_phase_two() {
        let root = workspace();
        let cwd = root.path().to_string_lossy().to_string();
        let mut config = crate::config::Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--offline",
            "--profile",
            "nextjs",
            "--ultra-plan",
            "fix missing restart hook",
        ]))
        .unwrap();
        config.eval_events_path = Some(root.path().join("events.jsonl"));
        let plan = UltraPlan {
            goal: "fix missing restart hook".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "fix".to_string(),
            phases: vec![
                UltraPhase {
                    id: "reproduce-before".to_string(),
                    prompt: "Reproduce the hook failure.".to_string(),
                },
                UltraPhase {
                    id: "isolate-cause".to_string(),
                    prompt: "Isolate the predicate failure.".to_string(),
                },
            ],
        };
        let command = restart_command();
        let before = StepPlan {
            goal: "reproduce".to_string(),
            steps: vec![PlanStep {
                id: "reproduce-before".to_string(),
                kind: "verify".to_string(),
                expected_result: "fail".to_string(),
                instruction: "Run the route-bound predicate.".to_string(),
                expected_paths: Vec::new(),
                verify: vec![command],
            }],
        };
        let mut runtime =
            crate::planner::fix_runtime::FixRuntime::for_plan(&plan, &config).unwrap();

        runtime
            .run_before_phase(&before, &config, &plan, &plan.phases[0], 0)
            .unwrap();

        let prompt = attach_to_phase_prompt(
            &plan.phases[1],
            runtime.contract_predicate(),
            "phase two".to_string(),
        );
        assert!(prompt.contains("src/app/page.tsx"));
        assert!(prompt.contains(r#"data-anvil-action="restart""#));
        let events = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap();
        assert_eq!(
            events.matches("contract_attribute_repair_guidance").count(),
            1
        );
    }
}
