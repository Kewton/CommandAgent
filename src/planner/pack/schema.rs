use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_yaml::Value;

use super::vocabulary::{
    AssistSource, CheckId, ExtractionId, InjectionPoint, NormalizerId, PackIntent, PackProfile,
    VocabularySource,
};

pub const ASSIST_SCHEMA_VERSION: &str = "commandagent.pack.assist/v0";
pub const EVAL_SCHEMA_VERSION: &str = "commandagent.pack.eval/v0";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackIdentity {
    pub id: String,
    pub version: String,
    pub profile: PackProfile,
    pub intent: PackIntent,
}

impl PackIdentity {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 64
            || !self.id.as_bytes()[0].is_ascii_lowercase()
            || self.id.split('-').any(|segment| {
                segment.is_empty()
                    || !segment
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            })
        {
            return Err(format!("invalid pack id `{}`", self.id));
        }
        let parts = self.version.split('.').collect::<Vec<_>>();
        if parts.len() != 3
            || parts.iter().any(|part| {
                part.is_empty()
                    || !part.bytes().all(|byte| byte.is_ascii_digit())
                    || (part.len() > 1 && part.starts_with('0'))
                    || part.parse::<u64>().is_err()
            })
        {
            return Err(format!(
                "pack version `{}` must be SemVer core MAJOR.MINOR.PATCH",
                self.version
            ));
        }
        let registered_profile = crate::planner::profile::ProfileId::parse(self.profile.as_str());
        if registered_profile.as_str() != self.profile.as_str() {
            return Err(format!("pack profile `{}` is not registered", self.profile));
        }
        if crate::planner::adjudication::contract::intent_contract(self.intent.as_str()).is_none() {
            return Err(format!("pack intent `{}` is not registered", self.intent));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssistPack {
    schema_version: String,
    pub pack: PackIdentity,
    #[serde(default)]
    inject: Vec<Injection>,
    #[serde(default)]
    literals: Vec<Literal>,
    #[serde(default)]
    vocabulary: Vec<Vocabulary>,
}

#[derive(Debug)]
pub struct AssistPackDocument {
    pub pack: PackIdentity,
    pub inject: Vec<Injection>,
    pub literals: Vec<Literal>,
    pub vocabulary: Vec<Vocabulary>,
}

impl AssistPack {
    pub(super) fn validate(self) -> Result<AssistPackDocument, String> {
        if self.schema_version != ASSIST_SCHEMA_VERSION {
            return Err(format!("schema_version must be `{ASSIST_SCHEMA_VERSION}`"));
        }
        self.pack.validate()?;
        if self.inject.is_empty() && self.literals.is_empty() && self.vocabulary.is_empty() {
            return Err("assist pack must declare inject, literals, or vocabulary".to_string());
        }
        unique_pairs(
            self.inject
                .iter()
                .map(|item| (item.point.as_str(), item.source.as_str())),
            "inject point/source",
        )?;
        unique_pairs(
            self.vocabulary
                .iter()
                .map(|item| (item.point.as_str(), item.source.as_str())),
            "vocabulary point/source",
        )?;
        for item in &self.inject {
            validate_point_owner(&self.pack, item.point)?;
            item.validate()?;
        }
        for literal in &self.literals {
            literal.validate()?;
        }
        for vocabulary in &self.vocabulary {
            validate_point_owner(&self.pack, vocabulary.point)?;
            vocabulary.validate()?;
        }
        Ok(AssistPackDocument {
            pack: self.pack,
            inject: self.inject,
            literals: self.literals,
            vocabulary: self.vocabulary,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Injection {
    pub point: InjectionPoint,
    pub source: AssistSource,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

impl Injection {
    fn validate(&self) -> Result<(), String> {
        if !self.required {
            return Err(format!(
                "source `{}` is not registered as optional in v0",
                self.source
            ));
        }
        validate_source_point(self.source, self.point)?;
        validate_source_params(self.source, &self.params)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Literal {
    pub gate: CheckId,
    pub example: LiteralExample,
}

impl Literal {
    fn validate(&self) -> Result<(), String> {
        if self.example.value.is_empty() || self.example.value.len() > 16_384 {
            return Err("literal example must be 1..16384 UTF-8 bytes".to_string());
        }
        if self.example.format == ExampleFormat::Json {
            serde_json::from_str::<serde_json::Value>(&self.example.value)
                .map_err(|error| format!("literal JSON example is invalid: {error}"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExampleFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiteralExample {
    pub format: ExampleFormat,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vocabulary {
    pub point: InjectionPoint,
    pub source: VocabularySource,
    pub mode: VocabularyMode,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

impl Vocabulary {
    fn validate(&self) -> Result<(), String> {
        if !self.required {
            return Err(format!(
                "vocabulary `{}` is not registered as optional in v0",
                self.source
            ));
        }
        let compatible = match self.source {
            VocabularySource::RequiredDelivery => true,
            VocabularySource::IngestCandidateIds => {
                self.point == InjectionPoint::ImplementIngestDelivery
            }
            VocabularySource::InvestigationWorkspaceFiles => self.point == InjectionPoint::Diagnose,
        };
        if !compatible {
            return Err(format!(
                "vocabulary `{}` is incompatible with point `{}`",
                self.source, self.point
            ));
        }
        match self.source {
            VocabularySource::RequiredDelivery if self.params.is_empty() => Ok(()),
            VocabularySource::RequiredDelivery => {
                Err("required_delivery_vocabulary takes no params".to_string())
            }
            VocabularySource::IngestCandidateIds => {
                validate_source_params(AssistSource::IngestCandidateIds, &self.params)
            }
            VocabularySource::InvestigationWorkspaceFiles => {
                validate_source_params(AssistSource::InvestigationWorkspaceFiles, &self.params)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VocabularyMode {
    Verbatim,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvalPack {
    schema_version: String,
    pub pack: PackIdentity,
    #[serde(default)]
    checks: Vec<CheckBinding>,
    #[serde(default)]
    schemas: Vec<ArtifactSchema>,
}

#[derive(Debug)]
pub struct EvalPackDocument {
    pub pack: PackIdentity,
    pub checks: Vec<CheckBinding>,
    pub schemas: Vec<ArtifactSchema>,
}

impl EvalPack {
    pub(super) fn validate(self) -> Result<EvalPackDocument, String> {
        if self.schema_version != EVAL_SCHEMA_VERSION {
            return Err(format!("schema_version must be `{EVAL_SCHEMA_VERSION}`"));
        }
        self.pack.validate()?;
        if self.checks.is_empty() && self.schemas.is_empty() {
            return Err("eval pack must declare checks or schemas".to_string());
        }
        unique(
            self.checks.iter().map(|check| check.id.as_str()),
            "check id",
        )?;
        unique(
            self.schemas.iter().map(|schema| schema.artifact.as_str()),
            "schema artifact",
        )?;
        for check in &self.checks {
            check.validate()?;
        }
        for schema in &self.schemas {
            schema.validate()?;
        }
        Ok(EvalPackDocument {
            pack: self.pack,
            checks: self.checks,
            schemas: self.schemas,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckBinding {
    pub id: CheckId,
    pub at: CheckAt,
    #[serde(default)]
    pub extraction: Vec<ExtractionId>,
    #[serde(default)]
    pub normalizers: Vec<NormalizerId>,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

impl CheckBinding {
    fn validate(&self) -> Result<(), String> {
        unique(
            self.extraction.iter().map(|item| item.as_str()),
            "extraction id",
        )?;
        unique(
            self.normalizers.iter().map(|item| item.as_str()),
            "normalizer id",
        )?;
        validate_check_params(&self.id, &self.params)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CheckAt {
    Phase { id: InjectionPoint },
    Stage { id: EvidenceStage },
    FinalAcceptance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceStage {
    Before,
    After,
    Diagnosis,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSchema {
    pub artifact: String,
    pub format: SchemaFormat,
    pub root: SchemaRoot,
    pub fields: Vec<SchemaField>,
    pub additional_fields: bool,
}

impl ArtifactSchema {
    fn validate(&self) -> Result<(), String> {
        crate::tools::path_guard::validate_workspace_relative(&self.artifact)
            .map_err(|error| format!("invalid schema artifact path: {error}"))?;
        if self.fields.is_empty() || self.fields.len() > 256 {
            return Err("schema fields must contain 1..256 entries".to_string());
        }
        unique(
            self.fields.iter().map(|field| field.name.as_str()),
            "field name",
        )?;
        self.fields.iter().try_for_each(SchemaField::validate)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaFormat {
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaRoot {
    Object,
    Array,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: SchemaFieldType,
    pub required: bool,
}

impl SchemaField {
    fn validate(&self) -> Result<(), String> {
        let mut bytes = self.name.bytes();
        let valid_first = bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_');
        if !valid_first
            || self.name.len() > 64
            || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(format!("invalid schema field name `{}`", self.name));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaFieldType {
    String,
    Number,
    Integer,
    Boolean,
    Object,
    Array,
    Null,
}

fn validate_source_point(source: AssistSource, point: InjectionPoint) -> Result<(), String> {
    let compatible = match source {
        AssistSource::IngestSnapshotStructure => point == InjectionPoint::DeclareIngestInspection,
        AssistSource::IngestCandidateIds => point == InjectionPoint::ImplementIngestDelivery,
        AssistSource::ReproducerOutput | AssistSource::InvestigationWorkspaceFiles => {
            point == InjectionPoint::Diagnose
        }
        AssistSource::FixFailureOutput => {
            matches!(point, InjectionPoint::IsolateCause | InjectionPoint::Repair)
        }
        AssistSource::VerifiedDiagnosis => {
            matches!(point, InjectionPoint::ImplementFix | InjectionPoint::Repair)
        }
        AssistSource::CliProbe => point == InjectionPoint::CliValidation,
        AssistSource::DataInspectionSchema => matches!(
            point,
            InjectionPoint::DataCleaning
                | InjectionPoint::DataAggregation
                | InjectionPoint::DataReporting
                | InjectionPoint::DataValidation
        ),
        AssistSource::BrowserInteraction => point == InjectionPoint::BuildVerification,
    };
    if compatible {
        Ok(())
    } else {
        Err(format!(
            "source `{source}` is incompatible with point `{point}`"
        ))
    }
}

fn validate_point_owner(identity: &PackIdentity, point: InjectionPoint) -> Result<(), String> {
    let compatible = match identity.intent {
        PackIntent::Create => match identity.profile {
            PackProfile::Data => matches!(
                point,
                InjectionPoint::DataInspection
                    | InjectionPoint::DataCleaning
                    | InjectionPoint::DataAggregation
                    | InjectionPoint::DataReporting
                    | InjectionPoint::DataValidation
            ),
            PackProfile::PythonCli => matches!(
                point,
                InjectionPoint::CliScaffold
                    | InjectionPoint::CliImplementation
                    | InjectionPoint::CliValidation
            ),
            PackProfile::Ingest => matches!(
                point,
                InjectionPoint::IngestImplement
                    | InjectionPoint::IngestRun
                    | InjectionPoint::IngestStructuralGate
                    | InjectionPoint::DeclareIngestInspection
                    | InjectionPoint::ImplementIngestDelivery
            ),
            PackProfile::Nextjs => matches!(
                point,
                InjectionPoint::ProjectSetup
                    | InjectionPoint::CoreImplementation
                    | InjectionPoint::ContractWiring
                    | InjectionPoint::BuildVerification
            ),
        },
        PackIntent::Investigate => matches!(
            point,
            InjectionPoint::ReproduceCandidate
                | InjectionPoint::Diagnose
                | InjectionPoint::BindVerify
        ),
        PackIntent::Fix => matches!(
            point,
            InjectionPoint::ReproduceBefore
                | InjectionPoint::IsolateCause
                | InjectionPoint::ImplementFix
                | InjectionPoint::Repair
                | InjectionPoint::VerifyAfter
                | InjectionPoint::VerifyRegressions
        ),
    };
    if compatible {
        Ok(())
    } else {
        Err(format!(
            "point `{point}` is not present in the resolved {} × {} plan",
            identity.profile, identity.intent
        ))
    }
}

fn validate_source_params(
    source: AssistSource,
    params: &BTreeMap<String, Value>,
) -> Result<(), String> {
    match source {
        AssistSource::IngestSnapshotStructure => validate_unsigned_params(
            params,
            &[
                ("max_files", 8),
                ("max_entries", 256),
                ("max_depth", 4),
                ("max_bytes_per_file", 65_536),
                ("leading_lines", 12),
                ("candidate_windows", 2),
                ("max_chars_per_line", 200),
            ],
        ),
        AssistSource::IngestCandidateIds => validate_unsigned_params(
            params,
            &[("max_ids", 1_024), ("max_rendered_bytes", 65_536)],
        ),
        AssistSource::ReproducerOutput => validate_fields_and_cap(
            params,
            &["command", "stdout", "stderr", "last_non_empty", "traceback"],
            "max_chars_per_stream",
            500,
        ),
        AssistSource::InvestigationWorkspaceFiles => validate_unsigned_params(
            params,
            &[("max_files", 64), ("max_entries", 1_024), ("max_depth", 8)],
        ),
        AssistSource::FixFailureOutput => validate_fields_and_cap(
            params,
            &[
                "location",
                "error_kind",
                "message",
                "excerpt",
                "selected_target",
                "artifact_presence",
            ],
            "max_chars_per_excerpt",
            500,
        ),
        AssistSource::VerifiedDiagnosis => {
            reject_unknown_params(params, &["render"])?;
            match params.get("render") {
                None => Ok(()),
                Some(Value::String(value)) if value == "full" => Ok(()),
                Some(_) => Err("verified_diagnosis.render must be `full`".to_string()),
            }
        }
        AssistSource::CliProbe => {
            reject_unknown_params(params, &["case", "fields", "max_bytes_per_stream"])?;
            if let Some(value) = params.get("case")
                && !matches!(value, Value::String(case) if matches!(case.as_str(), "normal" | "invalid"))
            {
                return Err("cli_probe.case must be `normal` or `invalid`".to_string());
            }
            validate_fields(
                params.get("fields"),
                &["argv", "exit_code", "stdout", "stderr"],
            )?;
            validate_optional_unsigned(params, "max_bytes_per_stream", 24_000)
        }
        AssistSource::DataInspectionSchema => {
            reject_unknown_params(params, &["fields"])?;
            validate_fields(
                params.get("fields"),
                &[
                    "input_path",
                    "column_names",
                    "input_row_count",
                    "type_summaries",
                    "distinct_values",
                    "sample_rows",
                ],
            )
        }
        AssistSource::BrowserInteraction => {
            reject_unknown_params(params, &["fields"])?;
            validate_fields(
                params.get("fields"),
                &[
                    "dispatched_inputs",
                    "observed_state",
                    "hook_status",
                    "surface",
                    "outcome",
                ],
            )
        }
    }
}

fn validate_check_params(id: &CheckId, params: &BTreeMap<String, Value>) -> Result<(), String> {
    let catalog_registered = crate::planner::capability_catalog::registry()
        .iter()
        .any(|spec| spec.id == id.as_str());
    if !catalog_registered {
        return if params.is_empty() {
            Ok(())
        } else {
            Err(format!("intent check `{}` takes no params", id.as_str()))
        };
    }
    let mut table = toml::value::Table::new();
    for (name, value) in params {
        table.insert(name.clone(), yaml_to_toml(value)?);
    }
    crate::planner::capability_catalog::resolve(id.as_str(), &table)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn yaml_to_toml(value: &Value) -> Result<toml::Value, String> {
    match value {
        Value::String(value) => Ok(toml::Value::String(value.clone())),
        Value::Number(value) => value
            .as_i64()
            .map(toml::Value::Integer)
            .ok_or_else(|| "pack params accept only signed integer numbers".to_string()),
        Value::Sequence(values) => values
            .iter()
            .map(yaml_to_toml)
            .collect::<Result<Vec<_>, _>>()
            .map(toml::Value::Array),
        _ => Err("pack params accept only strings, integers, and sequences".to_string()),
    }
}

fn validate_fields_and_cap(
    params: &BTreeMap<String, Value>,
    allowed_fields: &[&str],
    cap_name: &str,
    cap: u64,
) -> Result<(), String> {
    reject_unknown_params(params, &["fields", cap_name])?;
    validate_fields(params.get("fields"), allowed_fields)?;
    validate_optional_unsigned(params, cap_name, cap)
}

fn validate_fields(value: Option<&Value>, allowed: &[&str]) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let Value::Sequence(values) = value else {
        return Err("fields must be a sequence".to_string());
    };
    if values.is_empty() {
        return Err("fields must not be empty".to_string());
    }
    let mut seen = BTreeSet::new();
    for value in values {
        let Value::String(value) = value else {
            return Err("fields entries must be strings".to_string());
        };
        if !allowed.contains(&value.as_str()) {
            return Err(format!("unknown source field `{value}`"));
        }
        if !seen.insert(value) {
            return Err(format!("duplicate source field `{value}`"));
        }
    }
    Ok(())
}

fn validate_unsigned_params(
    params: &BTreeMap<String, Value>,
    allowed: &[(&str, u64)],
) -> Result<(), String> {
    let names = allowed.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    reject_unknown_params(params, &names)?;
    allowed
        .iter()
        .try_for_each(|(name, cap)| validate_optional_unsigned(params, name, *cap))
}

fn validate_optional_unsigned(
    params: &BTreeMap<String, Value>,
    name: &str,
    cap: u64,
) -> Result<(), String> {
    let Some(value) = params.get(name) else {
        return Ok(());
    };
    let Some(value) = value.as_u64() else {
        return Err(format!("{name} must be an unsigned integer"));
    };
    if value == 0 || value > cap {
        return Err(format!("{name} must be between 1 and {cap}"));
    }
    Ok(())
}

fn reject_unknown_params(params: &BTreeMap<String, Value>, allowed: &[&str]) -> Result<(), String> {
    if let Some(name) = params.keys().find(|name| !allowed.contains(&name.as_str())) {
        return Err(format!("unknown source parameter `{name}`"));
    }
    Ok(())
}

fn unique<'a>(values: impl Iterator<Item = &'a str>, label: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(format!("duplicate {label} `{value}`"));
        }
    }
    Ok(())
}

fn unique_pairs<'a>(
    values: impl Iterator<Item = (&'a str, &'a str)>,
    label: &str,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(format!("duplicate {label} `{}:{}`", value.0, value.1));
        }
    }
    Ok(())
}

const fn default_true() -> bool {
    true
}
