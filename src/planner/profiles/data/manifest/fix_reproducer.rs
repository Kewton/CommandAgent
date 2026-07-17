use crate::planner::capability_catalog::{InternalCapability, ProbeCapability, ResolvedCapability};
use crate::planner::profile::ProfileFixReproducerSuggestion;
use crate::planner::profile_manifest::CheckBinding;

pub(crate) fn suggestion_for(goal: &str) -> Option<ProfileFixReproducerSuggestion> {
    let lower = goal.to_ascii_lowercase();
    contract_suggestion(&lower).or_else(|| pipeline_suggestion(&lower))
}

fn contract_suggestion(goal: &str) -> Option<ProfileFixReproducerSuggestion> {
    let inspection = contains_any(
        goal,
        &[
            "inspection",
            "inspection.json",
            "data_inspection_schema",
            "データ検分",
            "検分結果",
        ],
    );
    let mut ids = Vec::new();
    if inspection {
        ids.push("data_inspection_schema");
    }
    if contains_any(
        goal,
        &[
            "results.json",
            "results schema",
            "results_schema",
            "data_results_schema",
            "結果スキーマ",
        ],
    ) || (!inspection && contains_any(goal, &["schema", "スキーマ"]))
    {
        ids.push("data_results_schema");
    }
    if contains_any(
        goal,
        &[
            "reconciliation",
            "data_reconciliation",
            "row accounting",
            "行数整合",
            "行整合",
            "リコンシリエーション",
        ],
    ) {
        ids.push("data_reconciliation");
    }
    if contains_any(
        goal,
        &[
            "claims binding",
            "claims_binding",
            "data_claims_binding",
            "claim binding",
            "数値照合",
            "照合",
        ],
    ) {
        ids.push("data_claims_binding");
    }
    if ids.is_empty() {
        return None;
    }
    let suggestions = ids
        .iter()
        .map(|id| internal_check_suggestion(id))
        .collect::<Option<Vec<_>>>()?;
    Some(ProfileFixReproducerSuggestion {
        basis: format!("goal_profile_contract:{}", ids.join(",")),
        suggestion: suggestions.join(" | "),
    })
}

fn pipeline_suggestion(goal: &str) -> Option<ProfileFixReproducerSuggestion> {
    if !contains_any(
        goal,
        &[
            "pipeline_probe",
            "pipeline failure",
            "pipeline error",
            "execution error",
            "runtime error",
            "traceback",
            "nonzero exit",
            "non-zero exit",
            "non zero exit",
            "exit非ゼロ",
            "実行エラー",
            "トレースバック",
            "非ゼロ終了",
            "終了コードが非ゼロ",
        ],
    ) {
        return None;
    }
    let check = manifest_check("pipeline_probe")?;
    let ResolvedCapability::Probe(ProbeCapability::Pipeline { entry, .. }) =
        crate::planner::capability_catalog::resolve(&check.id, &check.params).ok()?
    else {
        return None;
    };
    Some(ProfileFixReproducerSuggestion {
        basis: "goal_failure_kind:pipeline_execution".to_string(),
        suggestion: format!("profile_catalog:pipeline_probe(entry={entry}) => python3 -B {entry}"),
    })
}

fn internal_check_suggestion(id: &str) -> Option<String> {
    let check = manifest_check(id)?;
    if !matches!(
        crate::planner::capability_catalog::resolve(&check.id, &check.params).ok()?,
        ResolvedCapability::Internal(InternalCapability::Data(_))
    ) {
        return None;
    }
    Some(format!(
        "profile_catalog:{id} => {}",
        super::super::step_policy::catalog_check_command(id)
    ))
}

fn manifest_check(id: &str) -> Option<&'static CheckBinding> {
    super::get()
        .checks
        .values()
        .flatten()
        .find(|check| check.id == id)
}

fn contains_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::config::Config;
    use crate::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome, ProbeOutcome};
    use crate::planner::fix_diagnostics;
    use crate::planner::profile::{ProfileFixRegressionAdapter, profile_fix_regression_bindings};
    use crate::planner::repair_targeting::{
        RepairTargetPriority, RepairTargetResolutionInput, RepairTargetSelectionReason,
        resolve_repair_targets,
    };
    use crate::planner::step_plan::{PlanStep, StepPlan};
    use crate::planner::ultra_plan::UltraPhase;
    use crate::tools::bash::{BashOutcome, BashOutcomeKind};
    use clap::Parser;

    #[test]
    fn pipeline_failure_goal_suggests_canonical_probe_command() {
        assert_eq!(
            suggestion_for("pipeline/main.py が traceback を出して実行エラーになります。")
                .unwrap(),
            ProfileFixReproducerSuggestion {
                basis: "goal_failure_kind:pipeline_execution".to_string(),
                suggestion: "profile_catalog:pipeline_probe(entry=pipeline/main.py) => python3 -B pipeline/main.py".to_string(),
            }
        );
    }

    #[test]
    fn contract_artifact_goal_suggests_each_named_catalog_check() {
        assert_eq!(
            suggestion_for(
                "results.json のスキーマ、reconciliation、レポート数値の照合を修正してください。"
            )
            .unwrap(),
            ProfileFixReproducerSuggestion {
                basis: "goal_profile_contract:data_results_schema,data_reconciliation,data_claims_binding".to_string(),
                suggestion: "profile_catalog:data_results_schema => anvil-catalog-check:data_results_schema | profile_catalog:data_reconciliation => anvil-catalog-check:data_reconciliation | profile_catalog:data_claims_binding => anvil-catalog-check:data_claims_binding".to_string(),
            }
        );
    }

    #[test]
    fn inspection_goal_suggests_phase_scoped_schema_check() {
        assert_eq!(
            suggestion_for("output/inspection.json の inspection schema 違反を修正してください。")
                .unwrap(),
            ProfileFixReproducerSuggestion {
                basis: "goal_profile_contract:data_inspection_schema".to_string(),
                suggestion: "profile_catalog:data_inspection_schema => anvil-catalog-check:data_inspection_schema".to_string(),
            }
        );
    }

    #[test]
    fn data_fix_plan_generation_matches_corpus_snapshot() {
        const GOAL: &str =
            "The data pipeline exits non-zero with a traceback. Fix it and preserve validations.";
        let plan = crate::planner::intent::explicit_fix_plan(GOAL, "data", "default");
        let suggestion = suggestion_for(GOAL).unwrap();
        let execution_prompt = crate::planner::fix_reproducer::attach_to_phase_prompt(
            &plan,
            &plan.phases[0],
            None,
            plan.phases[0].prompt.clone(),
        );
        let snapshot = format!(
            "profile: {}\nintent: {}\nphase_ids: {}\nfirst_phase_prompt: {}\nreproducer_basis: {}\nreproducer_suggestion: {}\nfirst_phase_execution_prompt:\n{}\n",
            plan.profile,
            plan.intent,
            plan.phases
                .iter()
                .map(|phase| phase.id.as_str())
                .collect::<Vec<_>>()
                .join(" -> "),
            plan.phases[0].prompt,
            suggestion.basis,
            suggestion.suggestion,
            execution_prompt,
        );

        assert_eq!(
            snapshot,
            include_str!(
                "../../../../../tests/corpus/apps/test0717_d2a_data_fix_plan_generation/fixtures/data-fix-plan.txt"
            )
        );
    }

    #[test]
    fn data_f3_binding_snapshot_is_pipeline_plus_final_bound_e1_to_e4() {
        let root = tempfile::tempdir().unwrap();
        let bindings = profile_fix_regression_bindings(root.path(), "data", "fix pipeline");
        let snapshot = bindings
            .iter()
            .map(|binding| (binding.id.clone(), binding.adapter.clone()))
            .collect::<Vec<_>>();

        assert_eq!(
            snapshot,
            [
                (
                    "pipeline_probe".to_string(),
                    ProfileFixRegressionAdapter::DataManifestCheck
                ),
                (
                    "data_reconciliation".to_string(),
                    ProfileFixRegressionAdapter::DataManifestCheck
                ),
                (
                    "data_claims_binding".to_string(),
                    ProfileFixRegressionAdapter::DataManifestCheck
                ),
                (
                    "data_rerun_consistency".to_string(),
                    ProfileFixRegressionAdapter::DataManifestCheck
                ),
                (
                    "data_results_schema".to_string(),
                    ProfileFixRegressionAdapter::DataManifestCheck
                ),
            ]
        );
        assert!(
            !bindings
                .iter()
                .any(|binding| binding.id == "data_inspection_schema")
        );
    }

    #[test]
    fn internal_catalog_reproducer_executes_the_bound_check_before_and_after() {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().to_string_lossy().to_string();
        let config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--offline",
            "--profile",
            "data",
            "--intent",
            "fix",
            "--ultra-plan",
            "fix results schema",
        ]))
        .unwrap();
        let command = "anvil-catalog-check:data_results_schema";
        let before = fix_diagnostics::run_reproducer(
            &config,
            "d2a-catalog",
            "before_fails",
            EvidenceStage::Before,
            ExpectedOutcome::Failure,
            1,
            command,
            "reproducer:d2a-catalog",
            "data",
            "fix results schema",
        );
        assert_eq!(before.evidence.outcome, ProbeOutcome::Failure);
        assert!(!before.evidence.reason.contains("command not found"));

        std::fs::create_dir_all(root.path().join("output")).unwrap();
        std::fs::write(
            root.path().join("output/results.json"),
            r#"{"reconciliation":{"input_rows":1,"used_rows":1,"excluded":[]},"values":{"total":1.0}}"#,
        )
        .unwrap();
        let after = fix_diagnostics::run_reproducer(
            &config,
            "d2a-catalog",
            "after_passes",
            EvidenceStage::After,
            ExpectedOutcome::Success,
            2,
            command,
            "reproducer:d2a-catalog",
            "data",
            "fix results schema",
        );

        assert_eq!(after.evidence.outcome, ProbeOutcome::Success);
        assert_eq!(after.evidence.reason, "command_succeeded");
    }

    #[test]
    fn pipeline_traceback_reaches_phase_two_and_wins_fix_target_resolution() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::write(
            root.path().join("pipeline/main.py"),
            "raise ValueError('bad')\n",
        )
        .unwrap();
        let outcome = BashOutcome {
            kind: BashOutcomeKind::CommandFailed,
            status: Some("exit status: 1".to_string()),
            stdout: String::new(),
            stderr: "Traceback (most recent call last):\n  File \"pipeline/main.py\", line 17, in <module>\n    run()\nValueError: bad row\n".to_string(),
            elapsed_ms: 1,
            summary: "command failed".to_string(),
        };
        let diagnostic = fix_diagnostics::extract_failure_diagnostic(
            root.path(),
            "python3 -B pipeline/main.py",
            &outcome,
            None,
        )
        .expect("traceback diagnostic");
        let phase = UltraPhase {
            id: "repair".to_string(),
            prompt: "Repair the pipeline.".to_string(),
        };
        let mut plan = StepPlan {
            goal: "repair pipeline".to_string(),
            steps: vec![PlanStep {
                id: "repair".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Repair the pipeline.".to_string(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            }],
        };
        fix_diagnostics::bind_step_plan(&phase, Some(&diagnostic), &mut plan);
        let mapped = fix_diagnostics::repair_target_from_prompt(&plan.steps[0].instruction)
            .expect("mapped repair target");
        let selection = resolve_repair_targets(RepairTargetResolutionInput {
            root: root.path(),
            profile: "data",
            pending_evidence: &["data_results_schema".to_string()],
            missing_capabilities: &[],
            contract_attribute_paths: &[],
            repair_changed_paths: &[],
            required_paths: &["output/results.json".to_string()],
            fallback_paths: &[],
            mapped_selection: Some(&mapped),
            priority: RepairTargetPriority::FixIntent,
        })
        .unwrap();

        assert_eq!(diagnostic.target_path, "pipeline/main.py");
        assert_eq!(diagnostic.line, 17);
        assert_eq!(
            diagnostic.selection_reason,
            RepairTargetSelectionReason::TracebackMapped
        );
        assert!(plan.steps[0].instruction.contains("ValueError"));
        assert_eq!(plan.steps[0].expected_paths, ["pipeline/main.py"]);
        assert_eq!(selection.primary_target(), Some("pipeline/main.py"));
        assert_eq!(
            selection.selection_reason,
            RepairTargetSelectionReason::TracebackMapped
        );
    }
}
