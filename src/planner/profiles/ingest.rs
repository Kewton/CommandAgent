pub mod accounting;
pub(crate) mod guidance;
pub mod manifest;
pub(crate) mod phase_verify;
pub mod runtime;
pub mod source_binding;

use std::path::Path;

use crate::planner::profile::{
    DomainProfile, ProfileBehaviorProbeReport, ProfileQualityExpectations,
};
use crate::planner::ultra_plan::UltraPlan;
use crate::planner::verify::VerificationReport;

pub struct IngestProfile;

impl DomainProfile for IngestProfile {
    fn id(&self) -> &'static str {
        "ingest"
    }

    fn expected_scaffold_paths(&self, _root: &Path, _goal: &str) -> Vec<String> {
        manifest::required_artifacts()
    }

    fn verify_final(&self, root: &Path, _goal: &str) -> VerificationReport {
        let mut report = VerificationReport::pass();
        for path in manifest::required_artifacts() {
            if !root.join(&path).is_file() {
                report.push_profile_failure(format!("ingest required artifact missing: {path}"));
            }
        }
        let snapshot_count = root
            .join("data/snapshots")
            .read_dir()
            .ok()
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
                    .count()
            })
            .unwrap_or_default();
        if snapshot_count == 0 {
            report.push_profile_failure("ingest snapshot input missing: data/snapshots");
        }
        report
    }

    fn guidance(&self, _goal: &str) -> Option<String> {
        Some(manifest::guidance())
    }

    fn preset_ultra_plan(&self, goal: &str, style: &str, intent: &str) -> Option<UltraPlan> {
        manifest::preset_ultra_plan(goal, style, intent)
    }

    fn runtime_contract(&self, _intent: &str, _goal: &str) -> String {
        "- Read only fixed local files under data/snapshots; network access is forbidden.\n\
- Declare and freeze one deterministic candidate selector in output/inspection.json.\n\
- Bind each output field to one candidate block and record every declared normalization.\n\
- Write output/records.json and output/report.md deterministically."
            .to_string()
    }

    fn generation_rules(&self, _intent: &str) -> Option<&'static str> {
        Some(guidance::GENERATION_RULES)
    }

    fn quality_expectations(&self, _root: &Path, _goal: &str) -> ProfileQualityExpectations {
        ProfileQualityExpectations {
            required_artifacts: manifest::required_artifacts(),
            preferred_verify: Vec::new(),
            forbidden_verify: vec![
                "pip install".to_string(),
                "curl".to_string(),
                "wget".to_string(),
            ],
            dependency_order_hint: Some(
                "Inspect snapshots and declare candidates before implementing pipeline/main.py"
                    .to_string(),
            ),
        }
    }

    fn source_paths(&self, _root: &Path) -> Vec<String> {
        vec![
            "pipeline/main.py".to_string(),
            "output/inspection.json".to_string(),
        ]
    }

    fn evidence_repair_target_paths(&self, _root: &Path, _keys: &[String]) -> Vec<String> {
        self.source_paths(_root)
    }

    fn infer_required_capabilities(&self, _goal: &str) -> Vec<String> {
        manifest::required_capability_ids()
    }

    fn infer_required_obligations(
        &self,
        _goal: &str,
        _required_capabilities: &[String],
    ) -> Vec<String> {
        vec!["implementation".to_string()]
    }

    fn behavior_probe(
        &self,
        root: &Path,
        _goal: &str,
        _required_capabilities: &[String],
        _offline: bool,
    ) -> anyhow::Result<ProfileBehaviorProbeReport> {
        let summary = runtime::run_manifest_checks(root)?;
        Ok(ProfileBehaviorProbeReport {
            status: summary.assurance.behavior_status(),
            reasons: summary.reasons,
            evidence_path: Some(runtime::ASSURANCE_EVIDENCE_PATH.to_string()),
        })
    }
}
