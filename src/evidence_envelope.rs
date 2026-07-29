use std::collections::BTreeSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const ENVELOPE_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum EvidenceFamily {
    E,
    F,
    I,
    C,
    N,
    #[serde(rename = "circle")]
    Circle,
    #[serde(rename = "workflow")]
    Workflow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EvidenceClaim {
    pub index: usize,
    pub label: String,
    pub judgement: String,
    pub observation: Value,
    pub source_ref: Option<String>,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EvidenceNearestMiss {
    pub claim_index: usize,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EvidenceEnvelope {
    pub envelope_version: u8,
    pub family: EvidenceFamily,
    pub kind: String,
    pub epoch: u64,
    pub claims: Vec<EvidenceClaim>,
    pub nearest_miss: Vec<EvidenceNearestMiss>,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct EvidenceEnvelopeSpec {
    family: EvidenceFamily,
    kind: String,
    source_refs: Vec<String>,
}

impl EvidenceEnvelopeSpec {
    pub(crate) fn new(family: EvidenceFamily, kind: impl Into<String>) -> Self {
        Self {
            family,
            kind: kind.into(),
            source_refs: Vec::new(),
        }
    }

    pub(crate) fn with_source_refs(
        mut self,
        source_refs: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.source_refs = source_refs.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Serialize)]
struct EnvelopedEvidence<'a, T: Serialize> {
    #[serde(flatten)]
    legacy: &'a T,
    evidence_envelope: EvidenceEnvelope,
}

pub(crate) fn unix_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub(crate) fn event_envelope(
    family: EvidenceFamily,
    kind: impl Into<String>,
    epoch: u64,
    source_refs: impl IntoIterator<Item = impl Into<String>>,
) -> EvidenceEnvelope {
    EvidenceEnvelope {
        envelope_version: ENVELOPE_VERSION,
        family,
        kind: kind.into(),
        epoch,
        claims: Vec::new(),
        nearest_miss: Vec::new(),
        source_refs: source_refs
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}

pub(crate) fn to_vec_pretty<T: Serialize>(
    value: &T,
    spec: EvidenceEnvelopeSpec,
) -> anyhow::Result<Vec<u8>> {
    let legacy = serde_json::to_value(value).context("serialize legacy evidence")?;
    let envelope = build_envelope(&legacy, spec)?;
    serde_json::to_vec_pretty(&EnvelopedEvidence {
        legacy: value,
        evidence_envelope: envelope,
    })
    .context("serialize enveloped evidence")
}

pub(crate) fn write_json<T: Serialize>(
    path: &Path,
    value: &T,
    spec: EvidenceEnvelopeSpec,
    trailing_newline: bool,
) -> anyhow::Result<()> {
    let mut bytes = to_vec_pretty(value, spec)?;
    if trailing_newline {
        bytes.push(b'\n');
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

pub(crate) fn write_json_for_path<T: Serialize>(
    path: &Path,
    value: &T,
    family: EvidenceFamily,
    relative: &str,
    trailing_newline: bool,
) -> anyhow::Result<()> {
    let spec = EvidenceEnvelopeSpec::new(family, kind_for_path(family, relative));
    let spec = if family == EvidenceFamily::Circle {
        spec.with_source_refs(["evidence/workflow-events.jsonl"])
    } else {
        spec
    };
    write_json(path, value, spec, trailing_newline)
}

fn kind_for_path(family: EvidenceFamily, relative: &str) -> &'static str {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    match (family, name) {
        (EvidenceFamily::E, "pipeline-run.json") => "pipeline_probe",
        (EvidenceFamily::E, "inspection-schema.json") => "inspection_schema",
        (EvidenceFamily::E, "results-schema.json") => "results_schema",
        (EvidenceFamily::E, "reconciliation.json") => "reconciliation",
        (EvidenceFamily::E, "claims-binding.json") => "claims_binding",
        (EvidenceFamily::E, "rerun-consistency.json") => "rerun_consistency",
        (EvidenceFamily::E, "data-assurance.json") => "assurance",
        (EvidenceFamily::C, "cli-case-binding.json") => "case_binding",
        (EvidenceFamily::C, "cli-probe.json") => "argv_probe",
        (EvidenceFamily::C, "help-binding.json") => "help_binding",
        (EvidenceFamily::C, "cli-assurance.json") => "assurance",
        (EvidenceFamily::N, "ingest-candidate-freeze.json") => "candidate_freeze",
        (EvidenceFamily::N, "candidate-accounting.json") => "candidate_accounting",
        (EvidenceFamily::N, "source-binding.json") => "source_binding",
        (EvidenceFamily::N, "ingest-probe.json") => "ingest_probe",
        (EvidenceFamily::N, "format-schema.json") => "format_schema",
        (EvidenceFamily::N, "rerun-consistency.json") => "rerun_consistency",
        (EvidenceFamily::N, "ingest-assurance.json") => "assurance",
        (EvidenceFamily::Circle, "workflow-circle.json") => "workflow_circle",
        _ if family == EvidenceFamily::F && name.ends_with("-adjudication.json") => "adjudication",
        _ if family == EvidenceFamily::F && name.contains("-before-attempt-") => "before_attempt",
        _ if family == EvidenceFamily::F && name.ends_with("-before.json") => "before",
        _ if family == EvidenceFamily::F && name.ends_with("-after.json") => "after",
        _ if family == EvidenceFamily::F && name.contains("-regression-") => "regression",
        (EvidenceFamily::I, "investigation-binding.json") => "investigation_binding",
        (EvidenceFamily::I, _) if name.starts_with("investigation-run") => "investigation_run",
        (EvidenceFamily::E, _) => "data_check",
        (EvidenceFamily::F, _) => "fix_evidence",
        (EvidenceFamily::I, _) => "investigation_evidence",
        (EvidenceFamily::C, _) => "cli_check",
        (EvidenceFamily::N, _) => "ingest_check",
        (EvidenceFamily::Circle, _) => "circle_evidence",
        (EvidenceFamily::Workflow, _) => "workflow_evidence",
    }
}

fn build_envelope(legacy: &Value, spec: EvidenceEnvelopeSpec) -> anyhow::Result<EvidenceEnvelope> {
    let object = legacy
        .as_object()
        .context("evidence root must be a JSON object")?;
    if object.contains_key("evidence_envelope") {
        bail!("legacy evidence already contains evidence_envelope");
    }
    let claim_values = claim_values(object, &spec.kind);
    let mut claims = Vec::with_capacity(claim_values.len());
    let mut nearest_miss = Vec::new();
    let mut source_refs = spec.source_refs.into_iter().collect::<BTreeSet<_>>();
    for (index, claim) in claim_values.into_iter().enumerate() {
        let source_ref = string_at(
            claim,
            &["source_ref", "source", "source_path", "report_path"],
        );
        if let Some(source_ref) = source_ref.as_ref() {
            source_refs.insert(source_ref.clone());
        }
        let matched = bool_at(claim, &["ok", "matched"]);
        claims.push(EvidenceClaim {
            index,
            label: string_at(
                claim,
                &[
                    "claim",
                    "raw",
                    "quote",
                    "option",
                    "field",
                    "value",
                    "candidate_id",
                ],
            )
            .unwrap_or_default(),
            judgement: match matched {
                Some(true) => "matched",
                Some(false) => "violation",
                None => "observed",
            }
            .to_string(),
            observation: value_at(
                claim,
                &[
                    "observation",
                    "matched_result_value",
                    "output_value",
                    "normalized_source",
                    "value",
                ],
            )
            .unwrap_or(Value::Null),
            source_ref,
            direction: string_at(claim, &["direction"]),
        });
        if let Some(value) = value_at(claim, &["nearest_miss", "nearest"])
            && !value.is_null()
        {
            nearest_miss.push(EvidenceNearestMiss {
                claim_index: index,
                value,
            });
        }
    }
    for key in [
        "entry",
        "inspection_path",
        "results_path",
        "records_path",
        "candidate_freeze_path",
        "contract_ref",
    ] {
        if let Some(value) = object.get(key).and_then(Value::as_str) {
            source_refs.insert(value.to_string());
        }
    }
    for key in ["report_paths", "compared_paths"] {
        if let Some(values) = object.get(key).and_then(Value::as_array) {
            source_refs.extend(values.iter().filter_map(Value::as_str).map(str::to_string));
        }
    }
    if let Some(entry) = object
        .get("command")
        .and_then(Value::as_array)
        .and_then(|command| command.get(2))
        .and_then(Value::as_str)
    {
        source_refs.insert(entry.to_string());
    }
    if let Some(invalid) = source_refs
        .iter()
        .find(|reference| Path::new(reference.as_str()).is_absolute())
    {
        bail!("evidence source_ref must be workspace-relative: {invalid}");
    }
    Ok(EvidenceEnvelope {
        envelope_version: ENVELOPE_VERSION,
        family: spec.family,
        kind: spec.kind,
        epoch: unix_epoch(),
        claims,
        nearest_miss,
        source_refs: source_refs.into_iter().collect(),
    })
}

fn claim_values<'a>(object: &'a serde_json::Map<String, Value>, kind: &str) -> Vec<&'a Value> {
    let key = if object.get("claims").is_some_and(Value::is_array) {
        Some("claims")
    } else if object.get("output_claims").is_some_and(Value::is_array) {
        Some("output_claims")
    } else if matches!(kind, "help_binding" | "source_binding")
        && object.get("bindings").is_some_and(Value::is_array)
    {
        Some("bindings")
    } else {
        None
    };
    key.and_then(|key| object.get(key))
        .and_then(Value::as_array)
        .map(|values| values.iter().collect())
        .unwrap_or_default()
}

fn string_at(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .map(str::to_string)
}

fn bool_at(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| value.get(key).and_then(Value::as_bool))
}

fn value_at(value: &Value, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| value.get(key).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn envelope_is_additive_and_normalizes_claims() {
        let legacy = json!({
            "capability_id": "data_claims_binding",
            "status": "pass",
            "claims": [{
                "raw": "7",
                "ok": true,
                "matched_result_value": 7,
                "nearest_miss": null,
                "report_path": "output/report.md"
            }]
        });

        let serialized = to_vec_pretty(
            &legacy,
            EvidenceEnvelopeSpec::new(EvidenceFamily::E, "claims_binding"),
        )
        .unwrap();
        let output: Value = serde_json::from_slice(&serialized).unwrap();

        assert_eq!(output["capability_id"], legacy["capability_id"]);
        assert_eq!(output["status"], legacy["status"]);
        assert_eq!(output["claims"], legacy["claims"]);
        assert_eq!(output["evidence_envelope"]["envelope_version"], 1);
        assert_eq!(output["evidence_envelope"]["family"], "E");
        assert_eq!(output["evidence_envelope"]["kind"], "claims_binding");
        assert!(output["evidence_envelope"]["epoch"].is_u64());
        assert_eq!(
            output["evidence_envelope"]["claims"][0]["judgement"],
            "matched"
        );
        assert_eq!(
            output["evidence_envelope"]["source_refs"],
            json!(["output/report.md"])
        );
    }

    #[test]
    fn every_registered_family_has_a_stable_wire_value() {
        let families = [
            EvidenceFamily::E,
            EvidenceFamily::F,
            EvidenceFamily::I,
            EvidenceFamily::C,
            EvidenceFamily::N,
            EvidenceFamily::Circle,
            EvidenceFamily::Workflow,
        ];
        assert_eq!(
            families.map(|family| serde_json::to_value(family).unwrap()),
            [
                json!("E"),
                json!("F"),
                json!("I"),
                json!("C"),
                json!("N"),
                json!("circle"),
                json!("workflow"),
            ]
        );
    }

    #[test]
    fn rust_wire_families_match_the_transverse_registry() {
        #[derive(Deserialize)]
        struct Registry {
            families: Vec<String>,
        }

        let registry: Registry = toml::from_str(include_str!(
            "../workspace/management/evidence-families.toml"
        ))
        .unwrap();
        let rust = [
            EvidenceFamily::E,
            EvidenceFamily::F,
            EvidenceFamily::I,
            EvidenceFamily::C,
            EvidenceFamily::N,
            EvidenceFamily::Circle,
            EvidenceFamily::Workflow,
        ]
        .into_iter()
        .map(|family| {
            serde_json::to_value(family)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();

        assert_eq!(rust, registry.families);
    }

    #[test]
    fn absolute_source_refs_are_rejected() {
        let error = to_vec_pretty(
            &json!({"ok": true}),
            EvidenceEnvelopeSpec::new(EvidenceFamily::E, "test")
                .with_source_refs(["/private/input"]),
        )
        .unwrap_err();
        assert!(error.to_string().contains("workspace-relative"));
    }
}
