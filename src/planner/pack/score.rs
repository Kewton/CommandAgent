//! Fixed F-1 score declarations and additive checkpoint projection.
//!
//! Score consumes existing typed check states. It cannot define a judge,
//! change an earned verdict, or introduce an adoption threshold.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use serde_yaml::Value as YamlValue;

use super::schema::CheckBinding;
use super::{CheckId, LoadedPack};
use crate::evidence_envelope::{EvidenceEnvelopeSpec, EvidenceFamily, event_envelope, unix_epoch};

pub const SCORE_SCHEMA_VERSION: &str = "commandagent.eval.score/v0";
const SCORE_EVIDENCE_PATH: &str = "evidence/score-checkpoint.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoreUsage {
    Report,
    Allocation,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreDeclaration {
    schema_version: String,
    pub usage: Vec<ScoreUsage>,
    pub weights: Vec<ScoreWeight>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreWeight {
    pub atom: ScoreAtom,
    pub points: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreAtom {
    pub id: CheckId,
    pub params: BTreeMap<String, YamlValue>,
}

impl ScoreDeclaration {
    pub(super) fn validate(&self, checks: &[CheckBinding]) -> Result<(), String> {
        if self.schema_version != SCORE_SCHEMA_VERSION {
            return Err(format!(
                "score.schema_version must be `{SCORE_SCHEMA_VERSION}`"
            ));
        }
        if self.usage.is_empty() || self.usage.len() > 2 {
            return Err("score.usage must contain 1..2 entries".to_string());
        }
        let usage = self.usage.iter().copied().collect::<BTreeSet<_>>();
        if usage.len() != self.usage.len() {
            return Err("score.usage entries must be unique".to_string());
        }
        if self.weights.is_empty() || self.weights.len() > 256 {
            return Err("score.weights must contain 1..256 entries".to_string());
        }
        let bound = checks
            .iter()
            .map(|check| check.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut keys = BTreeSet::new();
        for weight in &self.weights {
            if !(1..=1_000).contains(&weight.points) {
                return Err("score weight points must be between 1 and 1000".to_string());
            }
            if !bound.contains(weight.atom.id.as_str()) {
                return Err(format!(
                    "score atom `{}` is not bound by eval.checks",
                    weight.atom.id.as_str()
                ));
            }
            validate_atom_params(&weight.atom)?;
            let key = weight.atom.canonical_key()?;
            if !keys.insert(key.clone()) {
                return Err(format!("duplicate score atom key `{key}`"));
            }
        }
        integrity_floor_guard()
    }

    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl ScoreAtom {
    pub fn canonical_key(&self) -> Result<String, String> {
        if self.params.is_empty() {
            return Ok(self.id.as_str().to_string());
        }
        let mut values = Vec::with_capacity(self.params.len());
        for (name, value) in &self.params {
            let value = value
                .as_str()
                .ok_or_else(|| format!("score atom parameter `{name}` must be a string"))?;
            values.push(format!("{name}={value}"));
        }
        Ok(format!("{}({})", self.id.as_str(), values.join(",")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AtomState {
    Pass,
    Absent,
    Violation,
    Unobserved,
}

impl AtomState {
    fn coefficient_twice(self) -> i64 {
        match self {
            Self::Pass => 2,
            Self::Absent | Self::Unobserved => 0,
            Self::Violation => -1,
        }
    }

    fn observed(self) -> bool {
        self != Self::Unobserved
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoreVector {
    pub reached: bool,
    pub score: Option<f64>,
    pub weighted_state_sum_twice: i64,
    pub weight_sum: u64,
    pub observed_weight: u64,
    pub atoms: Vec<ScoreAtomVector>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScoreAtomVector {
    pub key: String,
    pub state: AtomState,
    pub points: u16,
    pub source_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    state: AtomState,
    source_ref: String,
}

#[cfg(test)]
fn calculate(
    declaration: &ScoreDeclaration,
    states: &BTreeMap<String, AtomState>,
) -> Result<ScoreVector, String> {
    calculate_with_sources(
        declaration,
        &states
            .iter()
            .map(|(key, state)| {
                (
                    key.clone(),
                    Observation {
                        state: *state,
                        source_ref: "typed_observation".to_string(),
                    },
                )
            })
            .collect(),
    )
}

fn calculate_with_sources(
    declaration: &ScoreDeclaration,
    observations: &BTreeMap<String, Observation>,
) -> Result<ScoreVector, String> {
    let mut numerator_twice = 0_i64;
    let mut weight_sum = 0_u64;
    let mut observed_weight = 0_u64;
    let mut atoms = Vec::with_capacity(declaration.weights.len());
    for weight in &declaration.weights {
        let key = weight.atom.canonical_key()?;
        let observation = observations.get(&key).cloned().unwrap_or(Observation {
            state: AtomState::Unobserved,
            source_ref: "not_observed_at_checkpoint".to_string(),
        });
        let points = u64::from(weight.points);
        numerator_twice += observation.state.coefficient_twice() * i64::from(weight.points);
        weight_sum += points;
        if observation.state.observed() {
            observed_weight += points;
        }
        atoms.push(ScoreAtomVector {
            key,
            state: observation.state,
            points: weight.points,
            source_ref: observation.source_ref,
        });
    }
    let reached = observed_weight > 0;
    let score = reached.then(|| {
        let tenths = round_half_even_ratio(500 * numerator_twice, weight_sum as i64);
        tenths as f64 / 10.0
    });
    Ok(ScoreVector {
        reached,
        score,
        weighted_state_sum_twice: numerator_twice,
        weight_sum,
        observed_weight,
        atoms,
    })
}

fn round_half_even_ratio(numerator: i64, denominator: i64) -> i64 {
    let sign = numerator.signum();
    let absolute = numerator.abs();
    let quotient = absolute / denominator;
    let remainder = absolute % denominator;
    let rounded = match (remainder * 2).cmp(&denominator) {
        std::cmp::Ordering::Less => quotient,
        std::cmp::Ordering::Greater => quotient + 1,
        std::cmp::Ordering::Equal if quotient % 2 == 0 => quotient,
        std::cmp::Ordering::Equal => quotient + 1,
    };
    sign * rounded
}

fn integrity_floor_guard() -> Result<(), String> {
    let pass = AtomState::Pass.coefficient_twice();
    let absent = AtomState::Absent.coefficient_twice();
    let violation = AtomState::Violation.coefficient_twice();
    if pass > absent && absent > violation {
        Ok(())
    } else {
        Err("compiled score integrity floor is inverted".to_string())
    }
}

fn validate_atom_params(atom: &ScoreAtom) -> Result<(), String> {
    if atom.params.is_empty() {
        return Ok(());
    }
    let (parameter, allowed) = parameter_registry(atom.id.as_str()).ok_or_else(|| {
        format!(
            "score atom `{}` is not registered as parameterized",
            atom.id.as_str()
        )
    })?;
    if atom.params.len() != 1 || !atom.params.contains_key(parameter) {
        return Err(format!(
            "score atom `{}` accepts only parameter `{parameter}`",
            atom.id.as_str()
        ));
    }
    let value = atom.params[parameter]
        .as_str()
        .ok_or_else(|| format!("score atom parameter `{parameter}` must be a string"))?;
    if !allowed.contains(&value) {
        return Err(format!(
            "unregistered {parameter} `{value}` for score atom `{}`",
            atom.id.as_str()
        ));
    }
    Ok(())
}

fn parameter_registry(id: &str) -> Option<(&'static str, &'static [&'static str])> {
    const EXECUTION_BINDINGS: &[&str] = &[
        "pipeline.main",
        "cli.readme.normal_case",
        "cli.readme.help_surface",
        "fix.reproducer",
        "investigation.reproducer",
    ];
    const SCHEMAS: &[&str] = &["data.results.v1", "ingest.records.v1"];
    const ANCHORS: &[&str] = &[
        "cli.readme.observed_stdout",
        "data.report.executed_results",
        "ingest.output.frozen_source",
        "investigation.diagnosis.observed_failure",
    ];
    match id {
        "pipeline_probe" | "cli_probe" | "help_binding" | "before_fails" | "after_passes"
        | "reproducer_fails" => Some(("binding", EXECUTION_BINDINGS)),
        "data_results_schema" | "ingest_format_schema" => Some(("schema", SCHEMAS)),
        "data_claims_binding"
        | "cli_output_claims"
        | "ingest_source_binding"
        | "diagnosis_bound" => Some(("anchor", ANCHORS)),
        _ => None,
    }
}

pub(super) fn emit_checkpoint(
    pack: &LoadedPack,
    root: &Path,
    events_path: Option<&Path>,
) -> anyhow::Result<bool> {
    let Some(declaration) = pack.eval.as_ref().and_then(|eval| eval.score.as_ref()) else {
        return Ok(false);
    };
    let observations = declaration
        .weights
        .iter()
        .map(|weight| {
            let key = weight.atom.canonical_key().map_err(anyhow::Error::msg)?;
            Ok((
                key,
                observe_registered_atom(root, events_path, weight.atom.id.as_str()),
            ))
        })
        .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
    let vector = calculate_with_sources(declaration, &observations).map_err(anyhow::Error::msg)?;
    let source_refs = vector
        .atoms
        .iter()
        .filter(|atom| atom.state != AtomState::Unobserved)
        .map(|atom| atom.source_ref.clone())
        .collect::<BTreeSet<_>>();
    let epoch = unix_epoch();
    let evidence = ScoreCheckpointEvidence {
        schema_version: "commandagent.score-checkpoint/v0",
        score_schema_version: declaration.schema_version(),
        pack_id: pack.id(),
        pack_version: &pack.identity.version,
        pack_hash: &pack.hash,
        usage: &declaration.usage,
        checkpoint_epoch: epoch,
        vector: &vector,
    };
    let evidence_path =
        crate::tools::path_guard::resolve_optional_existing(root, SCORE_EVIDENCE_PATH)?;
    std::fs::create_dir_all(
        evidence_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("score evidence parent missing"))?,
    )?;
    crate::evidence_envelope::write_json(
        &evidence_path,
        &evidence,
        EvidenceEnvelopeSpec::new(EvidenceFamily::Score, "score_checkpoint")
            .with_source_refs(source_refs.iter().cloned()),
        true,
    )?;
    crate::eval_events::emit(
        events_path,
        json!({
            "event": "score_checkpoint",
            "score_schema_version": declaration.schema_version(),
            "pack_id": pack.id(),
            "pack_version": pack.identity.version,
            "pack_hash": pack.hash,
            "usage": declaration.usage,
            "checkpoint_epoch": epoch,
            "vector": vector,
            "evidence_path": SCORE_EVIDENCE_PATH,
            "evidence_envelope": event_envelope(
                EvidenceFamily::Score,
                "score_checkpoint",
                epoch,
                source_refs,
            ),
        }),
    );
    Ok(true)
}

#[derive(Serialize)]
struct ScoreCheckpointEvidence<'a> {
    schema_version: &'static str,
    score_schema_version: &'a str,
    pack_id: &'a str,
    pack_version: &'a str,
    pack_hash: &'a str,
    usage: &'a [ScoreUsage],
    checkpoint_epoch: u64,
    vector: &'a ScoreVector,
}

fn observe_registered_atom(root: &Path, events_path: Option<&Path>, id: &str) -> Observation {
    if let Some((relative, check)) = evidence_binding(id) {
        return observe_evidence(root, relative, check);
    }
    if matches!(
        id,
        "before_fails" | "after_passes" | "no_regression" | "reproducer_fails" | "diagnosis_bound"
    ) {
        return observe_event_requirement(root, events_path, id);
    }
    Observation {
        state: AtomState::Unobserved,
        source_ref: "registered_producer_not_observed".to_string(),
    }
}

fn evidence_binding(id: &str) -> Option<(&'static str, Option<&str>)> {
    match id {
        "pipeline_probe" => Some(("evidence/pipeline-run.json", None)),
        "data_inspection_schema" => Some(("evidence/inspection-schema.json", None)),
        "data_results_schema" => Some(("evidence/results-schema.json", None)),
        "data_reconciliation" => Some(("evidence/reconciliation.json", None)),
        "data_claims_binding" => Some(("evidence/claims-binding.json", None)),
        "data_rerun_consistency" => Some(("evidence/rerun-consistency.json", None)),
        "cli_probe" | "help_binding" | "cli_output_claims" | "cli_rerun_consistency" => {
            Some(("evidence/cli-assurance.json", Some(id)))
        }
        "ingest_source_binding"
        | "ingest_candidate_accounting"
        | "ingest_format_schema"
        | "ingest_rerun_consistency" => Some(("evidence/ingest-assurance.json", Some(id))),
        _ => None,
    }
}

fn observe_evidence(root: &Path, relative: &str, check: Option<&str>) -> Observation {
    let path = root.join(relative);
    let state = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .map(|document| {
            if let Some(check) = check {
                document
                    .pointer("/evidence/checks")
                    .and_then(Value::as_object)
                    .and_then(|checks| checks.get(check))
                    .and_then(Value::as_str)
                    .map(state_from_status)
                    .unwrap_or(AtomState::Unobserved)
            } else if document.get("ok").and_then(Value::as_bool) == Some(true) {
                AtomState::Pass
            } else if document.get("ok").and_then(Value::as_bool) == Some(false) {
                AtomState::Violation
            } else {
                document
                    .get("status")
                    .and_then(Value::as_str)
                    .map(state_from_status)
                    .unwrap_or(AtomState::Unobserved)
            }
        })
        .unwrap_or(AtomState::Unobserved);
    Observation {
        state,
        source_ref: relative.to_string(),
    }
}

fn observe_event_requirement(
    root: &Path,
    events_path: Option<&Path>,
    requirement: &str,
) -> Observation {
    let Some(path) = events_path else {
        return Observation {
            state: AtomState::Unobserved,
            source_ref: "events_unavailable".to_string(),
        };
    };
    let source = workspace_source_ref(root, path);
    let Ok(text) = std::fs::read_to_string(path) else {
        return Observation {
            state: AtomState::Unobserved,
            source_ref: source,
        };
    };
    let mut observed = AtomState::Unobserved;
    let mut observed_line = 0;
    for (index, line) in text.lines().enumerate() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(status) = event
            .get("requirement_statuses")
            .and_then(Value::as_object)
            .and_then(|statuses| statuses.get(requirement))
            .and_then(Value::as_str)
        else {
            continue;
        };
        observed = state_from_status(status);
        observed_line = index + 1;
    }
    Observation {
        state: observed,
        source_ref: if observed_line == 0 {
            source
        } else {
            format!("{source}:{observed_line}")
        },
    }
}

fn workspace_source_ref(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .and_then(Path::to_str)
        .unwrap_or("events.jsonl")
        .replace('\\', "/")
}

fn state_from_status(status: &str) -> AtomState {
    match status.trim().to_ascii_lowercase().as_str() {
        "pass" | "passed" | "success" | "full" | "complete" => AtomState::Pass,
        "absent" | "claims_absent" | "inconclusive" | "unavailable" => AtomState::Absent,
        "fail" | "failed" | "failure" | "violation" | "mismatch" => AtomState::Violation,
        _ => AtomState::Unobserved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::pack::{conform, parse_bytes};

    const VALID_SCORE_EVAL: &str = include_str!(
        "../../../tests/corpus/apps/test0802_f1_score_runtime/fixtures/valid-eval.yaml"
    );
    const INVALID_ADOPTION: &str = include_str!(
        "../../../tests/corpus/apps/test0802_f1_score_runtime/fixtures/invalid-adoption.yaml"
    );
    const INVALID_FLOOR: &str = include_str!(
        "../../../tests/corpus/apps/test0802_f1_score_runtime/fixtures/invalid-floor.yaml"
    );
    const INVALID_FREE_JUDGE: &str = include_str!(
        "../../../tests/corpus/apps/test0802_f1_score_runtime/fixtures/invalid-free-judge.yaml"
    );
    const INVALID_EXISTENCE_SCORE: &str = include_str!(
        "../../../tests/corpus/apps/test0802_f1_score_runtime/fixtures/invalid-existence-score.yaml"
    );

    fn declaration() -> (LoadedPack, ScoreDeclaration) {
        let pack = parse_bytes(None, Some(VALID_SCORE_EVAL.as_bytes())).unwrap();
        conform(&pack).unwrap();
        let score = pack.eval.as_ref().unwrap().score.as_ref().unwrap().clone();
        (pack, score)
    }

    #[test]
    fn fixed_floor_is_strict_and_fail_is_half_weight() {
        let (_, score) = declaration();
        let keys = score
            .weights
            .iter()
            .map(|weight| weight.atom.canonical_key().unwrap())
            .collect::<Vec<_>>();
        let states = BTreeMap::from([
            (keys[0].clone(), AtomState::Pass),
            (keys[1].clone(), AtomState::Pass),
            (keys[2].clone(), AtomState::Violation),
            (keys[3].clone(), AtomState::Pass),
        ]);
        let vector = calculate(&score, &states).unwrap();
        assert_eq!(vector.weighted_state_sum_twice, 5);
        assert_eq!(vector.score, Some(62.5));

        let single = |state| {
            let mut score = score.clone();
            score.weights.truncate(1);
            calculate(&score, &BTreeMap::from([(keys[0].clone(), state)]))
                .unwrap()
                .score
                .unwrap()
        };
        assert!(single(AtomState::Pass) > single(AtomState::Absent));
        assert!(single(AtomState::Absent) > single(AtomState::Violation));
    }

    #[test]
    fn all_unobserved_is_not_a_zero_score_run() {
        let (_, score) = declaration();
        let vector = calculate(&score, &BTreeMap::new()).unwrap();
        assert!(!vector.reached);
        assert_eq!(vector.score, None);
        assert_eq!(vector.observed_weight, 0);
    }

    #[test]
    fn schema_rejects_adoption_and_floor_overrides() {
        let adoption = parse_bytes(None, Some(INVALID_ADOPTION.as_bytes()))
            .unwrap_err()
            .to_string();
        assert!(adoption.contains("adoption"), "{adoption}");

        let floor = parse_bytes(None, Some(INVALID_FLOOR.as_bytes()))
            .unwrap_err()
            .to_string();
        assert!(
            floor.contains("state_values") || floor.contains("formula"),
            "{floor}"
        );
    }

    #[test]
    fn schema_rejects_free_judges_and_existence_points() {
        let judge = parse_bytes(None, Some(INVALID_FREE_JUDGE.as_bytes()))
            .unwrap_err()
            .to_string();
        assert!(judge.contains("judge"), "{judge}");

        let existence = parse_bytes(None, Some(INVALID_EXISTENCE_SCORE.as_bytes()))
            .unwrap_err()
            .to_string();
        assert!(existence.contains("file_exists"), "{existence}");
    }

    #[test]
    fn runtime_event_is_additive_and_envelope_compliant() {
        let (pack, _) = declaration();
        let root = tempfile::tempdir().unwrap();
        let evidence = root.path().join("evidence");
        std::fs::create_dir_all(&evidence).unwrap();
        std::fs::write(
            evidence.join("cli-assurance.json"),
            r#"{"evidence":{"checks":{"cli_probe":"pass","help_binding":"pass","cli_output_claims":"pass","cli_rerun_consistency":"pass"}}}"#,
        )
        .unwrap();
        let events = root.path().join("events.jsonl");
        std::fs::write(&events, "{\"event\":\"existing\"}\n").unwrap();

        assert!(emit_checkpoint(&pack, root.path(), Some(&events)).unwrap());

        let lines = std::fs::read_to_string(&events).unwrap();
        assert!(lines.starts_with("{\"event\":\"existing\"}\n"));
        let event: Value = serde_json::from_str(lines.lines().last().unwrap()).unwrap();
        assert_eq!(event["event"], "score_checkpoint");
        assert_eq!(event["vector"]["score"], 100.0);
        assert_eq!(event["evidence_envelope"]["family"], "score");
        assert_eq!(event["evidence_envelope"]["kind"], "score_checkpoint");
        assert_eq!(event["usage"], json!(["report", "allocation"]));
        assert!(root.path().join(SCORE_EVIDENCE_PATH).is_file());
    }

    #[test]
    fn absent_score_declaration_preserves_event_bytes() {
        let eval = VALID_SCORE_EVAL
            .split("\nscore:\n")
            .next()
            .unwrap()
            .as_bytes();
        let pack = parse_bytes(None, Some(eval)).unwrap();
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join("events.jsonl");
        let original = b"{\"event\":\"existing\"}\n";
        std::fs::write(&events, original).unwrap();

        assert!(!emit_checkpoint(&pack, root.path(), Some(&events)).unwrap());
        assert_eq!(std::fs::read(&events).unwrap(), original);
        assert!(!root.path().join(SCORE_EVIDENCE_PATH).exists());
    }
}
