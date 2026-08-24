use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{accounting, manifest, source_binding};
use crate::evidence_envelope::EvidenceFamily;
use crate::minimal_loop::pipeline_probe::{self, PipelineProbeConfig, PipelineProbeReport};
use crate::minimal_loop::rerun_consistency;
use crate::planner::capability_catalog::{ProbeCapability, ResolvedCapability};
use crate::planner::failure_vocabulary::ViolationId;

pub const ASSURANCE_EVIDENCE_PATH: &str = "evidence/ingest-assurance.json";
pub const INGEST_PROBE_EVIDENCE_PATH: &str = "evidence/ingest-probe.json";
pub const FORMAT_SCHEMA_EVIDENCE_PATH: &str = "evidence/format-schema.json";
pub const RERUN_EVIDENCE_PATH: &str = "evidence/rerun-consistency.json";
pub const N1: &str = "pipeline_probe";
pub const N2: &str = "ingest_source_binding";
pub const N3: &str = "ingest_candidate_accounting";
pub const N4: &str = "ingest_format_schema";
pub const N5: &str = "ingest_rerun_consistency";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    ClaimsAbsent,
    Failed,
    NotExecuted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IngestAssurance {
    Full,
    Partial,
    Static,
    Failed,
}

impl IngestAssurance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Partial => "partial",
            Self::Static => "static",
            Self::Failed => "failed",
        }
    }

    pub const fn behavior_status(self) -> &'static str {
        match self {
            Self::Full => "pass",
            Self::Partial => "partial",
            Self::Static => "static",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestEvidenceState {
    pub checks: BTreeMap<String, CheckStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestCheckSummary {
    pub status: String,
    pub assurance: IngestAssurance,
    pub evidence: IngestEvidenceState,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatSchemaEvidence {
    pub capability_id: String,
    pub status: String,
    pub ok: bool,
    pub declared_fields: Vec<String>,
    pub record_count: usize,
    pub failure_kinds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestProbeEvidence {
    pub capability_id: String,
    pub status: String,
    pub ok: bool,
    pub candidate_freeze_path: String,
    pub snapshot_ids: Vec<String>,
    pub required_artifacts: BTreeMap<String, bool>,
    pub execution: Option<PipelineProbeReport>,
    pub failure_kinds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RerunEvidence {
    pub capability_id: String,
    pub status: String,
    pub ok: bool,
    pub compared_paths: Vec<String>,
    pub failure_kinds: Vec<String>,
}

#[derive(Debug)]
struct Adapters {
    entry: String,
    timeout_seconds: u16,
    rerun_entry: String,
    rerun_timeout_seconds: u16,
}

pub fn run_manifest_checks(root: &Path) -> anyhow::Result<IngestCheckSummary> {
    let adapters = adapters()?;
    let frozen = accounting::freeze(root)?;
    let mut checks = BTreeMap::new();
    let mut reasons = Vec::new();

    let first = run_ingest_probe(
        root,
        &adapters.entry,
        adapters.timeout_seconds,
        &frozen,
        &mut reasons,
    )?;
    checks.insert(N1.to_string(), status(first));

    match source_binding::check(root, &frozen) {
        Ok(evidence) => {
            reasons.extend(evidence.failure_kinds);
            checks.insert(
                N2.to_string(),
                match evidence.status.as_str() {
                    "pass" => CheckStatus::Pass,
                    "claims_absent" => CheckStatus::ClaimsAbsent,
                    _ => CheckStatus::Failed,
                },
            );
        }
        Err(error) => {
            reasons.push(format!("{N2}:error:{error}"));
            checks.insert(N2.to_string(), CheckStatus::Failed);
        }
    }

    match accounting::check(root, &frozen) {
        Ok(evidence) => {
            reasons.extend(evidence.failure_kinds);
            checks.insert(N3.to_string(), status(evidence.ok));
        }
        Err(error) => {
            reasons.push(format!("{N3}:error:{error}"));
            checks.insert(N3.to_string(), CheckStatus::Failed);
        }
    }

    match check_format_schema(root, &frozen) {
        Ok(evidence) => {
            reasons.extend(evidence.failure_kinds.clone());
            checks.insert(N4.to_string(), status(evidence.ok));
        }
        Err(error) => {
            reasons.push(format!("{N4}:error:{error}"));
            checks.insert(N4.to_string(), CheckStatus::Failed);
        }
    }

    let rerun = check_rerun(
        root,
        &adapters.rerun_entry,
        adapters.rerun_timeout_seconds,
        &frozen,
    )?;
    reasons.extend(rerun.failure_kinds.clone());
    checks.insert(N5.to_string(), status(rerun.ok));

    reasons.sort();
    reasons.dedup();
    let evidence = IngestEvidenceState { checks };
    let assurance = classify(&evidence);
    let summary = IngestCheckSummary {
        status: assurance.as_str().to_string(),
        assurance,
        evidence,
        reasons,
    };
    write_json(root, ASSURANCE_EVIDENCE_PATH, &summary)?;
    Ok(summary)
}

pub fn classify(evidence: &IngestEvidenceState) -> IngestAssurance {
    let get = |id| {
        evidence
            .checks
            .get(id)
            .copied()
            .unwrap_or(CheckStatus::NotExecuted)
    };
    if get(N1) == CheckStatus::NotExecuted {
        return IngestAssurance::Static;
    }
    if [N1, N2, N3, N4, N5]
        .into_iter()
        .any(|id| matches!(get(id), CheckStatus::Failed | CheckStatus::NotExecuted))
    {
        return IngestAssurance::Failed;
    }
    if [N1, N2, N3, N4, N5]
        .into_iter()
        .all(|id| get(id) == CheckStatus::Pass)
    {
        return IngestAssurance::Full;
    }
    if get(N1) == CheckStatus::Pass
        && get(N2) == CheckStatus::ClaimsAbsent
        && [N3, N4, N5]
            .into_iter()
            .all(|id| get(id) == CheckStatus::Pass)
    {
        return IngestAssurance::Partial;
    }
    IngestAssurance::Failed
}

fn adapters() -> anyhow::Result<Adapters> {
    let resolved = manifest::get().resolve()?;
    let mut pipeline = None;
    let mut rerun = None;
    for check in resolved.into_values().flatten() {
        match check.capability {
            ResolvedCapability::Probe(ProbeCapability::Pipeline {
                entry,
                timeout_seconds,
            }) if check.id == N1 => pipeline = Some((entry, timeout_seconds)),
            ResolvedCapability::Probe(ProbeCapability::DataRerunConsistency {
                entry,
                timeout_seconds,
            }) if check.id == N5 => rerun = Some((entry, timeout_seconds)),
            _ => {}
        }
    }
    let (entry, timeout_seconds) = pipeline.context("ingest N1 adapter missing")?;
    let (rerun_entry, rerun_timeout_seconds) = rerun.context("ingest N5 adapter missing")?;
    Ok(Adapters {
        entry,
        timeout_seconds,
        rerun_entry,
        rerun_timeout_seconds,
    })
}

fn run_pipeline(root: &Path, entry: &str, timeout_seconds: u16, reasons: &mut Vec<String>) -> bool {
    match pipeline_probe::run(
        root,
        PipelineProbeConfig::new(entry)
            .with_timeout(Duration::from_secs(timeout_seconds.into()))
            .with_evidence_family(EvidenceFamily::N),
    ) {
        Ok(report) => {
            reasons.extend(report.failure_kinds.clone());
            report.ok
        }
        Err(error) => {
            reasons.push(format!("{N1}:error:{error}"));
            false
        }
    }
}

fn run_ingest_probe(
    root: &Path,
    entry: &str,
    timeout_seconds: u16,
    frozen: &accounting::CandidateFreeze,
    reasons: &mut Vec<String>,
) -> anyhow::Result<bool> {
    let mut failure_kinds = Vec::new();
    let execution = match pipeline_probe::run(
        root,
        PipelineProbeConfig::new(entry)
            .with_timeout(Duration::from_secs(timeout_seconds.into()))
            .with_evidence_family(EvidenceFamily::N),
    ) {
        Ok(report) => {
            failure_kinds.extend(report.failure_kinds.clone());
            Some(report)
        }
        Err(error) => {
            failure_kinds.push(format!("{N1}:error:{error}"));
            None
        }
    };
    let required_artifacts = [
        "pipeline/main.py",
        "output/records.json",
        "output/report.md",
    ]
    .into_iter()
    .map(|path| (path.to_string(), root.join(path).is_file()))
    .collect::<BTreeMap<_, _>>();
    for (path, present) in &required_artifacts {
        if !present {
            failure_kinds.push(format!("ingest_probe:required_artifact_missing:{path}"));
        }
    }
    failure_kinds.sort();
    failure_kinds.dedup();
    reasons.extend(failure_kinds.clone());
    let evidence = IngestProbeEvidence {
        capability_id: "ingest_probe".to_string(),
        status: if failure_kinds.is_empty() {
            "pass"
        } else {
            "failed"
        }
        .to_string(),
        ok: failure_kinds.is_empty(),
        candidate_freeze_path: accounting::FREEZE_EVIDENCE_PATH.to_string(),
        snapshot_ids: frozen
            .snapshots
            .iter()
            .map(|snapshot| snapshot.fnv1a64.clone())
            .collect(),
        required_artifacts,
        execution,
        failure_kinds,
    };
    write_json(root, INGEST_PROBE_EVIDENCE_PATH, &evidence)?;
    Ok(evidence.ok)
}

fn check_format_schema(
    root: &Path,
    frozen: &accounting::CandidateFreeze,
) -> anyhow::Result<FormatSchemaEvidence> {
    let format = source_binding::record_format(frozen)?;
    let records = source_binding::records(root)?;
    let declared = format
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<BTreeSet<_>>();
    let mut failure_kinds = Vec::new();
    let current = accounting::load_inspection(root)?;
    if current.record_format != frozen.record_format {
        failure_kinds.push("format_schema_violation:declaration_changed_after_freeze".to_string());
    }
    for (record_index, record) in records.iter().enumerate() {
        let observed = record.keys().cloned().collect::<BTreeSet<_>>();
        if observed != declared {
            failure_kinds.push(
                ViolationId::format_schema(format!("record={record_index}:fields")).to_string(),
            );
        }
        for field in &format.fields {
            let valid = record.get(&field.name).is_some_and(|value| {
                matches!(
                    (field.field_type, value),
                    (source_binding::FieldType::String, Value::String(_))
                        | (source_binding::FieldType::Number, Value::Number(_))
                        | (source_binding::FieldType::Boolean, Value::Bool(_))
                )
            });
            if !valid {
                failure_kinds.push(
                    ViolationId::format_schema(format!(
                        "record={record_index}:field={}:type",
                        field.name
                    ))
                    .to_string(),
                );
            }
        }
    }
    failure_kinds.sort();
    failure_kinds.dedup();
    let evidence = FormatSchemaEvidence {
        capability_id: N4.to_string(),
        status: if failure_kinds.is_empty() {
            "pass"
        } else {
            "failed"
        }
        .to_string(),
        ok: failure_kinds.is_empty(),
        declared_fields: declared.into_iter().collect(),
        record_count: records.len(),
        failure_kinds,
    };
    write_json(root, FORMAT_SCHEMA_EVIDENCE_PATH, &evidence)?;
    Ok(evidence)
}

fn check_rerun(
    root: &Path,
    entry: &str,
    timeout_seconds: u16,
    frozen: &accounting::CandidateFreeze,
) -> anyhow::Result<RerunEvidence> {
    let paths = ["output/records.json", "output/report.md"];
    let baseline = capture(root, &paths);
    let mut failure_kinds = Vec::new();
    let pipeline_ok = run_pipeline(root, entry, timeout_seconds, &mut failure_kinds);
    let rerun = capture(root, &paths);
    if !pipeline_ok {
        failure_kinds.push("rerun_violation:pipeline_failed".to_string());
    }
    if !rerun_consistency::reproduced(&baseline, &rerun) {
        failure_kinds.push("rerun_violation:output_changed".to_string());
    }
    let declarations_intact = accounting::load_inspection(root).is_ok_and(|inspection| {
        inspection.candidate_selector == frozen.selector
            && inspection.record_format == frozen.record_format
    });
    if !declarations_intact || !accounting::candidate_lineage_matches(root, frozen) {
        failure_kinds.push("rerun_violation:frozen_input_or_declaration_changed".to_string());
    }
    if baseline.values().any(Option::is_none) || rerun.values().any(Option::is_none) {
        failure_kinds.push("rerun_violation:artifact_missing".to_string());
    }
    failure_kinds.sort();
    failure_kinds.dedup();
    let evidence = RerunEvidence {
        capability_id: N5.to_string(),
        status: if failure_kinds.is_empty() {
            "pass"
        } else {
            "failed"
        }
        .to_string(),
        ok: failure_kinds.is_empty(),
        compared_paths: paths.into_iter().map(str::to_string).collect(),
        failure_kinds,
    };
    write_json(root, RERUN_EVIDENCE_PATH, &evidence)?;
    Ok(evidence)
}

fn capture(root: &Path, paths: &[&str]) -> BTreeMap<String, Option<Vec<u8>>> {
    paths
        .iter()
        .map(|path| {
            let bytes = crate::tools::path_guard::resolve_existing(root, path)
                .ok()
                .and_then(|resolved| std::fs::read(resolved).ok());
            ((*path).to_string(), bytes)
        })
        .collect()
}

fn status(ok: bool) -> CheckStatus {
    if ok {
        CheckStatus::Pass
    } else {
        CheckStatus::Failed
    }
}

fn write_json<T: Serialize>(root: &Path, relative: &str, value: &T) -> anyhow::Result<()> {
    let path = crate::tools::path_guard::resolve_optional_existing(root, relative)
        .with_context(|| format!("ingest evidence path escapes workspace: {relative}"))?;
    std::fs::create_dir_all(path.parent().context("ingest evidence parent missing")?)?;
    crate::evidence_envelope::write_json_for_path(&path, value, EvidenceFamily::N, relative, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn assurance_mapping_matches_the_fixed_contract() {
        let all = |status| IngestEvidenceState {
            checks: [N1, N2, N3, N4, N5]
                .into_iter()
                .map(|id| (id.to_string(), status))
                .collect(),
        };
        assert_eq!(
            classify(&IngestEvidenceState {
                checks: BTreeMap::new()
            }),
            IngestAssurance::Static
        );
        assert_eq!(classify(&all(CheckStatus::Failed)), IngestAssurance::Failed);
        assert_eq!(classify(&all(CheckStatus::Pass)), IngestAssurance::Full);
        let mut absent = all(CheckStatus::Pass);
        absent
            .checks
            .insert(N2.to_string(), CheckStatus::ClaimsAbsent);
        assert_eq!(classify(&absent), IngestAssurance::Partial);
    }

    #[test]
    fn n4_rejects_fields_or_types_outside_the_declared_format() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/snapshots")).unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(dir.path().join("data/snapshots/events.txt"), "EVENT|one\n").unwrap();
        std::fs::write(
            dir.path().join(accounting::INSPECTION_PATH),
            serde_json::to_vec_pretty(&json!({
                "candidate_selector": {"kind":"line_prefix","value":"EVENT|"},
                "candidate_accounting": {"accepted":[],"excluded":[]},
                "record_format": {"fields":[
                    {"name":"date","type":"string","normalizations":["identity"]}
                ]}
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("output/records.json"),
            r#"[{"date":25,"invented":"value"}]"#,
        )
        .unwrap();

        let frozen = accounting::freeze(dir.path()).unwrap();
        let mut inspection: Value = serde_json::from_slice(
            &std::fs::read(dir.path().join(accounting::INSPECTION_PATH)).unwrap(),
        )
        .unwrap();
        inspection["record_format"]["fields"][0]["type"] = json!("number");
        std::fs::write(
            dir.path().join(accounting::INSPECTION_PATH),
            serde_json::to_vec_pretty(&inspection).unwrap(),
        )
        .unwrap();
        let evidence = check_format_schema(dir.path(), &frozen).unwrap();

        assert!(!evidence.ok);
        assert!(
            evidence
                .failure_kinds
                .contains(&"format_schema_violation:record=0:fields".to_string())
        );
        assert!(
            evidence
                .failure_kinds
                .contains(&"format_schema_violation:record=0:field=date:type".to_string())
        );
        assert!(
            evidence
                .failure_kinds
                .contains(&"format_schema_violation:declaration_changed_after_freeze".to_string())
        );
    }
}
