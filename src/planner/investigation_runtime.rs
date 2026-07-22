use serde_json::json;

use crate::config::Config;
use crate::planner::adjudication::contract::{
    EvidenceStage, ExpectedOutcome, IntentId, ProbeOutcome,
};
use crate::planner::adjudication::investigate::{
    InvestigationAssurance, InvestigationBindingEvidence, InvestigationRunEvidence,
    evaluate_investigation_evidence,
};
use crate::planner::step_plan::{ExpectedResult, StepKind, StepPlan};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

pub(crate) struct InvestigationRuntime {
    run_id: String,
    run: Option<InvestigationRunEvidence>,
}

pub(crate) enum InvestigationBeforeOutcome {
    Confirmed,
    RebuildRequired { feedback: String },
}

impl InvestigationRuntime {
    pub(crate) fn for_plan(plan: &UltraPlan, config: &Config) -> Option<Self> {
        (config.intent_override == Some(IntentId::Investigate) && plan.intent == "investigate")
            .then(|| Self {
                run_id: uuid::Uuid::now_v7().to_string(),
                run: None,
            })
    }

    pub(crate) const fn is_reproducer_phase(&self, index: usize) -> bool {
        index == 0
    }

    pub(crate) fn run_reproducer_phase(
        &mut self,
        step_plan: &StepPlan,
        config: &Config,
        plan: &UltraPlan,
        phase: &UltraPhase,
        index: usize,
    ) -> anyhow::Result<InvestigationBeforeOutcome> {
        let command = extract_reproducer(step_plan)?;
        let lineage = crate::planner::external_reproducer::lineage_for_execution(config, &command)
            .map_err(anyhow::Error::msg)?;
        let execution = crate::planner::fix_diagnostics::run_reproducer(
            config,
            &self.run_id,
            "reproducer_fails",
            EvidenceStage::Diagnosis,
            ExpectedOutcome::Failure,
            1,
            &command,
            &lineage,
            &plan.profile,
            &plan.goal,
        );
        let mut run = InvestigationRunEvidence::new(&command, 1, execution.evidence.outcome);
        if crate::planner::external_reproducer::is_workflow_node(config) {
            run.reproducer_lineage = lineage;
        }
        run.stderr = execution.evidence.reason.clone();
        run.failure_classification = execution.evidence.failure_classification;
        let evidence_dir = config.workspace_root.join("evidence");
        std::fs::create_dir_all(&evidence_dir)?;
        let evidence_name = if run.failure_classification.is_reproducer_defect() {
            "investigation-run-attempt-1.json"
        } else {
            "investigation-run.json"
        };
        std::fs::write(
            evidence_dir.join(evidence_name),
            serde_json::to_vec_pretty(&run)?,
        )?;
        if run.failure_classification.is_reproducer_defect() {
            return Ok(InvestigationBeforeOutcome::RebuildRequired {
                feedback: format!(
                    "reproducer_defect: {}. Rebuild exactly one deterministic verify command.",
                    run.stderr
                ),
            });
        }
        self.run = Some(run.clone());
        if run.outcome != ProbeOutcome::Failure {
            anyhow::bail!("investigation baseline gate failed: baseline_not_reproduced");
        }
        emit_direct_phase_complete(config, plan, phase, index);
        Ok(InvestigationBeforeOutcome::Confirmed)
    }

    pub(crate) fn finish(self, config: &Config, plan: &UltraPlan) -> anyhow::Result<String> {
        let diagnosis_path = config.workspace_root.join("output/diagnosis.md");
        let report_written = diagnosis_path.is_file();
        let binding_path = config
            .workspace_root
            .join("evidence/investigation-binding.json");
        let binding = binding_path
            .is_file()
            .then(|| std::fs::read(&binding_path))
            .transpose()?
            .map(|bytes| serde_json::from_slice::<InvestigationBindingEvidence>(&bytes))
            .transpose()?;
        let adjudication =
            evaluate_investigation_evidence(report_written, self.run.as_ref(), binding.as_ref());
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "investigation_adjudicated",
                "profile": plan.profile,
                "assurance_level": format!("{:?}", adjudication.assurance).to_ascii_lowercase(),
                "assurance_reason": adjudication.reason,
                "evidence_paths": [
                    "evidence/investigation-run.json",
                    "evidence/investigation-binding.json"
                ],
            }),
        );
        if adjudication.assurance == InvestigationAssurance::Failed {
            anyhow::bail!(
                "investigation final acceptance failed: {}",
                adjudication.reason
            );
        }
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "ultra_plan_complete",
                "total_phases": plan.phases.len(),
                "profile": plan.profile,
                "intent": "investigate",
                "assurance_level": format!("{:?}", adjudication.assurance).to_ascii_lowercase(),
                "assurance_reason": adjudication.reason,
                "ok": true,
            }),
        );
        Ok(format!(
            "ultra-plan-run complete: {} phases",
            plan.phases.len()
        ))
    }
}

fn extract_reproducer(plan: &StepPlan) -> anyhow::Result<String> {
    let mut commands = plan
        .steps
        .iter()
        .filter(|step| {
            step.step_kind() == StepKind::Verify
                && step.expected_result_kind() == ExpectedResult::Fail
                && step.verify.len() == 1
        })
        .map(|step| step.verify[0].trim());
    let command = commands
        .next()
        .filter(|command| !command.is_empty())
        .ok_or_else(|| anyhow::anyhow!("investigation reproducer was not identified"))?;
    if commands.next().is_some() {
        anyhow::bail!("investigation reproducer must contain exactly one command");
    }
    Ok(crate::planner::verify::normalize_verify_command(command)?.into_string())
}

fn emit_direct_phase_complete(config: &Config, plan: &UltraPlan, phase: &UltraPhase, index: usize) {
    for (event, stage) in [
        (
            "ultra_phase_execute_complete",
            "investigation_reproducer_probe",
        ),
        (
            "ultra_phase_profile_check",
            "investigation_reproducer_observed",
        ),
        ("ultra_phase_complete", "complete"),
    ] {
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": event,
                "phase_id": phase.id,
                "phase_index": index + 1,
                "total_phases": plan.phases.len(),
                "final_phase": false,
                "stage": stage,
                "ok": true,
                "reason": "",
                "step_count": 1,
            }),
        );
    }
}
