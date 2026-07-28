use serde_json::json;

use crate::config::{Config, PlanPreset};
use crate::planner::adjudication::contract::IntentId;
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

const IMPLEMENT_PATHS: [&str; 2] = ["pipeline/main.py", "output/inspection.json"];
const RUN_OUTPUT_PATHS: [&str; 2] = ["output/records.json", "output/report.md"];
const RUN_COMMAND: &str = "python3 -B pipeline/main.py";

// INGEST-4 canonical-default machine-floor audit baseline (table and boundaries:
// workspace/management/runs/uat-test0726-ingest-elev-003/floor-audit.md).
// INGEST-6 adds the bounded source-material row below without rewriting that
// historical campaign record.
//
// | floor | canonicality | production binding / state |
// |---|---|---|
// | preset selection | machine-fixed | Config default_create_ingest / closed |
// | UltraPlan + phase order | machine-fixed | ingest manifest PHASE_IDS / closed |
// | phase StepPlan source | machine-fixed | phase_plan_synthesis dispatch / closed |
// | implement guidance | literal guidance distributed | GENERATION_RULES / closed |
// | source structure material | machine-fixed, bounded | snapshot_structure injection / closed |
// | expected_paths ownership | machine-fixed | model-authored IMPLEMENT_PATHS / closed |
// | phase x expected artifact x producer | machine-fixed | pipeline-produced RUN_OUTPUT_PATHS checked after RUN_COMMAND / closed |
// | verifier artifact exclusion | machine-fixed | two-path implement closed set / closed |
// | run + structure commands | machine-fixed | RUN_COMMAND + run output postconditions + phase_verify / closed |
// | finalizer + lint | machine-fixed | finalize_step_plan_for_execution / closed |
// | expected-path execution | machine-fixed | generic ownership verifier / closed |
// | command classification | machine-fixed | verify::dependency_classification / closed |
// | execution progress | machine-fixed | execution_progress tracker / closed |
// | profile structure check | machine-fixed | IngestProfile::verify_final / closed |
// | N1-N5 activation | machine-fixed | IngestProfile::behavior_probe / closed |
// | N adapters + freeze | machine-fixed | ingest manifest + runtime / closed |
// | N evidence + assurance | machine-fixed | ingest runtime classifier / closed |
// | repair boundary | machine-fixed | ingest source/evidence target paths / closed |
// | assurance projection | machine-fixed | completion_metadata::ingest / closed |
// | admission cap | machine-fixed | profile_admission draft cap / closed |
//
// No canonical-default floor is planner-derived. The executor model still owns
// delivery content, by design; explicit `--plan-preset none` is an operator
// opt-out and is outside this default production path.
pub(crate) fn resolve_phase_plan(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    fallback: impl FnOnce() -> anyhow::Result<StepPlan>,
) -> anyhow::Result<StepPlan> {
    if !applies(config, plan) {
        return fallback();
    }
    ensure_shape(plan)?;
    let mut step_plan = match phase.id.as_str() {
        "ingest-implement" => implementation_plan(&plan.goal),
        "ingest-run" => run_plan(&plan.goal),
        "ingest-structural-gate" => structural_gate_plan(&plan.goal),
        other => anyhow::bail!("unsupported synthesized ingest phase: {other}"),
    };
    let report = crate::planner::step_plan_finalize::finalize_step_plan_for_execution(
        &mut step_plan,
        config,
    );
    if !report.is_pass() {
        anyhow::bail!(
            "synthesized ingest phase `{}` failed lint: {}",
            phase.id,
            report.primary_message()
        );
    }
    emit_synthesized(config, plan, phase, &step_plan);
    crate::tui::presentation::emit_step_plan_block(&phase.id, &step_plan, None);
    Ok(step_plan)
}

fn applies(config: &Config, plan: &UltraPlan) -> bool {
    config.plan_preset == PlanPreset::Profile
        && config.resolved_run_intent() == IntentId::Create
        && plan.intent == "create"
        && crate::planner::ultra_preset::is_profile_preset_plan(config, plan)
        && crate::planner::profile::domain_profile(&plan.profile).id() == "ingest"
}

fn ensure_shape(plan: &UltraPlan) -> anyhow::Result<()> {
    let actual = plan
        .phases
        .iter()
        .map(|phase| phase.id.as_str())
        .collect::<Vec<_>>();
    if actual != crate::planner::profiles::ingest::manifest::PHASE_IDS {
        anyhow::bail!("ingest synthesis requires fixed three-phase order; got {actual:?}");
    }
    Ok(())
}

fn implementation_plan(goal: &str) -> StepPlan {
    StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "implement-ingest-delivery".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!(
                "Create only the model-authored files pipeline/main.py and \
output/inspection.json. The following run phase executes the pipeline to generate \
output/records.json and output/report.md; do not hand-author those runtime outputs. \
Do not create any verification script.\n\n{}",
                crate::planner::profiles::ingest::guidance::GENERATION_RULES
            ),
            expected_paths: IMPLEMENT_PATHS.into_iter().map(str::to_string).collect(),
            verify: Vec::new(),
        }],
    }
}

fn run_plan(goal: &str) -> StepPlan {
    StepPlan {
        goal: goal.to_string(),
        steps: vec![
            PlanStep {
                id: "run-ingest-pipeline".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction:
                    "Run the machine-fixed offline pipeline command without changing files or substituting another command."
                        .to_string(),
                expected_paths: Vec::new(),
                verify: vec![RUN_COMMAND.to_string()],
            },
            PlanStep {
                id: "verify-ingest-run-outputs".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "After the machine-fixed pipeline command has completed, verify that it generated output/records.json and output/report.md. Do not replace pipeline execution with hand-authored outputs."
                    .to_string(),
                expected_paths: Vec::new(),
                verify: run_output_verify_commands(),
            },
        ],
    }
}

fn run_output_verify_commands() -> Vec<String> {
    RUN_OUTPUT_PATHS
        .into_iter()
        .map(|path| format!("test -f {path}"))
        .collect()
}

fn structural_gate_plan(goal: &str) -> StepPlan {
    StepPlan {
        goal: goal.to_string(),
        steps: vec![crate::planner::profiles::ingest::phase_verify::structure_check_step()],
    }
}

fn emit_synthesized(config: &Config, plan: &UltraPlan, phase: &UltraPhase, step_plan: &StepPlan) {
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ingest_plan_synthesized",
            "profile": plan.profile,
            "intent": plan.intent,
            "phase_id": phase.id,
            "template_id": format!("ingest-create-{}", phase.id),
            "planner_skipped": true,
            "expected_paths": step_plan.steps.iter().flat_map(|step| step.expected_paths.iter()).cloned().collect::<Vec<_>>(),
            "verify": step_plan.steps.iter().flat_map(|step| step.verify.iter()).cloned().collect::<Vec<_>>(),
        }),
    );
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    const MEASURED_GOAL: &str = "data/snapshots/ 配下のHTMLから自治体イベント情報を抽出し、JSON形式（フィールド: name, date (YYYY-MM-DD), location, source_file）で output/records.json に整形してください。候補検出に用いる決定的セレクタを宣言し、抽出できない候補は理由を明記して除外してください。output/report.md に件数と要約を記載してください。";
    const MEASURED_GAPS: &str =
        include_str!("../../tests/fixtures/ingest-plan-synthesis/elev-003-gaps.yaml");
    const MEASURED_ELEV_004_GAP: &str =
        include_str!("../../tests/fixtures/ingest-plan-synthesis/elev-004-gaps.yaml");
    const MEASURED_ELEV_005_GAP: &str =
        include_str!("../../tests/fixtures/ingest-plan-synthesis/elev-005-gaps.yaml");
    const PRESET_SNAPSHOT: &str =
        include_str!("../../tests/fixtures/ingest-plan-synthesis/canonical-preset.yaml");
    const LIST_HTML: &str = include_str!(
        "../../workspace/management/bench/assets/ingest/list/data/snapshots/events-list.html"
    );
    const TABLE_HTML: &str = include_str!(
        "../../workspace/management/bench/assets/ingest/table/data/snapshots/events-table.html"
    );

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Snapshot {
        phase_order: Vec<String>,
        plans: Vec<PlanProjection>,
        implement_guidance: ImplementGuidanceProjection,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct PlanProjection {
        phase_id: String,
        step_id: String,
        kind: String,
        expected_paths: Vec<String>,
        verify: Vec<String>,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct ImplementGuidanceProjection {
        snapshot_files: Vec<String>,
        candidate_structure_markers: Vec<String>,
        selector_derivation_rule: bool,
    }

    fn config() -> Config {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--intent",
            "create",
            "--profile",
            "ingest",
            "--ultra-plan",
            MEASURED_GOAL,
        ]))
        .unwrap()
    }

    fn config_with_snapshot(filename: &str, content: &str) -> (tempfile::TempDir, Config) {
        let dir = tempfile::tempdir().unwrap();
        let snapshots = dir.path().join("data/snapshots");
        std::fs::create_dir_all(&snapshots).unwrap();
        std::fs::write(snapshots.join(filename), content).unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--intent",
            "create",
            "--profile",
            "ingest",
            "--ultra-plan",
            MEASURED_GOAL,
        ]))
        .unwrap();
        (dir, config)
    }

    fn guidance_projection(instruction: &str) -> ImplementGuidanceProjection {
        let mut candidate_structure_markers = Vec::new();
        for marker in instruction.lines().filter_map(|line| {
            line.split_once(" | ")
                .map(|(_, content)| content.trim())
                .filter(|content| content.starts_with("<article ") || content.starts_with("<tr "))
                .map(str::to_string)
        }) {
            if !candidate_structure_markers.contains(&marker) {
                candidate_structure_markers.push(marker);
            }
            if candidate_structure_markers.len() == 2 {
                break;
            }
        }
        ImplementGuidanceProjection {
            snapshot_files: instruction
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("Snapshot file: ")
                        .and_then(|rest| rest.split_once(" ("))
                        .map(|(path, _)| path.to_string())
                })
                .collect(),
            candidate_structure_markers,
            selector_derivation_rule: instruction.contains(
                crate::planner::profiles::ingest::snapshot_structure::SELECTOR_DERIVATION_RULE,
            ),
        }
    }

    #[test]
    fn default_create_ingest_synthesizes_every_plan_without_model_fallback() {
        let (_workspace, mut config) = config_with_snapshot("events-list.html", LIST_HTML);
        let events = config.workspace_root.join("events.jsonl");
        config.eval_events_path = Some(events.clone());
        assert_eq!(config.plan_preset, PlanPreset::Profile);
        assert_eq!(config.plan_preset_origin(), "default_create_ingest");
        let plan = crate::planner::ultra_preset::maybe_prebuilt_ultra_plan(
            &config,
            MEASURED_GOAL,
            "create",
        )
        .unwrap()
        .unwrap();
        let mut generated_plans = plan
            .phases
            .iter()
            .map(|phase| {
                resolve_phase_plan(&config, &plan, phase, || {
                    panic!("ingest preset must not call the planner fallback")
                })
                .unwrap()
            })
            .collect::<Vec<_>>();
        crate::planner::step_material::inject(&config, &mut generated_plans[0].steps[0]).unwrap();
        let generated = generated_plans
            .iter()
            .zip(&plan.phases)
            .flat_map(|(step_plan, phase)| {
                step_plan
                    .steps
                    .iter()
                    .map(|step| PlanProjection {
                        phase_id: phase.id.clone(),
                        step_id: step.id.clone(),
                        kind: step.kind.clone(),
                        expected_paths: step.expected_paths.clone(),
                        verify: step.verify.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let expected: Snapshot = serde_yaml::from_str(PRESET_SNAPSHOT).unwrap();
        assert_eq!(
            expected,
            Snapshot {
                phase_order: plan.phases.iter().map(|phase| phase.id.clone()).collect(),
                plans: generated,
                implement_guidance: guidance_projection(&generated_plans[0].steps[0].instruction),
            }
        );

        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains("\"event\":\"preset_ultra_plan_used\""));
        assert_eq!(
            event_text
                .matches("\"event\":\"ingest_plan_synthesized\"")
                .count(),
            3
        );
        assert_eq!(
            event_text
                .matches("\"event\":\"ingest_snapshot_structure_injected\"")
                .count(),
            1
        );
        assert!(event_text.contains("\"relative_path\":\"data/snapshots/events-list.html\""));
        assert!(event_text.contains("\"candidate_windows\":2"));
        assert_eq!(event_text.matches("\"planner_skipped\":true").count(), 4);
    }

    #[test]
    fn synthesized_implementation_owns_only_model_authored_artifacts_and_literal_guidance() {
        let (_workspace, config) = config_with_snapshot("events-list.html", LIST_HTML);
        let plan = crate::planner::profiles::ingest::manifest::preset_ultra_plan(
            MEASURED_GOAL,
            "default",
            "create",
        )
        .unwrap();
        let mut generated =
            resolve_phase_plan(&config, &plan, &plan.phases[0], || panic!("model fallback"))
                .unwrap();
        crate::planner::step_material::inject(&config, &mut generated.steps[0]).unwrap();
        let step = &generated.steps[0];
        assert_eq!(
            step.expected_paths,
            IMPLEMENT_PATHS
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert!(step.verify.is_empty());
        for marker in [
            crate::planner::profiles::ingest::guidance::SELECTOR_LITERAL,
            crate::planner::profiles::ingest::guidance::INSPECTION_LITERAL,
            crate::planner::profiles::ingest::guidance::RECORDS_LITERAL,
            "examples only",
            "actual snapshots",
            "only the model-authored files",
            "following run phase",
            "Do not create any verification script",
            "Snapshot file: data/snapshots/events-list.html",
            "<article class=\"event\" id=\"list-01\">",
            crate::planner::profiles::ingest::snapshot_structure::SELECTOR_DERIVATION_RULE,
        ] {
            assert!(step.instruction.contains(marker), "missing {marker}");
        }
        for runtime_output in RUN_OUTPUT_PATHS {
            assert!(
                !step
                    .expected_paths
                    .iter()
                    .any(|path| path == runtime_output)
            );
        }
    }

    #[test]
    fn elev_005_unread_selector_shape_receives_measured_snapshot_structure() {
        for measured in [
            "uat-test0726-ingest-elev-005",
            "snapshot_content_reads: 0",
            "tr.event-row",
            ".event-item",
            "div.event-item",
            "div.event-card",
            "actual_candidate: tbody > tr",
            "detected: 0",
        ] {
            assert!(
                MEASURED_ELEV_005_GAP.contains(measured),
                "fixture lacks {measured}"
            );
        }

        let (_workspace, config) = config_with_snapshot("events-table.html", TABLE_HTML);
        let plan = crate::planner::profiles::ingest::manifest::preset_ultra_plan(
            MEASURED_GOAL,
            "default",
            "create",
        )
        .unwrap();
        let mut generated =
            resolve_phase_plan(&config, &plan, &plan.phases[0], || panic!("model fallback"))
                .unwrap();
        crate::planner::step_material::inject(&config, &mut generated.steps[0]).unwrap();
        let instruction = &generated.steps[0].instruction;
        for marker in [
            "Snapshot file: data/snapshots/events-table.html",
            "L0018 |       <tbody>",
            "L0019 |         <tr id=\"table-01\">",
            "HTML tag=tr occurrences=10",
            crate::planner::profiles::ingest::snapshot_structure::SELECTOR_DERIVATION_RULE,
        ] {
            assert!(instruction.contains(marker), "missing {marker}");
        }
        for stale_selector in [".event-item", "div.event-item", "div.event-card"] {
            assert!(
                !instruction.contains(stale_selector),
                "measured stale selector leaked into guidance: {stale_selector}"
            );
        }
    }

    #[test]
    fn elev_003_planner_gaps_are_unrepresentable_in_the_machine_preset() {
        for measured in [
            "uat-test0726-ingest-elev-003",
            "55cb2e0921ddd64414a46073f2013fa1f3a64c251783f3e77d06122bd878145c",
            "a2aa7d3a1504ba175e21013825f601415c9cf11755157e4ae73462b92f0f9015",
            "smoke-check.js",
            "verify-artifacts.js",
            "Repair or finalize pipeline/main.py",
        ] {
            assert!(MEASURED_GAPS.contains(measured), "fixture lacks {measured}");
        }

        let config = config();
        let plan = crate::planner::profiles::ingest::manifest::preset_ultra_plan(
            MEASURED_GOAL,
            "default",
            "create",
        )
        .unwrap();
        let generated = plan
            .phases
            .iter()
            .map(|phase| {
                resolve_phase_plan(&config, &plan, phase, || panic!("model fallback")).unwrap()
            })
            .collect::<Vec<_>>();
        let canonical = serde_yaml::to_string(&generated).unwrap();
        for forbidden in [
            "smoke-check.py",
            "verify_pipeline.py",
            "smoke-check.js",
            "verify-artifacts.js",
            "Repair or finalize pipeline/main.py",
        ] {
            assert!(
                !canonical.contains(forbidden),
                "preset retained {forbidden}"
            );
        }
        assert_eq!(generated[1].steps[0].verify, [RUN_COMMAND.to_string()]);
        assert!(generated[1].steps[1].expected_paths.is_empty());
        assert_eq!(generated[1].steps[1].verify, run_output_verify_commands());
        assert_eq!(
            generated[2].steps[0].verify,
            [crate::planner::profiles::ingest::phase_verify::CHECK_COMMAND.to_string()]
        );
    }

    #[test]
    fn elev_004_runtime_outputs_are_checked_only_after_pipeline_execution() {
        for measured in [
            "uat-test0726-ingest-elev-004",
            "7f9101db36a4172e781125790dd69fd4573769564d32625eccc2e77fdc9df548",
            "output/records.json",
            "output/report.md",
            "formal_runs: 6",
            "run_phase_reached: 0",
        ] {
            assert!(
                MEASURED_ELEV_004_GAP.contains(measured),
                "fixture lacks {measured}"
            );
        }

        let config = config();
        let plan = crate::planner::profiles::ingest::manifest::preset_ultra_plan(
            MEASURED_GOAL,
            "default",
            "create",
        )
        .unwrap();
        let implement =
            resolve_phase_plan(&config, &plan, &plan.phases[0], || panic!("model fallback"))
                .unwrap();
        let run = resolve_phase_plan(&config, &plan, &plan.phases[1], || panic!("model fallback"))
            .unwrap();
        assert_eq!(
            implement.steps[0].expected_paths,
            IMPLEMENT_PATHS
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(run.steps.len(), 2);
        assert_eq!(run.steps[0].verify, [RUN_COMMAND.to_string()]);
        assert!(run.steps[0].expected_paths.is_empty());
        assert!(run.steps[1].expected_paths.is_empty());
        assert_eq!(run.steps[1].verify, run_output_verify_commands());

        let generated = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(generated.path().join("pipeline")).unwrap();
        std::fs::write(
            generated.path().join("pipeline/main.py"),
            "from pathlib import Path\nPath('output').mkdir(exist_ok=True)\nPath('output/records.json').write_text('[]')\nPath('output/report.md').write_text('# report\\n')\n",
        )
        .unwrap();
        assert!(crate::planner::verify::verify_step(generated.path(), &run.steps[0]).is_pass());
        assert!(crate::planner::verify::verify_step(generated.path(), &run.steps[1]).is_pass());

        let missing = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(missing.path().join("pipeline")).unwrap();
        std::fs::write(missing.path().join("pipeline/main.py"), "pass\n").unwrap();
        assert!(crate::planner::verify::verify_step(missing.path(), &run.steps[0]).is_pass());
        let report = crate::planner::verify::verify_step(missing.path(), &run.steps[1]);
        assert!(!report.is_pass());
        assert!(report.missing_paths.is_empty());
        assert_eq!(report.command_failures.len(), RUN_OUTPUT_PATHS.len());
        for output in RUN_OUTPUT_PATHS {
            assert!(
                report
                    .command_failures
                    .iter()
                    .any(|failure| failure.command.contains(output)),
                "missing post-run failure for {output}"
            );
        }
    }

    #[test]
    fn other_plan_shapes_still_use_the_existing_fallback() {
        let config = config();
        let mut plan = crate::planner::profiles::ingest::manifest::preset_ultra_plan(
            MEASURED_GOAL,
            "default",
            "create",
        )
        .unwrap();
        plan.profile = "data".to_string();
        let fallback = StepPlan::single("fallback");
        assert_eq!(
            resolve_phase_plan(&config, &plan, &plan.phases[0], || Ok(fallback.clone())).unwrap(),
            fallback
        );
    }
}
