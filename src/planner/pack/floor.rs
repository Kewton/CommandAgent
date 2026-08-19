use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use thiserror::Error;
use toml::value::Table;

use crate::planner::adjudication::contract;
use crate::planner::capability_catalog::{self, ProbeCapability, ResolvedCapability};
use crate::planner::profile_manifest::ManifestV1;
use crate::planner::profiles::{data, ingest, python_cli};

use super::schema::{CheckAt, EvidenceStage, yaml_to_toml};
use super::{
    CheckBinding, ExtractionId, LoadedPack, NormalizerId, PackError, PackIdentity, PackIntent,
    PackProfile,
};

#[derive(Debug, Error)]
pub enum ConformanceError {
    #[error(transparent)]
    Load(#[from] PackError),
    #[error("contract floor definition is invalid: {0}")]
    FloorDefinition(String),
    #[error("pack contract floor violation: {0}")]
    FloorViolation(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FloorCheck {
    at: CheckAt,
    capability: Option<ResolvedCapability>,
    extraction: Vec<ExtractionId>,
    normalizers: Vec<NormalizerId>,
}

#[derive(Debug, Serialize)]
pub struct ConformanceReport {
    pub status: &'static str,
    pub pack_id: String,
    pub pack_version: String,
    pub profile: String,
    pub intent: String,
    pub exact_byte_hash: String,
    pub assist_present: bool,
    pub eval_present: bool,
    pub floor_check_count: usize,
    pub effective_check_count: usize,
    pub schema_count: usize,
}

pub fn conform_directory(path: &Path) -> Result<ConformanceReport, ConformanceError> {
    conform(&super::load_directory(path)?)
}

pub fn conform(pack: &LoadedPack) -> Result<ConformanceReport, ConformanceError> {
    let floor = contract_floor(&pack.identity)?;
    let mut effective = floor.clone();

    if let Some(eval) = &pack.eval {
        for check in &eval.checks {
            let id = check.id.as_str();
            let resolved = if let Some(floor_check) = floor.get(id) {
                validate_override(id, floor_check, check)?
            } else {
                validate_additive(&pack.identity, check)?
            };
            effective.insert(id.to_string(), resolved);
        }
    }

    // This separate assertion deliberately remains after merge: tests can feed
    // a damaged effective set and prove that omission never earns conformance.
    guard_effective_floor(&floor, &effective)?;
    validate_shared_cli_input(&pack.identity, &effective)?;
    validate_assist_gates(pack, &floor)?;
    validate_material_injections(pack)?;

    Ok(ConformanceReport {
        status: "conformant",
        pack_id: pack.identity.id.clone(),
        pack_version: pack.identity.version.clone(),
        profile: pack.identity.profile.as_str().to_string(),
        intent: pack.identity.intent.as_str().to_string(),
        exact_byte_hash: pack.hash.clone(),
        assist_present: pack.assist.is_some(),
        eval_present: pack.eval.is_some(),
        floor_check_count: floor.len(),
        effective_check_count: effective.len(),
        schema_count: pack
            .eval
            .as_ref()
            .map_or(0, |document| document.schemas.len()),
    })
}

fn validate_additive(
    identity: &PackIdentity,
    binding: &CheckBinding,
) -> Result<FloorCheck, ConformanceError> {
    if identity.profile != PackProfile::Nextjs || identity.intent != PackIntent::Create {
        return Err(ConformanceError::FloorViolation(format!(
            "check `{}` is not part of the registered {} × {} contract floor",
            binding.id.as_str(),
            identity.profile,
            identity.intent
        )));
    }
    if binding.at != CheckAt::FinalAcceptance
        || !binding.extraction.is_empty()
        || !binding.normalizers.is_empty()
    {
        return Err(ConformanceError::FloorViolation(format!(
            "additive check `{}` must remain a final_acceptance check without extractors or normalizers",
            binding.id.as_str()
        )));
    }
    let capability = resolve_binding(binding)?;
    if !matches!(
        capability,
        Some(ResolvedCapability::Internal(
            crate::planner::capability_catalog::InternalCapability::Pack(_)
        ))
    ) {
        return Err(ConformanceError::FloorViolation(format!(
            "check `{}` is not registered as an additive pack check",
            binding.id.as_str()
        )));
    }
    Ok(FloorCheck {
        at: binding.at.clone(),
        capability,
        extraction: Vec::new(),
        normalizers: Vec::new(),
    })
}

fn validate_material_injections(pack: &LoadedPack) -> Result<(), ConformanceError> {
    super::material_document::validate_all(pack)
        .map_err(|error| ConformanceError::FloorViolation(error.to_string()))?;
    let Some(assist) = &pack.assist else {
        return Ok(());
    };
    for injection in &assist.inject {
        if injection.source != super::AssistSource::PackMaterialDocument {
            continue;
        }
        let file = injection
            .params
            .get("file")
            .and_then(serde_yaml::Value::as_str)
            .expect("pack material file is schema-validated");
        if !pack.materials.contains_key(file) {
            return Err(ConformanceError::FloorViolation(format!(
                "pack material `materials/{file}` is missing"
            )));
        }
    }
    Ok(())
}

fn validate_assist_gates(
    pack: &LoadedPack,
    floor: &BTreeMap<String, FloorCheck>,
) -> Result<(), ConformanceError> {
    let Some(assist) = &pack.assist else {
        return Ok(());
    };
    for literal in &assist.literals {
        if !floor.contains_key(literal.gate.as_str()) {
            return Err(ConformanceError::FloorViolation(format!(
                "literal gate `{}` is not in the {} × {} contract floor",
                literal.gate.as_str(),
                pack.identity.profile,
                pack.identity.intent
            )));
        }
    }
    Ok(())
}

fn validate_override(
    id: &str,
    floor: &FloorCheck,
    binding: &CheckBinding,
) -> Result<FloorCheck, ConformanceError> {
    if binding.at != floor.at {
        return Err(ConformanceError::FloorViolation(format!(
            "check `{id}` moved from {:?} to {:?}",
            floor.at, binding.at
        )));
    }
    if binding.extraction != floor.extraction {
        return Err(ConformanceError::FloorViolation(format!(
            "check `{id}` changed required extraction from {:?} to {:?}",
            floor.extraction, binding.extraction
        )));
    }
    if id != "ingest_source_binding" && binding.normalizers != floor.normalizers {
        return Err(ConformanceError::FloorViolation(format!(
            "check `{id}` does not expose normalizer parameterization"
        )));
    }
    if id == "ingest_source_binding" {
        validate_ingest_normalizers(&binding.normalizers)?;
    }

    let resolved = resolve_binding(binding)?;
    if !capability_is_not_weaker(floor.capability.as_ref(), resolved.as_ref()) {
        return Err(ConformanceError::FloorViolation(format!(
            "check `{id}` changed its registered capability parameters"
        )));
    }
    Ok(FloorCheck {
        at: binding.at.clone(),
        capability: resolved,
        extraction: binding.extraction.clone(),
        normalizers: binding.normalizers.clone(),
    })
}

fn validate_ingest_normalizers(normalizers: &[NormalizerId]) -> Result<(), ConformanceError> {
    if normalizers.first() != Some(&NormalizerId::Identity) {
        return Err(ConformanceError::FloorViolation(
            "check `ingest_source_binding` must preserve `identity` as its first normalizer"
                .to_string(),
        ));
    }
    if normalizers.contains(&NormalizerId::DocumentYearContext)
        && !normalizers.contains(&NormalizerId::JapaneseDateToIso)
    {
        return Err(ConformanceError::FloorViolation(
            "`document_year_context` requires `japanese_date_to_iso`".to_string(),
        ));
    }
    Ok(())
}

fn capability_is_not_weaker(
    floor: Option<&ResolvedCapability>,
    configured: Option<&ResolvedCapability>,
) -> bool {
    match (floor, configured) {
        (None, None) => true,
        (
            Some(ResolvedCapability::Probe(ProbeCapability::Pipeline {
                entry: floor_entry,
                timeout_seconds: floor_timeout,
            })),
            Some(ResolvedCapability::Probe(ProbeCapability::Pipeline {
                entry,
                timeout_seconds,
            })),
        )
        | (
            Some(ResolvedCapability::Probe(ProbeCapability::DataRerunConsistency {
                entry: floor_entry,
                timeout_seconds: floor_timeout,
            })),
            Some(ResolvedCapability::Probe(ProbeCapability::DataRerunConsistency {
                entry,
                timeout_seconds,
            })),
        ) => entry == floor_entry && timeout_seconds <= floor_timeout,
        (
            Some(ResolvedCapability::Probe(ProbeCapability::Cli(floor_cli))),
            Some(ResolvedCapability::Probe(ProbeCapability::Cli(cli))),
        ) => {
            cli.check == floor_cli.check
                && cli.entry == floor_cli.entry
                && cli.timeout_seconds <= floor_cli.timeout_seconds
                && floor_cli
                    .usage_paths
                    .iter()
                    .all(|required| cli.usage_paths.contains(required))
        }
        (Some(floor), Some(configured)) => floor == configured,
        _ => false,
    }
}

fn validate_shared_cli_input(
    identity: &PackIdentity,
    effective: &BTreeMap<String, FloorCheck>,
) -> Result<(), ConformanceError> {
    if identity.profile != PackProfile::PythonCli || identity.intent != PackIntent::Create {
        return Ok(());
    }
    let inputs = [
        "cli_probe",
        "help_binding",
        "cli_output_claims",
        "cli_rerun_consistency",
    ]
    .into_iter()
    .map(|id| {
        effective
            .get(id)
            .and_then(|check| check.capability.as_ref())
            .and_then(|capability| match capability {
                ResolvedCapability::Probe(ProbeCapability::Cli(cli)) => Some((
                    cli.entry.as_str(),
                    cli.usage_paths.as_slice(),
                    cli.timeout_seconds,
                )),
                _ => None,
            })
            .ok_or_else(|| {
                ConformanceError::FloorDefinition(format!(
                    "CLI floor check `{id}` did not resolve to a CLI capability"
                ))
            })
    })
    .collect::<Result<Vec<_>, _>>()?;
    if inputs.windows(2).all(|pair| pair[0] == pair[1]) {
        Ok(())
    } else {
        Err(ConformanceError::FloorViolation(
            "all four CLI checks must retain one shared runtime input".to_string(),
        ))
    }
}

fn resolve_binding(binding: &CheckBinding) -> Result<Option<ResolvedCapability>, ConformanceError> {
    if !capability_catalog::registry()
        .iter()
        .any(|spec| spec.id == binding.id.as_str())
    {
        return Ok(None);
    }
    let mut params = Table::new();
    for (name, value) in &binding.params {
        params.insert(
            name.clone(),
            yaml_to_toml(value).map_err(ConformanceError::FloorDefinition)?,
        );
    }
    capability_catalog::resolve(binding.id.as_str(), &params)
        .map(Some)
        .map_err(|error| ConformanceError::FloorDefinition(error.to_string()))
}

fn guard_effective_floor(
    floor: &BTreeMap<String, FloorCheck>,
    effective: &BTreeMap<String, FloorCheck>,
) -> Result<(), ConformanceError> {
    let missing = floor
        .keys()
        .filter(|id| !effective.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(ConformanceError::FloorViolation(format!(
            "effective pack removed required checks: {}",
            missing.join(", ")
        )))
    }
}

fn contract_floor(
    identity: &PackIdentity,
) -> Result<BTreeMap<String, FloorCheck>, ConformanceError> {
    match identity.intent {
        PackIntent::Create => create_floor(identity.profile),
        PackIntent::Fix | PackIntent::Investigate => intent_floor(identity.intent),
    }
}

fn create_floor(profile: PackProfile) -> Result<BTreeMap<String, FloorCheck>, ConformanceError> {
    let manifest = match profile {
        PackProfile::Data => data::manifest::get(),
        PackProfile::PythonCli => python_cli::manifest::get(),
        PackProfile::Ingest => ingest::manifest::get(),
        // Next.js has no registered E/F/I/C/N pack check in v0. Its testimony
        // gate is deliberately a P-1 addition, not an invented P-0b ID.
        PackProfile::Nextjs => return Ok(BTreeMap::new()),
    };
    manifest_floor(manifest)
}

fn manifest_floor(manifest: &ManifestV1) -> Result<BTreeMap<String, FloorCheck>, ConformanceError> {
    let resolved = manifest
        .resolve()
        .map_err(|error| ConformanceError::FloorDefinition(error.to_string()))?;
    let mut floor = BTreeMap::new();
    for check in resolved.values().flatten() {
        let at = match check.phases.as_deref() {
            None => CheckAt::FinalAcceptance,
            Some([phase]) => CheckAt::Phase {
                id: super::InjectionPoint::parse(phase).ok_or_else(|| {
                    ConformanceError::FloorDefinition(format!(
                        "manifest phase `{phase}` is not a registered pack injection point"
                    ))
                })?,
            },
            Some(phases) => {
                return Err(ConformanceError::FloorDefinition(format!(
                    "check `{}` has unsupported multi-phase floor {phases:?}",
                    check.id
                )));
            }
        };
        let (extraction, normalizers) = extraction_floor(&check.id);
        if floor
            .insert(
                check.id.clone(),
                FloorCheck {
                    at,
                    capability: Some(check.capability.clone()),
                    extraction,
                    normalizers,
                },
            )
            .is_some()
        {
            return Err(ConformanceError::FloorDefinition(format!(
                "duplicate manifest floor check `{}`",
                check.id
            )));
        }
    }
    Ok(floor)
}

fn intent_floor(intent: PackIntent) -> Result<BTreeMap<String, FloorCheck>, ConformanceError> {
    let contract = contract::intent_contract(intent.as_str()).ok_or_else(|| {
        ConformanceError::FloorDefinition(format!("missing intent contract `{intent}`"))
    })?;
    let mut floor = BTreeMap::new();
    for requirement in contract.requirements {
        if requirement.id == "create_acceptance" {
            continue;
        }
        let at = match requirement.stage {
            contract::EvidenceStage::Before => CheckAt::Stage {
                id: EvidenceStage::Before,
            },
            contract::EvidenceStage::After => CheckAt::Stage {
                id: EvidenceStage::After,
            },
            contract::EvidenceStage::Diagnosis => CheckAt::Stage {
                id: EvidenceStage::Diagnosis,
            },
            contract::EvidenceStage::Unstaged => CheckAt::FinalAcceptance,
        };
        let (extraction, normalizers) = extraction_floor(requirement.id);
        floor.insert(
            requirement.id.to_string(),
            FloorCheck {
                at,
                capability: None,
                extraction,
                normalizers,
            },
        );
    }
    Ok(floor)
}

fn extraction_floor(id: &str) -> (Vec<ExtractionId>, Vec<NormalizerId>) {
    use ExtractionId as E;
    let extraction = match id {
        "data_claims_binding" => vec![E::NumericClaims, E::DateLabelSpans],
        "cli_probe" => vec![E::CliUsageCase],
        "help_binding" => vec![E::HelpOptions],
        "cli_output_claims" => vec![E::CliOutputExamples],
        "diagnosis_bound" => vec![E::DiagnosisBinding],
        "ingest_candidate_accounting" => vec![E::CandidateEnumeration],
        "ingest_source_binding" => vec![E::SourceValues],
        _ => Vec::new(),
    };
    let normalizers = if id == "ingest_source_binding" {
        vec![NormalizerId::Identity]
    } else {
        Vec::new()
    };
    (extraction, normalizers)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INGEST_EVAL: &str = r#"schema_version: commandagent.pack.eval/v0
pack:
  id: ingest-floor
  version: 1.0.0
  profile: ingest
  intent: create
checks:
  - id: pipeline_probe
    at: { kind: final_acceptance }
    params: { entry: pipeline/main.py, timeout_seconds: 30 }
  - id: ingest_source_binding
    at: { kind: final_acceptance }
    extraction: [source_binding.source_values]
    normalizers: [identity]
    params: {}
  - id: ingest_candidate_accounting
    at: { kind: final_acceptance }
    extraction: [accounting.enumerate]
    params: {}
  - id: ingest_format_schema
    at: { kind: final_acceptance }
    params: {}
  - id: ingest_rerun_consistency
    at: { kind: final_acceptance }
    params: { entry: pipeline/main.py, timeout_seconds: 30 }
"#;

    #[test]
    fn complete_floor_is_conformant() {
        let pack = super::super::parse_bytes(None, Some(INGEST_EVAL.as_bytes())).unwrap();
        let report = conform(&pack).unwrap();
        assert_eq!(report.floor_check_count, 5);
        assert_eq!(report.effective_check_count, 5);
    }

    #[test]
    fn reviewed_builtin_pack_matches_its_exact_byte_pin() {
        let directory =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("packs/builtin/ingest-create/1.0.0");
        let expected = std::fs::read_to_string(directory.join("pack.sha256")).unwrap();
        let report = conform_directory(&directory).unwrap();
        assert_eq!(report.exact_byte_hash, expected.trim());
        assert_eq!(report.status, "conformant");
    }

    #[test]
    fn nextjs_repository_fixture_adds_three_checks_without_changing_its_floor() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("packs/nextjs-acme/1.0.0");
        let expected = std::fs::read_to_string(directory.join("pack.sha256")).unwrap();
        let report = conform_directory(&directory).unwrap();
        assert_eq!(report.exact_byte_hash, expected.trim());
        assert_eq!(report.floor_check_count, 0);
        assert_eq!(report.effective_check_count, 3);
        assert_eq!(report.schema_count, 1);
    }

    #[test]
    fn omission_cannot_remove_the_builtin_floor() {
        let identity = PackIdentity {
            id: "ingest-floor".to_string(),
            version: "1.0.0".to_string(),
            profile: PackProfile::Ingest,
            intent: PackIntent::Create,
        };
        let floor = contract_floor(&identity).unwrap();
        let mut damaged = floor.clone();
        damaged.remove("ingest_candidate_accounting");
        assert!(matches!(
            guard_effective_floor(&floor, &damaged),
            Err(ConformanceError::FloorViolation(message))
                if message.contains("ingest_candidate_accounting")
        ));
    }

    #[test]
    fn weakened_or_moved_floor_checks_are_rejected() {
        for changed in [
            INGEST_EVAL.replace("timeout_seconds: 30", "timeout_seconds: 31"),
            INGEST_EVAL.replace(
                "id: ingest_format_schema\n    at: { kind: final_acceptance }",
                "id: ingest_format_schema\n    at: { kind: phase, id: ingest-run }",
            ),
        ] {
            let pack = super::super::parse_bytes(None, Some(changed.as_bytes())).unwrap();
            assert!(matches!(
                conform(&pack),
                Err(ConformanceError::FloorViolation(_))
            ));
        }
    }

    #[test]
    fn explicit_floor_removal_request_is_rejected_by_the_closed_schema() {
        let removal = INGEST_EVAL.replace(
            "  - id: ingest_format_schema\n",
            "  - id: ingest_format_schema\n    enabled: false\n",
        );
        assert!(super::super::parse_bytes(None, Some(removal.as_bytes())).is_err());
    }

    #[test]
    fn registered_timeout_narrowing_is_allowed() {
        let narrowed = INGEST_EVAL.replace("timeout_seconds: 30", "timeout_seconds: 20");
        let pack = super::super::parse_bytes(None, Some(narrowed.as_bytes())).unwrap();
        assert!(conform(&pack).is_ok());
    }

    #[test]
    fn required_extraction_cannot_be_removed() {
        let changed = INGEST_EVAL.replace("    extraction: [source_binding.source_values]\n", "");
        let pack = super::super::parse_bytes(None, Some(changed.as_bytes())).unwrap();
        assert!(matches!(
            conform(&pack),
            Err(ConformanceError::FloorViolation(_))
        ));
    }

    #[test]
    fn literal_gate_cannot_move_outside_the_identity_floor() {
        let assist = r#"schema_version: commandagent.pack.assist/v0
pack:
  id: ingest-floor
  version: 1.0.0
  profile: ingest
  intent: create
literals:
  - gate: cli_probe
    example: { format: text, value: example }
"#;
        let pack = super::super::parse_bytes(Some(assist.as_bytes()), None).unwrap();
        assert!(matches!(
            conform(&pack),
            Err(ConformanceError::FloorViolation(_))
        ));
    }
}
