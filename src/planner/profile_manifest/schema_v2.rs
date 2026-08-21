//! Compact, profile-neutral schema for externally supplied draft profiles.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{
    ArtifactRequirements, CheckBinding, EvidenceTargetsReference, EvidenceTargetsSection,
    ManifestError, ManifestGuidance, ManifestMetadata, ManifestPlan, ManifestPlanPhase,
    ManifestStatus, ManifestV1, PhaseKeywordMatcher, PlanPlaceholders, ScaffoldTemplateMatcher,
    SchemaVersion, SetupClassifier, SharedKnowledgeSource, StepTemplates, TemplateArtifacts,
    TemplateOwnedArtifacts, TemplateOwnership, VocabularyReference, VocabularySection,
};

#[derive(Debug, Deserialize)]
struct ManifestHeader {
    metadata: ManifestHeaderMetadata,
}

#[derive(Debug, Deserialize)]
struct ManifestHeaderMetadata {
    schema_version: SchemaVersion,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestV2 {
    metadata: ManifestMetadataV2,
    plan: ManifestPlanV2,
    artifacts: ArtifactRequirements,
    guidance: ManifestGuidance,
    checks: BTreeMap<String, Vec<CheckBinding>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestMetadataV2 {
    id: String,
    display_name: String,
    schema_version: SchemaVersion,
    #[serde(default)]
    status: Option<ManifestStatus>,
    task_family: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestPlanV2 {
    #[serde(default = "default_style")]
    style: String,
    intent: String,
    phases: Vec<ManifestPlanPhase>,
}

pub(super) fn from_external_toml(input: &str) -> Result<ManifestV1, ManifestError> {
    let header = toml::from_str::<ManifestHeader>(input).map_err(ManifestError::Parse)?;
    match header.metadata.schema_version {
        SchemaVersion::V1 => ManifestV1::from_toml(input),
        SchemaVersion::V2 => ManifestV2::from_toml(input),
    }
}

impl ManifestV2 {
    fn from_toml(input: &str) -> Result<ManifestV1, ManifestError> {
        let manifest = toml::from_str::<Self>(input).map_err(ManifestError::Parse)?;
        if manifest.metadata.schema_version != SchemaVersion::V2 {
            return Err(ManifestError::Invalid {
                field: "metadata.schema_version",
                reason: "ManifestV2 accepts only `v2`".to_string(),
            });
        }
        let expanded = manifest.expand();
        expanded.resolve()?;
        Ok(expanded)
    }

    fn expand(self) -> ManifestV1 {
        let id = self.metadata.id;
        let scaffold_files = self.artifacts.preferred_paths();
        ManifestV1 {
            metadata: ManifestMetadata {
                id: id.clone(),
                display_name: self.metadata.display_name,
                schema_version: SchemaVersion::V2,
                status: self.metadata.status.unwrap_or(ManifestStatus::Draft),
                task_family: Some(self.metadata.task_family),
            },
            plan: ManifestPlan {
                profile: id,
                style: self.plan.style,
                intent: self.plan.intent,
                placeholders: PlanPlaceholders {
                    goal: "{goal}".to_string(),
                    port: None,
                },
                phases: self.plan.phases,
            },
            step_templates: neutral_step_templates(scaffold_files),
            artifacts: self.artifacts,
            vocabulary: VocabularyReference {
                source: SharedKnowledgeSource::EvidenceKnowledge,
                sections: vec![
                    VocabularySection::Vocabulary,
                    VocabularySection::GoalHintTranslations,
                ],
            },
            guidance: self.guidance,
            checks: self.checks,
            evidence_targets: EvidenceTargetsReference {
                source: Some(SharedKnowledgeSource::EvidenceKnowledge),
                section: Some(EvidenceTargetsSection::RepairTargets),
                mappings: BTreeMap::new(),
            },
        }
    }
}

fn neutral_step_templates(scaffold_files: Vec<String>) -> StepTemplates {
    let empty_matcher = || PhaseKeywordMatcher { phase: Vec::new() };
    StepTemplates {
        scaffold: ScaffoldTemplateMatcher {
            phase: Vec::new(),
            phase_id: Vec::new(),
            port_phase_markers: Vec::new(),
            port_script_phase: Vec::new(),
        },
        build_verify: empty_matcher(),
        implementation_kill: empty_matcher(),
        ownership: TemplateOwnership {
            setup_classifier: SetupClassifier {
                package_phrases: Vec::new(),
                package_tokens: Vec::new(),
                scaffold_phrases: Vec::new(),
                scaffold_tokens: Vec::new(),
                scaffold_setup_markers: Vec::new(),
                scaffold_project_marker: "external-profile".to_string(),
                scaffold_dependency_exclusion: "external-profile".to_string(),
            },
            template_owned_artifacts: TemplateOwnedArtifacts {
                package_phrases: Vec::new(),
                package_tokens: Vec::new(),
                scaffold_phrases: Vec::new(),
                scaffold_tokens: Vec::new(),
                package_manifest_names: Vec::new(),
                artifact_path_suffixes: Vec::new(),
                artifact_path_contains: Vec::new(),
                package_check_marker: "external-profile".to_string(),
                scaffold_check_marker: "external-profile".to_string(),
            },
        },
        artifacts: TemplateArtifacts {
            package_script_build: "build".to_string(),
            package_script_dev: "dev".to_string(),
            package_script_start: "start".to_string(),
            required_hooks: Vec::new(),
            scaffold_files,
            tailwind_config_rels: Vec::new(),
            tailwind_config: "tailwind.config.ts".to_string(),
            tailwind_config_cjs: "tailwind.config.cjs".to_string(),
            package_json: "package.json".to_string(),
            tsconfig: "tsconfig.json".to_string(),
            postcss_config: "postcss.config.js".to_string(),
            tailwind_css: "tailwind.css".to_string(),
            global_d_ts: "global.d.ts".to_string(),
            layout_tsx: "layout.tsx".to_string(),
        },
    }
}

fn default_style() -> String {
    "default".to_string()
}
