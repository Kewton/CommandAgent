use std::collections::BTreeMap;

use serde::Deserialize;
use toml::value::Table;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestMetadata {
    pub id: String,
    pub display_name: String,
    pub schema_version: SchemaVersion,
    pub status: ManifestStatus,
    /// Required for externally supplied profiles, which have no compiled-in
    /// route entry. Embedded profiles keep their catalog-declared families.
    #[serde(default)]
    pub task_family: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum SchemaVersion {
    #[serde(rename = "v1")]
    V1,
}

impl SchemaVersion {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestStatus {
    Draft,
    Admitted,
}

impl ManifestStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Admitted => "admitted",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestPlan {
    pub profile: String,
    pub style: String,
    pub intent: String,
    pub placeholders: PlanPlaceholders,
    pub phases: Vec<ManifestPlanPhase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPlaceholders {
    pub goal: String,
    #[serde(default)]
    pub port: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestPlanPhase {
    pub id: String,
    pub prompt: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepTemplates {
    pub scaffold: ScaffoldTemplateMatcher,
    pub build_verify: PhaseKeywordMatcher,
    pub implementation_kill: PhaseKeywordMatcher,
    pub ownership: TemplateOwnership,
    pub artifacts: TemplateArtifacts,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScaffoldTemplateMatcher {
    pub phase: Vec<String>,
    pub phase_id: Vec<String>,
    pub port_phase_markers: Vec<String>,
    pub port_script_phase: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseKeywordMatcher {
    pub phase: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateOwnership {
    pub setup_classifier: SetupClassifier,
    pub template_owned_artifacts: TemplateOwnedArtifacts,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupClassifier {
    pub package_phrases: Vec<String>,
    pub package_tokens: Vec<String>,
    pub scaffold_phrases: Vec<String>,
    pub scaffold_tokens: Vec<String>,
    pub scaffold_setup_markers: Vec<String>,
    pub scaffold_project_marker: String,
    pub scaffold_dependency_exclusion: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateOwnedArtifacts {
    pub package_phrases: Vec<String>,
    pub package_tokens: Vec<String>,
    pub scaffold_phrases: Vec<String>,
    pub scaffold_tokens: Vec<String>,
    pub package_manifest_names: Vec<String>,
    pub artifact_path_suffixes: Vec<String>,
    pub artifact_path_contains: Vec<String>,
    pub package_check_marker: String,
    pub scaffold_check_marker: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateArtifacts {
    pub package_script_build: String,
    pub package_script_dev: String,
    pub package_script_start: String,
    pub required_hooks: Vec<String>,
    pub scaffold_files: Vec<String>,
    pub tailwind_config_rels: Vec<String>,
    pub tailwind_config: String,
    pub tailwind_config_cjs: String,
    pub package_json: String,
    pub tsconfig: String,
    pub postcss_config: String,
    pub tailwind_css: String,
    pub global_d_ts: String,
    pub layout_tsx: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRequirements {
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub groups: Vec<ArtifactGroup>,
}

impl ArtifactRequirements {
    pub fn preferred_paths(&self) -> Vec<String> {
        let mut paths = self.required.clone();
        paths.extend(self.groups.iter().map(|group| group.preferred.clone()));
        paths
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactGroup {
    pub id: String,
    pub cardinality: ArtifactCardinality,
    pub paths: Vec<String>,
    pub preferred: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactCardinality {
    EitherOf,
    ExactlyOneOf,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VocabularyReference {
    pub source: SharedKnowledgeSource,
    pub sections: Vec<VocabularySection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedKnowledgeSource {
    EvidenceKnowledge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum VocabularySection {
    #[serde(rename = "vocabulary")]
    Vocabulary,
    #[serde(rename = "goal_hints.translations")]
    GoalHintTranslations,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestGuidance {
    pub variants: BTreeMap<String, GuidanceVariant>,
}

impl ManifestGuidance {
    pub fn message(&self, variant: &str, message: &str) -> Option<&str> {
        self.variants
            .get(variant)?
            .messages
            .get(message)
            .map(String::as_str)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuidanceVariant {
    pub triggers: Vec<GuidanceTrigger>,
    pub messages: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuidanceTrigger {
    pub condition: GuidanceTriggerCondition,
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidanceTriggerCondition {
    Always,
    CheckFailure,
    EvidenceKey,
    FailureKindPrefix,
    GoalSignal,
    HiddenPath,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckBinding {
    pub id: String,
    #[serde(default)]
    pub phases: Option<Vec<String>>,
    #[serde(default)]
    pub params: Table,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceTargetsReference {
    #[serde(default)]
    pub source: Option<SharedKnowledgeSource>,
    #[serde(default)]
    pub section: Option<EvidenceTargetsSection>,
    #[serde(default)]
    pub mappings: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum EvidenceTargetsSection {
    #[serde(rename = "repair_targets")]
    RepairTargets,
}
