//! Exercise the production preset, full prompt renderer and registered selector.
use super::super::*;
use crate::planner::profiles::nextjs;

const R0: &str = include_str!("../../../../tests/corpus/apps/nextjs-domain-preset-routing/r0.json");

fn r0() -> Value {
    serde_json::from_str(R0).unwrap()
}

fn rendered_selection(
    plan: &UltraPlan,
    phase: &UltraPhase,
    root: &Path,
    layout: PromptLayout,
) -> (
    String,
    Option<crate::planner::profile::ProfileDeterministicStepPlan>,
) {
    let mut cfg = config(root.to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.prompt_layout = layout;
    let prompt = ultra_phase_prompt(plan, phase, &cfg, &UltraRunContext::default(), None);
    // Driver selection passes the complete phase prompt as its goal, too.
    let selected =
        resolve_profile_runtime("nextjs").deterministic_step_plan(&prompt, root, &prompt);
    (prompt, selected)
}

#[test]
fn r0_full_core_preset_routes_to_planner() {
    let fixture = r0();
    let goal = fixture["goal"].as_str().unwrap();
    let runtime = resolve_profile_runtime("nextjs");
    let plan = runtime
        .preset_ultra_plan(goal, "default", "create")
        .unwrap();
    let core = &plan.phases[1];
    assert_eq!(core.id, fixture["phase_id"]);
    // Guidance is retained byte-for-byte; removing TypeScript/import/export
    // from the preset cannot make this regression test pass.
    assert_eq!(core.prompt, fixture["regressed_core_task"]);
    let dir = tempfile::tempdir().unwrap();
    for layout in [PromptLayout::Legacy, PromptLayout::Stable] {
        let (prompt, selected) = rendered_selection(&plan, core, dir.path(), layout);
        for path in fixture["observed_step_expected_paths"].as_array().unwrap() {
            assert!(prompt.contains(path.as_str().unwrap()), "{path}");
        }
        assert!(selected.is_none(), "{layout:?}: {selected:?}");
        let old_core = UltraPhase {
            id: core.id.clone(),
            prompt: fixture["old_core_task"].as_str().unwrap().to_string(),
        };
        assert!(
            rendered_selection(&plan, &old_core, dir.path(), layout)
                .1
                .is_none()
        );
    }
}

#[test]
fn all_four_preset_phases_preserve_business_and_game_intent() {
    let fixture = r0();
    let goals = [
        fixture["goal"].as_str().unwrap(),
        "Build a project management app with projects, dated tasks, assignees and status filters on port 60302 using TypeScript import/export contracts.",
        "Build a general business app with inventory, shifts and expenses; configure package scripts for port 60302.",
        "Build a TypeScript project management app with import/export contracts.",
        "在庫と受注を管理する",
        "スタッフとシフトを管理する",
        "部門と経費申請を管理する",
        "Create a Space Invaders game on port 60302 using TypeScript import/export modules.",
        "Create a Breakout game on port 60302 using TypeScript import/export modules.",
        "Create an interactive Quiz on port 60302 using TypeScript import/export modules.",
    ];
    let dir = tempfile::tempdir().unwrap();
    for goal in goals {
        let plan = resolve_profile_runtime("nextjs")
            .preset_ultra_plan(goal, "default", "create")
            .unwrap();
        assert_eq!(plan.phases.len(), 4);
        for layout in [PromptLayout::Legacy, PromptLayout::Stable] {
            for (phase, expected) in plan.phases.iter().zip([
                Some("nextjs-scaffold"),
                None,
                None,
                Some("nextjs-build-verification"),
            ]) {
                let (prompt, selected) = rendered_selection(&plan, phase, dir.path(), layout);
                assert!(prompt.contains("Profile generation rules:"));
                assert!(prompt.contains("Required final artifacts:"));
                assert!(prompt.contains(goal));
                assert!(prompt.contains("TypeScript"));
                assert_eq!(
                    selected
                        .as_ref()
                        .map(|selected| selected.template_id.as_str()),
                    expected,
                    "{goal} / {} / {layout:?}",
                    phase.id
                );
                if let Some(selected) = selected {
                    assert!(
                        selected.plan.steps[0]
                            .expected_paths
                            .contains(&"src/app/page.tsx".to_string())
                    );
                    assert!(
                        selected
                            .plan
                            .steps
                            .iter()
                            .flat_map(|s| &s.verify)
                            .any(|v| v == "npm run build")
                    );
                }
            }
        }
    }
}

#[test]
fn custom_phase_keywords_and_explicit_port_controls_use_real_templates() {
    let controls: Value = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/nextjs-domain-preset-routing/controls.json"
    ))
    .unwrap();
    let review: Value = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/nextjs-domain-preset-routing/review-controls.json"
    ))
    .unwrap();
    let goal = "Build a business app on port 60302 with TypeScript import/export contracts.";
    let dir = tempfile::tempdir().unwrap();
    let plan = resolve_profile_runtime("nextjs")
        .preset_ultra_plan(goal, "default", "create")
        .unwrap();
    for case in controls["cases"]
        .as_array()
        .unwrap()
        .iter()
        .chain(review["cases"].as_array().unwrap())
    {
        let phase = UltraPhase {
            id: case["phase_id"].as_str().unwrap().to_string(),
            prompt: case["task"].as_str().unwrap().to_string(),
        };
        for layout in [PromptLayout::Legacy, PromptLayout::Stable] {
            let (_, selected) = rendered_selection(&plan, &phase, dir.path(), layout);
            assert_eq!(
                selected.as_ref().map(|s| s.template_id.as_str()),
                case["template"].as_str(),
                "{} / {layout:?}",
                case["case"]
            );
            if case["template"] == "nextjs-port-scripts" {
                let selected = selected.unwrap();
                assert_eq!(selected.plan.steps.len(), 2);
                assert_eq!(selected.plan.steps[0].id, "configure-nextjs-port-scripts");
                // Full-context port extraction can still encounter the global
                // default first. Numeric resolution is outside this correction;
                // the isolated explicit-port case below checks its 60302 plan.
                assert!(
                    selected.plan.steps[0]
                        .instruction
                        .starts_with("Update package.json so scripts.dev runs next dev on port ")
                );
                assert_eq!(selected.plan.steps[1].id, "verify-nextjs-port-scripts");
                assert!(!selected.plan.steps[1].verify.is_empty());
            }
        }
    }
    // A genuinely package-only phase retains its package-only plan. The full
    // global artifact contract above is intentionally not weakened here.
    let selected = nextjs::deterministic_step_plan(
        "Phase id: package-scripts\nPhase task: Configure scripts for port 60302.",
        dir.path(),
        goal,
    )
    .unwrap();
    assert_eq!(selected.plan.steps[0].expected_paths, ["package.json"]);
    assert!(selected.plan.steps[0].instruction.contains("port 60302"));
    assert!(selected.plan.steps[1].verify[0].contains("60302"));
}

#[test]
fn frozen_r0_routing_provenance_is_intact() {
    use sha2::{Digest, Sha256};
    let provenance: Value = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/nextjs-domain-preset-routing/provenance.json"
    ))
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(R0.as_bytes())),
        provenance["fixture_sha256"]["r0.json"]
    );
    assert_eq!(
        provenance["frozen_from_commit"],
        "ab5c3d1461ede2a294a281dc56a5dc82ca97494e"
    );
}

#[test]
fn plural_implementation_intent_is_not_port_only() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/nextjs-domain-preset-routing/plural-controls.json"
    ))
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let plan = resolve_profile_runtime("nextjs")
        .preset_ultra_plan("Build an app on port 60302", "default", "create")
        .unwrap();
    let mut failures = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let phase = UltraPhase {
            id: fixture["phase_id"].as_str().unwrap().to_string(),
            prompt: case["task"].as_str().unwrap().to_string(),
        };
        for layout in [PromptLayout::Legacy, PromptLayout::Stable] {
            let (_, selected) = rendered_selection(&plan, &phase, dir.path(), layout);
            if let Some(selected) = selected {
                failures.push(format!(
                    "{} / {layout:?}: {}",
                    case["phrase"], selected.template_id
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn mixed_implementation_intent_is_not_setup_or_build_only() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/nextjs-domain-preset-routing/mixed-controls.json"
    ))
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let plan = resolve_profile_runtime("nextjs")
        .preset_ultra_plan("Build an app on port 60302", "default", "create")
        .unwrap();
    let mut failures = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let phase = UltraPhase {
            id: case["phase_id"].as_str().unwrap().to_string(),
            prompt: case["task"].as_str().unwrap().to_string(),
        };
        for layout in [PromptLayout::Legacy, PromptLayout::Stable] {
            let (_, selected) = rendered_selection(&plan, &phase, dir.path(), layout);
            if let Some(selected) = selected {
                failures.push(format!(
                    "{} / {} / {layout:?}: {}",
                    phase.id, phase.prompt, selected.template_id
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn legacy_implementation_veto_preserves_all_prior_keywords_and_inflections() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/nextjs-domain-preset-routing/legacy-implementation-keywords.json"
    )).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let plan = resolve_profile_runtime("nextjs")
        .preset_ultra_plan("Build an app", "default", "create")
        .unwrap();
    let mut tasks = fixture["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    for keyword in fixture["keywords"].as_array().unwrap() {
        let keyword = keyword.as_str().unwrap();
        assert!(
            nextjs::knowledge::get()
                .deterministic_keywords
                .implementation_phase
                .iter()
                .any(|current| current == keyword)
        );
        tasks.push(format!(
            "Implement {keyword} and configure package scripts for port 60302."
        ));
        tasks.push(format!(
            "Implement {keyword}_suffix and configure package scripts for port 60302."
        ));
    }
    for task in tasks {
        let phase = UltraPhase {
            id: "interaction".into(),
            prompt: task,
        };
        for layout in [PromptLayout::Legacy, PromptLayout::Stable] {
            assert!(
                rendered_selection(&plan, &phase, dir.path(), layout)
                    .1
                    .is_none(),
                "{} / {layout:?}",
                phase.prompt
            );
        }
    }
}
