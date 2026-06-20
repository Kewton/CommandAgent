use crate::agent::step_runner::profiles::ProfileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ArtifactProvenance {
    UserRequiredArtifact,
    StepExpectedPath,
    PhaseRequiredArtifact,
    WorkspaceObservation,
    ProfileFact,
    VerifierEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactKind {
    RouteEntry,
    RouteInfrastructure,
    UiSource,
    StyleSource,
    RuntimeSource,
    TestSource,
    Manifest,
    Config,
    GeneratedDeclaration,
    DependencyCache,
    BuildOutput,
    RawInput,
    DerivedOutput,
    Documentation,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ArtifactEligibility {
    pub(crate) route_integration: bool,
    pub(crate) source_integration: bool,
    pub(crate) verification_target: bool,
    pub(crate) recovery_target: bool,
    pub(crate) protected_input: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassifiedArtifact {
    pub(crate) path: String,
    pub(crate) provenance: ArtifactProvenance,
    pub(crate) kind: ArtifactKind,
    pub(crate) eligibility: ArtifactEligibility,
}

impl ClassifiedArtifact {
    fn new(
        path: String,
        provenance: ArtifactProvenance,
        kind: ArtifactKind,
        eligibility: ArtifactEligibility,
    ) -> Self {
        Self {
            path,
            provenance,
            kind,
            eligibility,
        }
    }
}

/// Classify profile paths before any obligation, verification, or recovery
/// producer reasons about them. Future producers should consume
/// `ClassifiedArtifact` values instead of parsing rendered profile text or
/// workspace-entry tokens, and classification must not execute tools or grant
/// workflow/retry authority.
pub(crate) fn classify_profile_artifact(
    profile: ProfileId,
    path: &str,
    provenance: ArtifactProvenance,
) -> ClassifiedArtifact {
    match profile {
        ProfileId::NextJs => classify_nextjs_artifact(path, provenance),
        ProfileId::Python => classify_python_artifact(path, provenance),
        ProfileId::Rust => classify_rust_artifact(path, provenance),
        ProfileId::Docs => classify_docs_artifact(path, provenance),
        ProfileId::DataAnalysis | ProfileId::DataPipeline => {
            classify_data_artifact(path, provenance)
        }
        ProfileId::Generic | ProfileId::Investigation => unknown_artifact(path, provenance),
    }
}

pub(crate) fn classify_nextjs_artifact(
    path: &str,
    provenance: ArtifactProvenance,
) -> ClassifiedArtifact {
    let path = normalize_path(path);
    let kind = if is_dependency_cache_path(&path) {
        ArtifactKind::DependencyCache
    } else if is_build_output_path(&path) {
        ArtifactKind::BuildOutput
    } else if is_generated_declaration(&path) {
        ArtifactKind::GeneratedDeclaration
    } else if is_nextjs_route_entry(&path) {
        ArtifactKind::RouteEntry
    } else if is_nextjs_route_infrastructure(&path) {
        ArtifactKind::RouteInfrastructure
    } else if is_manifest_path(&path) {
        ArtifactKind::Manifest
    } else if is_config_path(&path) {
        ArtifactKind::Config
    } else if is_nextjs_style_source_path(&path) {
        ArtifactKind::StyleSource
    } else if is_nextjs_ui_source_path(&path) {
        ArtifactKind::UiSource
    } else {
        ArtifactKind::Unknown
    };
    ClassifiedArtifact::new(path, provenance, kind, nextjs_eligibility(kind, provenance))
}

pub(crate) fn classify_python_artifact(
    path: &str,
    provenance: ArtifactProvenance,
) -> ClassifiedArtifact {
    let path = normalize_path(path);
    let kind = if is_dependency_cache_path(&path) {
        ArtifactKind::DependencyCache
    } else if is_python_build_output_path(&path) || is_build_output_path(&path) {
        ArtifactKind::BuildOutput
    } else if is_python_manifest_path(&path) {
        ArtifactKind::Manifest
    } else if is_python_config_path(&path) {
        ArtifactKind::Config
    } else if path.starts_with("tests/") && extension_is(&path, &["py"]) {
        ArtifactKind::TestSource
    } else if extension_is(&path, &["py"]) {
        ArtifactKind::RuntimeSource
    } else {
        ArtifactKind::Unknown
    };
    ClassifiedArtifact::new(path, provenance, kind, source_eligibility(kind))
}

pub(crate) fn classify_rust_artifact(
    path: &str,
    provenance: ArtifactProvenance,
) -> ClassifiedArtifact {
    let path = normalize_path(path);
    let kind = if path.starts_with("target/") {
        ArtifactKind::BuildOutput
    } else if matches!(path.as_str(), "Cargo.toml" | "Cargo.lock") {
        ArtifactKind::Manifest
    } else if matches!(
        path.as_str(),
        "rustfmt.toml" | ".rustfmt.toml" | "clippy.toml"
    ) {
        ArtifactKind::Config
    } else if (path.starts_with("tests/") || path.starts_with("benches/"))
        && extension_is(&path, &["rs"])
    {
        ArtifactKind::TestSource
    } else if (path.starts_with("src/") || path.starts_with("examples/"))
        && extension_is(&path, &["rs"])
    {
        ArtifactKind::RuntimeSource
    } else {
        ArtifactKind::Unknown
    };
    ClassifiedArtifact::new(path, provenance, kind, source_eligibility(kind))
}

pub(crate) fn classify_docs_artifact(
    path: &str,
    provenance: ArtifactProvenance,
) -> ClassifiedArtifact {
    let path = normalize_path(path);
    let kind = if path.starts_with("site/") || path.starts_with("dist/") {
        ArtifactKind::BuildOutput
    } else if path == "README.md" || path.starts_with("docs/") && extension_is(&path, &["md"]) {
        ArtifactKind::Documentation
    } else {
        ArtifactKind::Unknown
    };
    ClassifiedArtifact::new(path, provenance, kind, docs_eligibility(kind))
}

pub(crate) fn classify_data_artifact(
    path: &str,
    provenance: ArtifactProvenance,
) -> ClassifiedArtifact {
    let path = normalize_path(path);
    let kind = if is_raw_input_path(&path) {
        ArtifactKind::RawInput
    } else if path.starts_with("data/processed/") || path.starts_with("reports/") {
        ArtifactKind::DerivedOutput
    } else {
        ArtifactKind::Unknown
    };
    ClassifiedArtifact::new(path, provenance, kind, data_eligibility(kind))
}

pub(crate) fn is_generated_declaration(path: &str) -> bool {
    let path = normalize_path(path);
    file_name(&path) == "next-env.d.ts" || path.ends_with(".d.ts")
}

pub(crate) fn is_dependency_cache_path(path: &str) -> bool {
    let path = normalize_path(path);
    path.starts_with("node_modules/")
        || path.starts_with(".venv/")
        || path.starts_with("venv/")
        || path.contains("/node_modules/")
        || path.contains("/.venv/")
        || path.contains("/venv/")
}

pub(crate) fn is_build_output_path(path: &str) -> bool {
    let path = normalize_path(path);
    path.starts_with(".next/")
        || path.starts_with("target/")
        || path.starts_with("dist/")
        || path.starts_with("build/")
}

pub(crate) fn artifact_kind_label(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::RouteEntry => "route_entry",
        ArtifactKind::RouteInfrastructure => "route_infrastructure",
        ArtifactKind::UiSource => "source/ui",
        ArtifactKind::StyleSource => "source/style",
        ArtifactKind::RuntimeSource => "source/runtime",
        ArtifactKind::TestSource => "test",
        ArtifactKind::Manifest => "setup/manifest",
        ArtifactKind::Config => "setup/config",
        ArtifactKind::GeneratedDeclaration => "generated/declaration",
        ArtifactKind::DependencyCache => "dependency_cache",
        ArtifactKind::BuildOutput => "build_output",
        ArtifactKind::RawInput => "raw_input",
        ArtifactKind::DerivedOutput => "derived_output",
        ArtifactKind::Documentation => "documentation",
        ArtifactKind::Unknown => "unknown",
    }
}

pub(crate) fn setup_step_may_own_artifact(kind: ArtifactKind) -> bool {
    matches!(
        kind,
        ArtifactKind::Manifest | ArtifactKind::Config | ArtifactKind::Unknown
    )
}

pub(crate) fn is_manifest_path(path: &str) -> bool {
    let name = file_name(path);
    name == "package.json"
        || name == "package-lock.json"
        || name == "pnpm-lock.yaml"
        || name == "yarn.lock"
        || name == "Cargo.toml"
        || name == "Cargo.lock"
        || name == "pyproject.toml"
        || name == "setup.py"
        || name.starts_with("requirements") && name.ends_with(".txt")
}

pub(crate) fn is_config_path(path: &str) -> bool {
    let name = file_name(path);
    name == "tsconfig.json"
        || name.starts_with("next.config.")
        || name.starts_with("tailwind.config.")
        || name.starts_with("postcss.config.")
        || matches!(name, "pytest.ini" | "tox.ini" | ".flake8" | "mypy.ini")
}

pub(crate) fn extension_is(path: &str, extensions: &[&str]) -> bool {
    let Some(extension) = file_name(path).rsplit_once('.').map(|(_, ext)| ext) else {
        return false;
    };
    extensions.contains(&extension)
}

fn unknown_artifact(path: &str, provenance: ArtifactProvenance) -> ClassifiedArtifact {
    ClassifiedArtifact::new(
        normalize_path(path),
        provenance,
        ArtifactKind::Unknown,
        ArtifactEligibility::default(),
    )
}

fn nextjs_eligibility(kind: ArtifactKind, provenance: ArtifactProvenance) -> ArtifactEligibility {
    let contract_artifact = matches!(
        provenance,
        ArtifactProvenance::UserRequiredArtifact
            | ArtifactProvenance::StepExpectedPath
            | ArtifactProvenance::PhaseRequiredArtifact
    );
    ArtifactEligibility {
        route_integration: kind == ArtifactKind::UiSource && contract_artifact,
        source_integration: kind == ArtifactKind::UiSource && contract_artifact,
        verification_target: matches!(
            kind,
            ArtifactKind::RouteEntry
                | ArtifactKind::RouteInfrastructure
                | ArtifactKind::UiSource
                | ArtifactKind::StyleSource
                | ArtifactKind::Manifest
                | ArtifactKind::Config
        ),
        recovery_target: matches!(
            kind,
            ArtifactKind::RouteEntry
                | ArtifactKind::RouteInfrastructure
                | ArtifactKind::UiSource
                | ArtifactKind::StyleSource
                | ArtifactKind::Manifest
                | ArtifactKind::Config
        ),
        protected_input: false,
    }
}

fn source_eligibility(kind: ArtifactKind) -> ArtifactEligibility {
    ArtifactEligibility {
        route_integration: false,
        source_integration: matches!(kind, ArtifactKind::RuntimeSource | ArtifactKind::TestSource),
        verification_target: matches!(
            kind,
            ArtifactKind::RuntimeSource
                | ArtifactKind::TestSource
                | ArtifactKind::Manifest
                | ArtifactKind::Config
        ),
        recovery_target: matches!(
            kind,
            ArtifactKind::RuntimeSource
                | ArtifactKind::TestSource
                | ArtifactKind::Manifest
                | ArtifactKind::Config
        ),
        protected_input: false,
    }
}

fn docs_eligibility(kind: ArtifactKind) -> ArtifactEligibility {
    ArtifactEligibility {
        route_integration: false,
        source_integration: false,
        verification_target: kind == ArtifactKind::Documentation,
        recovery_target: kind == ArtifactKind::Documentation,
        protected_input: false,
    }
}

fn data_eligibility(kind: ArtifactKind) -> ArtifactEligibility {
    ArtifactEligibility {
        route_integration: false,
        source_integration: false,
        verification_target: matches!(kind, ArtifactKind::RawInput | ArtifactKind::DerivedOutput),
        recovery_target: kind == ArtifactKind::DerivedOutput,
        protected_input: kind == ArtifactKind::RawInput,
    }
}

fn is_nextjs_route_entry(path: &str) -> bool {
    matches!(
        path,
        "app/page.tsx"
            | "app/page.jsx"
            | "src/app/page.tsx"
            | "src/app/page.jsx"
            | "pages/index.tsx"
            | "pages/index.jsx"
            | "src/pages/index.tsx"
            | "src/pages/index.jsx"
    )
}

fn is_nextjs_route_infrastructure(path: &str) -> bool {
    matches!(
        path,
        "app/layout.tsx" | "app/layout.jsx" | "src/app/layout.tsx" | "src/app/layout.jsx"
    )
}

fn is_nextjs_ui_source_path(path: &str) -> bool {
    extension_is(path, &["tsx", "jsx", "ts", "js"])
        && (path.starts_with("components/")
            || path.starts_with("src/components/")
            || path.starts_with("app/")
            || path.starts_with("src/app/")
            || path.starts_with("lib/")
            || path.starts_with("src/lib/")
            || path.starts_with("hooks/")
            || path.starts_with("src/hooks/"))
}

fn is_nextjs_style_source_path(path: &str) -> bool {
    extension_is(path, &["css"])
        && (path.starts_with("app/")
            || path.starts_with("src/app/")
            || path.starts_with("styles/")
            || path.starts_with("src/styles/"))
}

fn is_python_manifest_path(path: &str) -> bool {
    let name = file_name(path);
    name == "pyproject.toml"
        || name == "setup.py"
        || name.starts_with("requirements") && name.ends_with(".txt")
}

fn is_python_config_path(path: &str) -> bool {
    matches!(
        file_name(path),
        "pytest.ini" | "tox.ini" | ".flake8" | "mypy.ini"
    )
}

fn is_python_build_output_path(path: &str) -> bool {
    let path = normalize_path(path);
    path.ends_with(".pyc") || path.starts_with("__pycache__/") || path.contains("/__pycache__/")
}

fn is_raw_input_path(path: &str) -> bool {
    path.starts_with("raw/")
        || path.starts_with("data/raw/")
        || path.starts_with("input/")
        || path.starts_with("inputs/")
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_generated_declarations_before_source_extensions() {
        assert!(is_generated_declaration("next-env.d.ts"));
        assert!(is_generated_declaration("types/foo.d.ts"));
        assert_eq!(
            classify_nextjs_artifact("next-env.d.ts", ArtifactProvenance::StepExpectedPath).kind,
            ArtifactKind::GeneratedDeclaration
        );
    }

    #[test]
    fn nextjs_generated_declaration_is_not_route_integration_eligible() {
        let artifact =
            classify_nextjs_artifact("next-env.d.ts", ArtifactProvenance::StepExpectedPath);

        assert_eq!(artifact.kind, ArtifactKind::GeneratedDeclaration);
        assert!(!artifact.eligibility.route_integration);
    }

    #[test]
    fn nextjs_component_required_artifact_is_route_integration_eligible() {
        let artifact = classify_nextjs_artifact(
            "components/Game.tsx",
            ArtifactProvenance::PhaseRequiredArtifact,
        );

        assert_eq!(artifact.kind, ArtifactKind::UiSource);
        assert!(artifact.eligibility.route_integration);
    }

    #[test]
    fn nextjs_component_workspace_observation_is_not_route_integration_eligible() {
        let artifact = classify_nextjs_artifact(
            "components/Game.tsx",
            ArtifactProvenance::WorkspaceObservation,
        );

        assert_eq!(artifact.kind, ArtifactKind::UiSource);
        assert!(!artifact.eligibility.route_integration);
    }

    #[test]
    fn nextjs_global_css_is_style_source_not_route_integration_artifact() {
        for path in ["app/globals.css", "src/app/globals.css"] {
            let artifact = classify_nextjs_artifact(path, ArtifactProvenance::StepExpectedPath);

            assert_eq!(artifact.kind, ArtifactKind::StyleSource, "{path}");
            assert_eq!(artifact_kind_label(artifact.kind), "source/style");
            assert!(!artifact.eligibility.route_integration, "{path}");
            assert!(!setup_step_may_own_artifact(artifact.kind), "{path}");
        }
    }

    #[test]
    fn nextjs_setup_files_are_setup_owned() {
        for path in ["package.json", "tailwind.config.js", "postcss.config.js"] {
            let artifact = classify_nextjs_artifact(path, ArtifactProvenance::StepExpectedPath);

            assert!(
                setup_step_may_own_artifact(artifact.kind),
                "{path} classified as {:?}",
                artifact.kind
            );
        }
    }

    #[test]
    fn classifies_dependency_and_build_outputs() {
        assert_eq!(
            classify_nextjs_artifact(
                "node_modules/react/index.js",
                ArtifactProvenance::WorkspaceObservation
            )
            .kind,
            ArtifactKind::DependencyCache
        );
        assert_eq!(
            classify_nextjs_artifact(
                ".next/types/routes.d.ts",
                ArtifactProvenance::WorkspaceObservation
            )
            .kind,
            ArtifactKind::BuildOutput
        );
        assert_eq!(
            classify_rust_artifact("target/debug/app", ArtifactProvenance::VerifierEvidence).kind,
            ArtifactKind::BuildOutput
        );
    }

    #[test]
    fn classifies_python_cache_and_sources() {
        assert_eq!(
            classify_python_artifact(".venv/lib/python/site.py", ArtifactProvenance::ProfileFact)
                .kind,
            ArtifactKind::DependencyCache
        );
        assert_eq!(
            classify_python_artifact("__pycache__/main.pyc", ArtifactProvenance::ProfileFact).kind,
            ArtifactKind::BuildOutput
        );
        assert_eq!(
            classify_python_artifact("tests/test_app.py", ArtifactProvenance::StepExpectedPath)
                .kind,
            ArtifactKind::TestSource
        );
    }

    #[test]
    fn classifies_rust_sources_without_target_output() {
        assert_eq!(
            classify_rust_artifact("src/main.rs", ArtifactProvenance::StepExpectedPath).kind,
            ArtifactKind::RuntimeSource
        );
        assert_eq!(
            classify_rust_artifact("tests/cli.rs", ArtifactProvenance::StepExpectedPath).kind,
            ArtifactKind::TestSource
        );
        assert!(
            !classify_rust_artifact(
                "target/debug/build.rs",
                ArtifactProvenance::StepExpectedPath
            )
            .eligibility
            .recovery_target
        );
    }

    #[test]
    fn classifies_docs_and_data_paths() {
        assert_eq!(
            classify_docs_artifact("docs/profiles.md", ArtifactProvenance::StepExpectedPath).kind,
            ArtifactKind::Documentation
        );
        assert_eq!(
            classify_docs_artifact("dist/index.html", ArtifactProvenance::WorkspaceObservation)
                .kind,
            ArtifactKind::BuildOutput
        );
        let raw = classify_data_artifact(
            "data/raw/source.csv",
            ArtifactProvenance::UserRequiredArtifact,
        );
        assert_eq!(raw.kind, ArtifactKind::RawInput);
        assert!(raw.eligibility.protected_input);
        assert!(!raw.eligibility.recovery_target);
    }

    #[test]
    fn dispatches_by_profile() {
        assert_eq!(
            classify_profile_artifact(
                ProfileId::NextJs,
                "components/Game.tsx",
                ArtifactProvenance::StepExpectedPath
            )
            .kind,
            ArtifactKind::UiSource
        );
        assert_eq!(
            classify_profile_artifact(
                ProfileId::Generic,
                "components/Game.tsx",
                ArtifactProvenance::StepExpectedPath
            )
            .kind,
            ArtifactKind::Unknown
        );
    }
}
