use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::planner::capability_catalog::{self, CatalogError, ResolvedCapability};

pub(crate) mod check_phase_scope;
pub mod commands;
pub mod overlay;
mod schema_v1;
mod schema_v2;
pub mod source;
pub mod supply;
mod v1_validation;
mod validation;
pub use schema_v1::*;
pub use source::{LoadedManifest, ManifestOrigin, ManifestSource, load_extension_manifests};

const NEXTJS_MANIFEST_TOML: &str = include_str!("profiles/nextjs/manifest.toml");

pub const MANIFEST_V1_SECTIONS: &[&str] = &[
    "metadata",
    "plan",
    "step_templates",
    "artifacts",
    "vocabulary",
    "guidance",
    "checks",
    "evidence_targets",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestV1 {
    pub metadata: ManifestMetadata,
    pub plan: ManifestPlan,
    pub step_templates: StepTemplates,
    pub artifacts: ArtifactRequirements,
    pub vocabulary: VocabularyReference,
    pub guidance: ManifestGuidance,
    pub checks: BTreeMap<String, Vec<CheckBinding>>,
    pub evidence_targets: EvidenceTargetsReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCheck {
    pub id: String,
    pub phases: Option<Vec<String>>,
    pub capability: ResolvedCapability,
}

pub type ResolvedCheckBindings = BTreeMap<String, Vec<ResolvedCheck>>;

#[derive(Debug)]
pub enum ManifestError {
    Parse(toml::de::Error),
    Invalid {
        field: &'static str,
        reason: String,
    },
    CheckBinding {
        binding: String,
        index: usize,
        id: String,
        source: Box<CatalogError>,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "manifest TOML is invalid: {err}"),
            Self::Invalid { field, reason } => {
                write!(f, "manifest field `{field}` is invalid: {reason}")
            }
            Self::CheckBinding {
                binding,
                index,
                id,
                source,
            } => write!(
                f,
                "check binding `{binding}` entry {index} (`{id}`) is invalid: {source}"
            ),
        }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(err) => Some(err),
            Self::CheckBinding { source, .. } => Some(source.as_ref()),
            Self::Invalid { .. } => None,
        }
    }
}

impl ManifestV1 {
    pub fn from_toml(input: &str) -> Result<Self, ManifestError> {
        let manifest = toml::from_str::<Self>(input).map_err(ManifestError::Parse)?;
        if manifest.metadata.schema_version != SchemaVersion::V1 {
            return Err(ManifestError::Invalid {
                field: "metadata.schema_version",
                reason: "ManifestV1 accepts only `v1`".to_string(),
            });
        }
        manifest.resolve()?;
        Ok(manifest)
    }

    pub fn resolve(&self) -> Result<ResolvedCheckBindings, ManifestError> {
        self.validate_structure()?;
        let mut resolved = BTreeMap::new();
        for (binding, checks) in &self.checks {
            let mut entries = Vec::with_capacity(checks.len());
            for (index, check) in checks.iter().enumerate() {
                if check.id == crate::planner::declarative_command_checks::ID
                    && check.phases.is_some()
                {
                    return Err(ManifestError::Invalid {
                        field: "checks.<binding>[].phases",
                        reason:
                            "command_check is registered only for final acceptance; omit phases"
                                .to_string(),
                    });
                }
                let capability =
                    capability_catalog::resolve(&check.id, &check.params).map_err(|source| {
                        ManifestError::CheckBinding {
                            binding: binding.clone(),
                            index,
                            id: check.id.clone(),
                            source: Box::new(source),
                        }
                    })?;
                entries.push(ResolvedCheck {
                    id: check.id.clone(),
                    phases: check.phases.clone(),
                    capability,
                });
            }
            resolved.insert(binding.clone(), entries);
        }
        Ok(resolved)
    }

    fn validate_structure(&self) -> Result<(), ManifestError> {
        validation::validate(self)?;
        v1_validation::validate(self)
    }
}

pub(crate) fn external_manifest_from_toml(input: &str) -> Result<ManifestV1, ManifestError> {
    schema_v2::from_external_toml(input)
}

pub fn nextjs_manifest() -> &'static ManifestV1 {
    static MANIFEST: OnceLock<ManifestV1> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        ManifestV1::from_toml(NEXTJS_MANIFEST_TOML)
            .expect("embedded Next.js profile manifest.toml must parse and resolve")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::profiles::nextjs::knowledge;

    #[test]
    fn embedded_manifest_keeps_existing_nextjs_knowledge_values() {
        let manifest = nextjs_manifest();
        let existing = knowledge::get();

        assert_eq!(manifest.plan.profile, existing.preset.profile);
        assert_eq!(manifest.plan.style, existing.preset.style);
        assert_eq!(manifest.plan.intent, existing.preset.intent);
        assert_eq!(manifest.plan.phases.len(), existing.preset.phases.len());
        for (manifest_phase, existing_phase) in
            manifest.plan.phases.iter().zip(&existing.preset.phases)
        {
            assert_eq!(manifest_phase.id, existing_phase.id);
            assert_eq!(manifest_phase.prompt, existing_phase.prompt);
        }

        let templates = &manifest.step_templates;
        assert_eq!(
            templates.scaffold.phase,
            existing.deterministic_keywords.scaffold_phase
        );
        assert_eq!(
            templates.scaffold.phase_id,
            existing.deterministic_keywords.scaffold_phase_id
        );
        assert_eq!(
            templates.scaffold.port_phase_markers,
            existing.deterministic_keywords.port_phase_markers
        );
        assert_eq!(
            templates.scaffold.port_script_phase,
            existing.deterministic_keywords.port_script_phase
        );
        assert_eq!(
            templates.build_verify.phase,
            existing.deterministic_keywords.build_verify_phase
        );
        assert_eq!(
            templates.implementation_kill.phase,
            existing.deterministic_keywords.implementation_phase
        );
        assert_eq!(
            templates.ownership.setup_classifier.package_phrases,
            existing.setup_classifier.package_phrases
        );
        assert_eq!(
            templates.ownership.setup_classifier.package_tokens,
            existing.setup_classifier.package_tokens
        );
        assert_eq!(
            templates.ownership.setup_classifier.scaffold_phrases,
            existing.setup_classifier.scaffold_phrases
        );
        assert_eq!(
            templates.ownership.setup_classifier.scaffold_tokens,
            existing.setup_classifier.scaffold_tokens
        );
        assert_eq!(
            templates.ownership.setup_classifier.scaffold_setup_markers,
            existing.setup_classifier.scaffold_setup_markers
        );
        assert_eq!(
            templates.ownership.setup_classifier.scaffold_project_marker,
            existing.setup_classifier.scaffold_project_marker
        );
        assert_eq!(
            templates
                .ownership
                .setup_classifier
                .scaffold_dependency_exclusion,
            existing.setup_classifier.scaffold_dependency_exclusion
        );
        let owned = &templates.ownership.template_owned_artifacts;
        assert_eq!(
            owned.package_phrases,
            existing.template_owned_artifacts.package_phrases
        );
        assert_eq!(
            owned.package_tokens,
            existing.template_owned_artifacts.package_tokens
        );
        assert_eq!(
            owned.scaffold_phrases,
            existing.template_owned_artifacts.scaffold_phrases
        );
        assert_eq!(
            owned.scaffold_tokens,
            existing.template_owned_artifacts.scaffold_tokens
        );
        assert_eq!(
            owned.package_manifest_names,
            existing.template_owned_artifacts.package_manifest_names
        );
        assert_eq!(
            templates
                .ownership
                .template_owned_artifacts
                .artifact_path_suffixes,
            existing.template_owned_artifacts.artifact_path_suffixes
        );
        assert_eq!(
            owned.artifact_path_contains,
            existing.template_owned_artifacts.artifact_path_contains
        );
        assert_eq!(
            owned.package_check_marker,
            existing.template_owned_artifacts.package_check_marker
        );
        assert_eq!(
            owned.scaffold_check_marker,
            existing.template_owned_artifacts.scaffold_check_marker
        );

        let artifacts = &templates.artifacts;
        let canonical = &existing.canonical;
        assert_eq!(
            artifacts.package_script_build,
            canonical.package_script_build
        );
        assert_eq!(artifacts.package_script_dev, canonical.package_script_dev);
        assert_eq!(
            artifacts.package_script_start,
            canonical.package_script_start
        );
        assert_eq!(artifacts.required_hooks, canonical.required_hooks);
        assert_eq!(artifacts.scaffold_files, canonical.scaffold_files);
        assert_eq!(
            artifacts.tailwind_config_rels,
            canonical.tailwind_config_rels
        );
        assert_eq!(artifacts.tailwind_config, canonical.tailwind_config);
        assert_eq!(artifacts.tailwind_config_cjs, canonical.tailwind_config_cjs);
        assert_eq!(artifacts.package_json, canonical.package_json);
        assert_eq!(artifacts.tsconfig, canonical.tsconfig);
        assert_eq!(artifacts.postcss_config, canonical.postcss_config);
        assert_eq!(artifacts.tailwind_css, canonical.tailwind_css);
        assert_eq!(artifacts.global_d_ts, canonical.global_d_ts);
        assert_eq!(artifacts.layout_tsx, canonical.layout_tsx);

        let guidance = &manifest.guidance;
        let message = |variant, name| guidance.message(variant, name).unwrap();
        assert_eq!(
            message("interaction", "generic_interaction"),
            existing.repair_guidance.generic_interaction
        );
        assert_eq!(
            message("interaction", "start_interaction"),
            existing.repair_guidance.start_interaction
        );
        assert_eq!(
            message("canvas_game", "canvas_game_interaction"),
            existing.repair_guidance.canvas_game_interaction
        );
        assert_eq!(
            message("canvas_game", "canvas_render_loop_checklist"),
            existing.repair_guidance.canvas_render_loop_checklist
        );
        assert_eq!(
            message("canvas_game", "canvas_input_wiring_checklist"),
            existing.repair_guidance.canvas_input_wiring_checklist
        );
        assert_eq!(
            message("persistence", "persistence"),
            existing.repair_guidance.persistence
        );
        assert_eq!(
            message("contracts", "state_binding_contract"),
            existing.contracts.state_binding_contract
        );
        assert_eq!(
            message("contracts", "input_coupled_dimension_requirement"),
            existing.contracts.input_coupled_dimension_requirement
        );
        assert_eq!(
            message("contracts", "contract_attribute_missing_kind"),
            existing.contracts.contract_attribute_missing_kind
        );
        assert_eq!(
            message("contracts", "contract_attribute_guidance"),
            existing.contracts.contract_attribute_guidance
        );
        assert_eq!(
            message("contracts", "state_requirement"),
            existing.contracts.state_requirement
        );
        assert_eq!(
            message("contracts", "restart_requirement"),
            existing.contracts.restart_requirement
        );
        assert_eq!(
            message("contracts", "input_requirement"),
            existing.contracts.input_requirement
        );
        assert_eq!(
            message("contracts", "primary_requirement"),
            existing.contracts.primary_requirement
        );
        assert_eq!(
            message("contracts", "state_example"),
            existing.contracts.state_example
        );
        assert_eq!(
            message("contracts", "restart_example"),
            existing.contracts.restart_example
        );
        assert_eq!(
            message("contracts", "input_example"),
            existing.contracts.input_example
        );
        assert_eq!(
            message("contracts", "primary_example"),
            existing.contracts.primary_example
        );
    }

    #[test]
    fn embedded_loader_parses_once() {
        assert!(std::ptr::eq(nextjs_manifest(), nextjs_manifest()));
    }

    #[test]
    fn unknown_root_section_is_rejected() {
        let invalid = format!("{NEXTJS_MANIFEST_TOML}\n[unknown_section]\nvalue = true\n");
        assert!(matches!(
            ManifestV1::from_toml(&invalid),
            Err(ManifestError::Parse(_))
        ));
    }

    #[test]
    fn unknown_check_id_is_rejected_during_load() {
        let invalid = NEXTJS_MANIFEST_TOML.replacen(
            "id = \"next_build_verify\"",
            "id = \"unknown_build_check\"",
            1,
        );
        assert!(matches!(
            ManifestV1::from_toml(&invalid),
            Err(ManifestError::CheckBinding { source, .. })
                if matches!(source.as_ref(), CatalogError::UnknownId(id) if id == "unknown_build_check")
        ));
    }

    #[test]
    fn invalid_check_params_are_rejected_during_load() {
        let invalid = NEXTJS_MANIFEST_TOML.replacen("port = 3011", "port = \"3011\"", 1);
        assert!(matches!(
            ManifestV1::from_toml(&invalid),
            Err(ManifestError::CheckBinding { source, .. })
                if matches!(
                    source.as_ref(),
                    CatalogError::TypeMismatch { parameter, .. } if parameter == "port"
                )
        ));
    }

    #[test]
    fn invalid_status_is_rejected() {
        let invalid =
            NEXTJS_MANIFEST_TOML.replacen("status = \"admitted\"", "status = \"retired\"", 1);
        assert!(matches!(
            ManifestV1::from_toml(&invalid),
            Err(ManifestError::Parse(_))
        ));
    }
}
