use crate::agent::step_runner::profile_artifact::{ArtifactProvenance, classify_profile_artifact};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const NEXTJS_ROUTE_GRAPH_MAX_DEPTH: usize = 3;
const NEXTJS_ROUTE_GRAPH_MAX_FILES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileId {
    Generic,
    NextJs,
    Python,
    Rust,
    Investigation,
    Docs,
    DataAnalysis,
    DataPipeline,
}

impl ProfileId {
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "generic" => Ok(Self::Generic),
            "nextjs" | "next.js" => Ok(Self::NextJs),
            "python" => Ok(Self::Python),
            "rust" => Ok(Self::Rust),
            "investigation" => Ok(Self::Investigation),
            "docs" | "documentation" => Ok(Self::Docs),
            "data-analysis" | "data_analysis" => Ok(Self::DataAnalysis),
            "data-pipeline" | "data_pipeline" => Ok(Self::DataPipeline),
            other => Err(ProfileError::UnknownProfile(other.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::NextJs => "nextjs",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::Investigation => "investigation",
            Self::Docs => "docs",
            Self::DataAnalysis => "data-analysis",
            Self::DataPipeline => "data-pipeline",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileContract {
    pub id: ProfileId,
    pub text: String,
    pub verifier_commands: Vec<String>,
    pub protected_path_prefixes: Vec<String>,
}

pub fn profile_contract(id: ProfileId) -> ProfileContract {
    match id {
        ProfileId::Generic => ProfileContract {
            id,
            text: "Keep changes scoped. Prefer Read/Bash inspection before editing. Use Write/Edit for file changes. End with deterministic checks when practical.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::NextJs => ProfileContract {
            id,
            text: "For Next.js work, preserve honest build scripts. New apps need package.json with next/react/react-dom dependencies, app/page.tsx or pages/index.tsx, and a build script that remains `next build`. If node_modules/.bin/next is missing, install dependencies when allowed or report dependency_missing; never fake build success.".to_string(),
            verifier_commands: vec!["npm run build".to_string()],
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Python => ProfileContract {
            id,
            text: "For Python work, keep modules importable, prefer small functions, and verify with pytest or direct local script execution when tests are not present.".to_string(),
            verifier_commands: vec!["python -m pytest".to_string()],
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Rust => ProfileContract {
            id,
            text: "For Rust work, keep Cargo.toml honest, use idiomatic modules, and verify with cargo test, cargo build, or cargo run when requested. For new minimal projects, create Cargo.toml and src/main.rs with Write/Edit instead of cargo init or cargo new. If integration tests use CARGO_BIN_EXE_<name>, ensure <name> matches the Cargo binary name defined by Cargo.toml; tests must reference actual package, binary, module, and public item names defined in the project.".to_string(),
            verifier_commands: vec!["cargo test".to_string()],
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Investigation => ProfileContract {
            id,
            text: "For investigation work, inspect first, preserve evidence, and avoid code changes unless the requested task explicitly asks for a fix.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Docs => ProfileContract {
            id,
            text: "For documentation work, keep claims tied to repository facts, update indexes when present, and avoid changing behavior code.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::DataAnalysis => ProfileContract {
            id,
            text: "For data analysis work, keep raw inputs immutable, write derived artifacts separately, and record assumptions and reproducible commands.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: data_protected_prefixes(),
        },
        ProfileId::DataPipeline => ProfileContract {
            id,
            text: "For data pipeline work, keep raw inputs immutable, separate extraction, transformation, and output steps, and make reruns deterministic.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: data_protected_prefixes(),
        },
    }
}

pub fn profile_contract_text(profile: &str) -> Result<String, ProfileError> {
    Ok(profile_contract(ProfileId::parse(profile)?).text)
}

pub fn profile_verifier_commands(profile: &str) -> Result<Vec<String>, ProfileError> {
    Ok(profile_contract(ProfileId::parse(profile)?).verifier_commands)
}

pub fn protected_by_profile(profile: &str, path: &str) -> Result<bool, ProfileError> {
    let contract = profile_contract(ProfileId::parse(profile)?);
    Ok(contract
        .protected_path_prefixes
        .iter()
        .any(|prefix| path == prefix || path.starts_with(&format!("{prefix}/"))))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileFactSummary {
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileVerificationContext {
    pub goal_excerpt: String,
    pub required_artifacts: Vec<String>,
    pub expected_paths: Vec<String>,
    pub phase_contract_facts: Vec<String>,
    pub profile_facts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileVerificationFailure {
    pub code: String,
    pub message: String,
    pub paths: Vec<String>,
}

impl ProfileVerificationFailure {
    fn new(code: &str, message: impl Into<String>, paths: Vec<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            paths,
        }
    }

    pub fn render(&self) -> String {
        if self.paths.is_empty() {
            format!("{}: {}", self.code, self.message)
        } else {
            format!(
                "{}: {} ({})",
                self.code,
                self.message,
                self.paths.join(", ")
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileObligationContext {
    pub goal_excerpt: String,
    pub required_artifacts: Vec<String>,
    pub phase_contract_facts: Vec<String>,
    pub profile_facts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileObligation {
    pub code: String,
    pub message: String,
    pub paths: Vec<String>,
    pub expected: Option<String>,
}

impl ProfileObligation {
    fn new(
        code: &str,
        message: impl Into<String>,
        paths: Vec<String>,
        expected: Option<String>,
    ) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            paths,
            expected,
        }
    }

    pub fn render(&self) -> String {
        let mut out = if self.paths.is_empty() {
            format!("{}: {}", self.code, self.message)
        } else {
            format!(
                "{}: {} ({})",
                self.code,
                self.message,
                self.paths.join(", ")
            )
        };
        if let Some(expected) = &self.expected {
            out.push_str(&format!(" expected={expected}"));
        }
        out
    }
}

pub fn profile_fact_summary(profile: &str, cwd: &Path) -> Result<ProfileFactSummary, ProfileError> {
    match ProfileId::parse(profile)? {
        ProfileId::NextJs => Ok(nextjs_fact_summary(cwd)),
        _ => Ok(ProfileFactSummary { lines: Vec::new() }),
    }
}

pub fn verify_profile(
    profile: &str,
    cwd: &Path,
    context: &ProfileVerificationContext,
) -> Result<Vec<ProfileVerificationFailure>, ProfileError> {
    match ProfileId::parse(profile)? {
        ProfileId::NextJs => Ok(verify_nextjs_profile(cwd, context)),
        _ => Ok(Vec::new()),
    }
}

pub fn profile_obligations(
    profile: &str,
    context: &ProfileObligationContext,
) -> Result<Vec<ProfileObligation>, ProfileError> {
    match ProfileId::parse(profile)? {
        ProfileId::NextJs => Ok(nextjs_profile_obligations(context)),
        _ => Ok(Vec::new()),
    }
}

pub fn render_profile_obligations(obligations: &[ProfileObligation]) -> Vec<String> {
    obligations
        .iter()
        .map(|obligation| {
            let paths = if obligation.paths.is_empty() {
                "none".to_string()
            } else {
                obligation
                    .paths
                    .iter()
                    .map(|path| bounded_value(path))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let expected = obligation
                .expected
                .as_deref()
                .map(bounded_value)
                .unwrap_or_else(|| "none".to_string());
            format!(
                "profile.obligation.{}={}; paths={}; expected={}",
                obligation.code,
                bounded_value(&obligation.message),
                paths,
                expected
            )
        })
        .collect()
}

fn data_protected_prefixes() -> Vec<String> {
    vec![
        "raw".to_string(),
        "data/raw".to_string(),
        "input".to_string(),
        "inputs".to_string(),
    ]
}

#[derive(Debug, Default)]
struct NextJsFacts {
    package_json: bool,
    scripts_dev: Option<String>,
    scripts_build: Option<String>,
    dependencies: BTreeSet<String>,
    dependency_versions: BTreeMap<String, String>,
    routes: Vec<String>,
    layouts: Vec<String>,
    config_files: Vec<String>,
    tailwind_css_files: Vec<String>,
    tsconfig_root_dir: Option<String>,
    tsconfig_has_alias: bool,
}

fn nextjs_fact_summary(cwd: &Path) -> ProfileFactSummary {
    let facts = collect_nextjs_facts(cwd);
    let mut lines = Vec::new();
    lines.push(format!(
        "nextjs.package_json={}",
        if facts.package_json {
            "present"
        } else {
            "missing"
        }
    ));
    push_optional_fact(
        &mut lines,
        "nextjs.scripts.dev",
        facts.scripts_dev.as_deref(),
    );
    push_optional_fact(
        &mut lines,
        "nextjs.scripts.build",
        facts.scripts_build.as_deref(),
    );
    lines.push(format!(
        "nextjs.dependencies={}",
        if facts.dependencies.is_empty() {
            "none".to_string()
        } else {
            facts
                .dependencies
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        }
    ));
    lines.push(format!(
        "nextjs.routes={}",
        bounded_join(&facts.routes, "none")
    ));
    lines.push(format!(
        "nextjs.layouts={}",
        bounded_join(&facts.layouts, "none")
    ));
    lines.push(format!(
        "nextjs.app_root={}",
        selected_nextjs_root(&facts).unwrap_or_else(|| "unknown".to_string())
    ));
    lines.push(format!(
        "nextjs.configs={}",
        bounded_join(&facts.config_files, "none")
    ));
    lines.push(format!(
        "nextjs.tailwind_css={}",
        bounded_join(&facts.tailwind_css_files, "none")
    ));
    push_optional_fact(
        &mut lines,
        "nextjs.tsconfig.rootDir",
        facts.tsconfig_root_dir.as_deref(),
    );
    lines.push(format!(
        "nextjs.tsconfig.alias=@/*:{}",
        if facts.tsconfig_has_alias {
            "present"
        } else {
            "missing"
        }
    ));
    ProfileFactSummary {
        lines: lines.into_iter().take(16).collect(),
    }
}

fn verify_nextjs_profile(
    cwd: &Path,
    context: &ProfileVerificationContext,
) -> Vec<ProfileVerificationFailure> {
    let facts = collect_nextjs_facts(cwd);
    let mut failures = Vec::new();

    let has_app = !facts.routes.is_empty() || !facts.layouts.is_empty();
    let root = selected_nextjs_root(&facts);
    if root.as_deref() == Some("mixed") {
        failures.push(ProfileVerificationFailure::new(
            "nextjs_app_root_ambiguous",
            "both root app and src/app routes are present without an explicit migration boundary",
            facts.routes.clone(),
        ));
    }

    if let Some(build) = &facts.scripts_build {
        if build.trim() != "next build" {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_build_script_drift",
                format!("scripts.build must remain `next build`, got `{build}`"),
                vec!["package.json".to_string()],
            ));
        }
    } else if facts.package_json && has_app {
        failures.push(ProfileVerificationFailure::new(
            "nextjs_build_script_drift",
            "package.json is missing scripts.build for a Next.js app",
            vec!["package.json".to_string()],
        ));
    }

    if context_requires_literal(context, "3011") {
        match &facts.scripts_dev {
            Some(dev) if dev.contains("next dev") && dev.contains("3011") => {}
            Some(dev) => failures.push(ProfileVerificationFailure::new(
                "nextjs_dev_port_drift",
                format!("scripts.dev must preserve requested port 3011, got `{dev}`"),
                vec!["package.json".to_string()],
            )),
            None if facts.package_json => failures.push(ProfileVerificationFailure::new(
                "nextjs_dev_port_drift",
                "package.json is missing scripts.dev for requested port 3011",
                vec!["package.json".to_string()],
            )),
            None => {}
        }
    }

    if facts.package_json && has_app {
        for dep in ["next", "react", "react-dom"] {
            if !facts.dependencies.contains(dep) {
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_missing_dependency",
                    format!("package.json is missing `{dep}`"),
                    vec!["package.json".to_string()],
                ));
            }
        }
        if let Some(message) = nextjs_dependency_version_conflict(&facts) {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_dependency_version_conflict",
                message,
                vec!["package.json".to_string()],
            ));
        }
        if let Some(message) = nextjs_tailwind_dependency_version_conflict(&facts) {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_dependency_version_conflict",
                message,
                vec!["package.json".to_string()],
            ));
        }
        if let Some(message) = nextjs_typescript_toolchain_version_conflict(&facts) {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_dependency_version_conflict",
                message,
                vec!["package.json".to_string()],
            ));
        }
    }

    if !facts.tailwind_css_files.is_empty() {
        for dep in ["tailwindcss", "postcss", "autoprefixer"] {
            if !facts.dependencies.contains(dep) {
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_tailwind_contract",
                    format!("Tailwind directives require `{dep}` in package.json"),
                    vec!["package.json".to_string()],
                ));
            }
        }
        if !facts
            .config_files
            .iter()
            .any(|path| path.starts_with("tailwind.config."))
        {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_tailwind_contract",
                "Tailwind directives require tailwind.config.*",
                facts.tailwind_css_files.clone(),
            ));
        }
        if !facts
            .config_files
            .iter()
            .any(|path| path.starts_with("postcss.config."))
        {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_tailwind_contract",
                "Tailwind directives require postcss.config.*",
                facts.tailwind_css_files.clone(),
            ));
        }
        for path in facts
            .config_files
            .iter()
            .filter(|path| path.starts_with("postcss.config."))
        {
            let text = fs::read_to_string(cwd.join(path)).unwrap_or_default();
            if text.contains("@tailwindcss/postcss")
                && !facts.dependencies.contains("@tailwindcss/postcss")
            {
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_tailwind_contract",
                    format!(
                        "{path} uses @tailwindcss/postcss, so package.json must include `@tailwindcss/postcss`"
                    ),
                    vec!["package.json".to_string(), path.clone()],
                ));
            }
            if facts.dependencies.contains("@tailwindcss/postcss")
                && postcss_config_uses_tailwindcss_directly(&text)
            {
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_tailwind_contract",
                    format!(
                        "{path} must use @tailwindcss/postcss instead of tailwindcss directly as a PostCSS plugin"
                    ),
                    vec![path.clone()],
                ));
            }
        }
    }

    if root.as_deref() == Some("app")
        && facts
            .tsconfig_root_dir
            .as_deref()
            .is_some_and(|root_dir| root_dir == "src" || root_dir == "./src")
    {
        failures.push(ProfileVerificationFailure::new(
            "nextjs_tsconfig_excludes_route",
            "tsconfig compilerOptions.rootDir points at src while the selected route root is app",
            vec!["tsconfig.json".to_string(), "app/page.tsx".to_string()],
        ));
    }

    let selected_route = selected_route_file(&facts);
    if let Some(route) = selected_route.as_deref() {
        let route_text = fs::read_to_string(cwd.join(route)).unwrap_or_default();
        let route_graph = nextjs_route_graph(cwd, route);
        if (route_text.contains("\"@/") || route_text.contains("'@/")) && !facts.tsconfig_has_alias
        {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_alias_missing",
                "route uses @/* imports but tsconfig compilerOptions.paths does not define @/*",
                vec![route.to_string(), "tsconfig.json".to_string()],
            ));
        }

        for explicit_path in explicit_integration_paths(context) {
            if explicit_path == route {
                continue;
            }
            if !cwd.join(&explicit_path).exists() {
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_integration_artifact_missing",
                    format!(
                        "explicit artifact `{explicit_path}` does not exist before route integration with selected route `{route}`"
                    ),
                    vec![explicit_path, route.to_string()],
                ));
                continue;
            }
            if !route_graph_integrates_artifact(cwd, &route_graph, &explicit_path) {
                let repair_target = route_graph_repair_target(&route_graph, &explicit_path)
                    .unwrap_or_else(|| route.to_string());
                let mut paths = vec![route.to_string(), explicit_path.clone()];
                if repair_target != route {
                    paths.push(repair_target.clone());
                }
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_route_not_integrated",
                    format!(
                        "explicit artifact `{explicit_path}` is not referenced from selected route graph rooted at `{route}`; repair target `{repair_target}`"
                    ),
                    paths,
                ));
            }
        }
    }

    failures
}

fn nextjs_profile_obligations(context: &ProfileObligationContext) -> Vec<ProfileObligation> {
    let mut obligations = Vec::new();
    if nextjs_app_requested(context) {
        obligations.push(ProfileObligation::new(
            "nextjs_build_script_required",
            "package.json scripts.build must remain an honest Next.js build",
            vec!["package.json".to_string()],
            Some("scripts.build == next build".to_string()),
        ));
        obligations.push(ProfileObligation::new(
            "nextjs_dependencies_required",
            "package.json dependencies must include compatible runtime packages for a Next.js app",
            vec!["package.json".to_string()],
            Some("dependencies include next, react, react-dom with React 18.2 or newer compatibility".to_string()),
        ));
    }
    if obligation_context_requires_literal(context, "3011") {
        obligations.push(ProfileObligation::new(
            "nextjs_dev_port_required",
            "package.json scripts.dev must preserve the requested development port",
            vec!["package.json".to_string()],
            Some("scripts.dev contains next dev and 3011".to_string()),
        ));
    }
    if nextjs_tailwind_requested(context) {
        obligations.push(ProfileObligation::new(
            "nextjs_tailwind_dependencies_required",
            "package.json dependencies must include Tailwind runtime tooling when Tailwind directives or config are requested",
            vec!["package.json".to_string()],
            Some("dependencies include compatible tailwindcss, postcss, autoprefixer versions".to_string()),
        ));
    }
    if let Some((route, artifact)) = nextjs_route_integration_obligation_target(context) {
        obligations.push(ProfileObligation::new(
            "nextjs_route_integration_required",
            "selected Next.js route must import or reference explicit UI/game source artifacts",
            vec![route.clone(), artifact.clone()],
            Some(format!(
                "selected route `{route}` references `{artifact}` or its module name"
            )),
        ));
    }
    obligations
}

fn nextjs_app_requested(context: &ProfileObligationContext) -> bool {
    let goal = context.goal_excerpt.to_ascii_lowercase();
    if goal.contains("next.js") || goal.contains("nextjs") || goal.contains("next js") {
        return true;
    }
    context
        .required_artifacts
        .iter()
        .chain(context.phase_contract_facts.iter())
        .chain(context.profile_facts.iter())
        .any(|value| {
            value.contains("app/page.")
                || value.contains("src/app/page.")
                || value.contains("pages/index.")
                || (value.contains("nextjs.routes=") && !value.contains("nextjs.routes=none"))
                || (value.contains("nextjs.app_root=")
                    && !value.contains("nextjs.app_root=unknown"))
        })
}

fn nextjs_tailwind_requested(context: &ProfileObligationContext) -> bool {
    let goal = context.goal_excerpt.to_ascii_lowercase();
    if goal.contains("tailwind") {
        return true;
    }
    context
        .required_artifacts
        .iter()
        .chain(context.phase_contract_facts.iter())
        .chain(context.profile_facts.iter())
        .any(|value| {
            let lower = value.to_ascii_lowercase();
            lower.contains("tailwind.config")
                || lower.contains("postcss.config")
                || (lower.contains("nextjs.tailwind_css=")
                    && !lower.contains("nextjs.tailwind_css=none"))
                || lower.contains("@tailwind")
        })
}

fn obligation_context_requires_literal(context: &ProfileObligationContext, literal: &str) -> bool {
    context.goal_excerpt.contains(literal)
        || context
            .required_artifacts
            .iter()
            .chain(context.phase_contract_facts.iter())
            .chain(context.profile_facts.iter())
            .any(|value| value.contains(literal))
}

fn nextjs_route_integration_obligation_target(
    context: &ProfileObligationContext,
) -> Option<(String, String)> {
    let route = selected_nextjs_route_from_obligation_context(context)?;
    let artifact = explicit_obligation_integration_paths(context)
        .into_iter()
        .find(|path| path != &route)?;
    Some((route, artifact))
}

fn selected_nextjs_route_from_obligation_context(
    context: &ProfileObligationContext,
) -> Option<String> {
    let mut root = None;
    let mut routes = Vec::new();
    for value in context
        .required_artifacts
        .iter()
        .chain(context.phase_contract_facts.iter())
        .chain(context.profile_facts.iter())
    {
        if let Some(value) = value.strip_prefix("nextjs.app_root=")
            && !matches!(value, "unknown" | "mixed")
        {
            root = Some(value.to_string());
        }
        if let Some(value) = value.strip_prefix("nextjs.routes=") {
            for route in value.split(',') {
                if route != "none" && is_nextjs_route_infra_path(route) {
                    routes.push(route.to_string());
                }
            }
        }
        for token in value.split([',', ' ', '=']) {
            if is_nextjs_route_infra_path(token) && !routes.iter().any(|route| route == token) {
                routes.push(token.to_string());
            }
        }
    }
    if let Some(root) = root {
        routes
            .into_iter()
            .find(|route| nextjs_route_matches_root(route, &root))
    } else {
        routes.into_iter().next()
    }
}

fn explicit_obligation_integration_paths(context: &ProfileObligationContext) -> Vec<String> {
    let mut paths = context
        .required_artifacts
        .iter()
        .filter(|path| {
            nextjs_route_integration_candidate_with_provenance(
                path,
                ArtifactProvenance::PhaseRequiredArtifact,
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn collect_nextjs_facts(cwd: &Path) -> NextJsFacts {
    let mut facts = NextJsFacts::default();
    let package_path = cwd.join("package.json");
    facts.package_json = package_path.exists();
    if let Ok(text) = fs::read_to_string(&package_path)
        && let Ok(json) = serde_json::from_str::<Value>(&text)
    {
        facts.scripts_dev = json_string_at(&json, &["scripts", "dev"]);
        facts.scripts_build = json_string_at(&json, &["scripts", "build"]);
        collect_dependency_keys(
            &mut facts.dependencies,
            &mut facts.dependency_versions,
            &json,
            "dependencies",
        );
        collect_dependency_keys(
            &mut facts.dependencies,
            &mut facts.dependency_versions,
            &json,
            "devDependencies",
        );
    }

    for path in [
        "app/page.tsx",
        "app/page.jsx",
        "src/app/page.tsx",
        "src/app/page.jsx",
        "pages/index.tsx",
        "pages/index.jsx",
        "src/pages/index.tsx",
        "src/pages/index.jsx",
    ] {
        if cwd.join(path).exists() {
            facts.routes.push(path.to_string());
        }
    }
    for path in [
        "app/layout.tsx",
        "app/layout.jsx",
        "src/app/layout.tsx",
        "src/app/layout.jsx",
    ] {
        if cwd.join(path).exists() {
            facts.layouts.push(path.to_string());
        }
    }
    for path in [
        "next.config.js",
        "next.config.mjs",
        "next.config.ts",
        "tailwind.config.js",
        "tailwind.config.cjs",
        "tailwind.config.mjs",
        "tailwind.config.ts",
        "postcss.config.js",
        "postcss.config.cjs",
        "postcss.config.mjs",
        "tsconfig.json",
    ] {
        if cwd.join(path).exists() {
            facts.config_files.push(path.to_string());
        }
    }
    for path in [
        "app/globals.css",
        "src/app/globals.css",
        "styles/globals.css",
        "src/styles/globals.css",
    ] {
        if fs::read_to_string(cwd.join(path))
            .map(|text| text.contains("@tailwind "))
            .unwrap_or(false)
        {
            facts.tailwind_css_files.push(path.to_string());
        }
    }
    if let Ok(text) = fs::read_to_string(cwd.join("tsconfig.json"))
        && let Ok(json) = serde_json::from_str::<Value>(&text)
    {
        facts.tsconfig_root_dir = json_string_at(&json, &["compilerOptions", "rootDir"]);
        facts.tsconfig_has_alias = json
            .pointer("/compilerOptions/paths/@~1*")
            .or_else(|| json.pointer("/compilerOptions/paths/@/*"))
            .is_some();
    }

    facts
}

fn json_string_at(json: &Value, path: &[&str]) -> Option<String> {
    let mut current = json;
    for part in path {
        current = current.get(*part)?;
    }
    current.as_str().map(ToString::to_string)
}

fn collect_dependency_keys(
    out: &mut BTreeSet<String>,
    versions: &mut BTreeMap<String, String>,
    json: &Value,
    key: &str,
) {
    let Some(object) = json.get(key).and_then(Value::as_object) else {
        return;
    };
    for (name, value) in object {
        out.insert(name.clone());
        if let Some(version) = value.as_str() {
            versions.insert(name.clone(), version.to_string());
        }
    }
}

fn nextjs_dependency_version_conflict(facts: &NextJsFacts) -> Option<String> {
    let next = facts.dependency_versions.get("next")?;
    if !version_major_at_least(next, 14) {
        return None;
    }
    let mut conflicts = Vec::new();
    for dep in ["react", "react-dom"] {
        if facts
            .dependency_versions
            .get(dep)
            .and_then(|version| exact_version_tuple(version))
            .is_some_and(|version| version < (18, 2, 0))
        {
            conflicts.push(format!(
                "{}@{}",
                dep,
                facts.dependency_versions.get(dep).unwrap()
            ));
        }
    }
    if conflicts.is_empty() {
        None
    } else {
        Some(format!(
            "Next.js 14 requires React peer versions compatible with 18.2 or newer; incompatible exact pins: {}",
            conflicts.join(", ")
        ))
    }
}

fn nextjs_tailwind_dependency_version_conflict(facts: &NextJsFacts) -> Option<String> {
    let autoprefixer = facts.dependency_versions.get("autoprefixer")?;
    let postcss = facts.dependency_versions.get("postcss")?;
    let autoprefixer_exact = exact_version_tuple(autoprefixer)?;
    let postcss_exact = exact_version_tuple(postcss)?;
    if autoprefixer_exact.0 >= 10 && postcss_exact < (8, 0, 2) {
        Some(format!(
            "autoprefixer@{autoprefixer} requires postcss peer range ^8.0.2 or newer; incompatible exact pin: postcss@{postcss}"
        ))
    } else {
        None
    }
}

fn nextjs_typescript_toolchain_version_conflict(facts: &NextJsFacts) -> Option<String> {
    let next = facts.dependency_versions.get("next")?;
    if !version_major_at_least(next, 14) {
        return None;
    }

    let mut conflicts = Vec::new();
    if facts
        .dependency_versions
        .get("typescript")
        .and_then(|version| version_major(version))
        .is_some_and(|major| major >= 6)
    {
        conflicts.push(format!(
            "typescript@{}",
            facts.dependency_versions.get("typescript").unwrap()
        ));
    }

    let react_major = facts
        .dependency_versions
        .get("react")
        .and_then(|version| version_major(version));
    let react_types_major = facts
        .dependency_versions
        .get("@types/react")
        .and_then(|version| version_major(version));
    if react_major == Some(18) && react_types_major.is_some_and(|major| major >= 19) {
        conflicts.push(format!(
            "@types/react@{}",
            facts.dependency_versions.get("@types/react").unwrap()
        ));
    }

    if conflicts.is_empty() {
        None
    } else {
        Some(format!(
            "Next.js 14 generated TypeScript apps should use a stable TypeScript 5.x and React 18 type family; incompatible generated toolchain pins: {}",
            conflicts.join(", ")
        ))
    }
}

fn postcss_config_uses_tailwindcss_directly(text: &str) -> bool {
    text.contains("tailwindcss") && !text.contains("@tailwindcss/postcss")
}

fn version_major(value: &str) -> Option<u64> {
    value
        .trim_start_matches(['^', '~', 'v'])
        .split('.')
        .next()
        .and_then(|part| part.parse::<u64>().ok())
}

fn version_major_at_least(value: &str, minimum: u64) -> bool {
    version_major(value).is_some_and(|major| major >= minimum)
}

fn exact_version_tuple(value: &str) -> Option<(u64, u64, u64)> {
    let first = value.chars().next()?;
    if !first.is_ascii_digit() {
        return None;
    }
    let mut parts = value.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch_part = parts.next().unwrap_or("0");
    let patch = patch_part
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse::<u64>()
        .ok()?;
    Some((major, minor, patch))
}

fn selected_nextjs_root(facts: &NextJsFacts) -> Option<String> {
    let has_app = facts
        .routes
        .iter()
        .chain(facts.layouts.iter())
        .any(|path| path.starts_with("app/"));
    let has_src_app = facts
        .routes
        .iter()
        .chain(facts.layouts.iter())
        .any(|path| path.starts_with("src/app/"));
    let has_pages = facts.routes.iter().any(|path| path.contains("pages/"));
    match (has_app, has_src_app, has_pages) {
        (true, false, false) => Some("app".to_string()),
        (false, true, false) => Some("src/app".to_string()),
        (false, false, true) => Some("pages".to_string()),
        (false, false, false) => None,
        _ => Some("mixed".to_string()),
    }
}

fn selected_route_file(facts: &NextJsFacts) -> Option<String> {
    let root = selected_nextjs_root(facts)?;
    if root == "mixed" {
        return None;
    }
    facts
        .routes
        .iter()
        .find(|path| match root.as_str() {
            "app" => path.starts_with("app/"),
            "src/app" => path.starts_with("src/app/"),
            "pages" => path.contains("pages/"),
            _ => false,
        })
        .cloned()
}

pub(crate) fn nextjs_selected_route_from_workspace(cwd: &Path) -> Option<String> {
    selected_route_file(&collect_nextjs_facts(cwd))
}

fn nextjs_route_matches_root(route: &str, root: &str) -> bool {
    match root {
        "app" => route.starts_with("app/"),
        "src/app" => route.starts_with("src/app/"),
        "pages" => route.contains("pages/"),
        _ => false,
    }
}

fn context_requires_literal(context: &ProfileVerificationContext, literal: &str) -> bool {
    context.goal_excerpt.contains(literal)
        || context
            .required_artifacts
            .iter()
            .chain(context.expected_paths.iter())
            .chain(context.phase_contract_facts.iter())
            .chain(context.profile_facts.iter())
            .any(|value| value.contains(literal))
}

fn explicit_integration_paths(context: &ProfileVerificationContext) -> Vec<String> {
    let mut paths = context
        .required_artifacts
        .iter()
        .filter(|path| {
            nextjs_route_integration_candidate_with_provenance(
                path,
                ArtifactProvenance::PhaseRequiredArtifact,
            )
        })
        .chain(context.expected_paths.iter().filter(|path| {
            nextjs_route_integration_candidate_with_provenance(
                path,
                ArtifactProvenance::StepExpectedPath,
            )
        }))
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn is_nextjs_route_infra_path(path: &str) -> bool {
    matches!(
        path,
        "app/page.tsx"
            | "app/page.jsx"
            | "app/layout.tsx"
            | "app/layout.jsx"
            | "src/app/page.tsx"
            | "src/app/page.jsx"
            | "src/app/layout.tsx"
            | "src/app/layout.jsx"
            | "pages/index.tsx"
            | "pages/index.jsx"
            | "src/pages/index.tsx"
            | "src/pages/index.jsx"
    )
}

pub(crate) fn nextjs_route_integration_candidate(path: &str) -> bool {
    nextjs_route_integration_candidate_with_provenance(path, ArtifactProvenance::StepExpectedPath)
}

fn nextjs_route_integration_candidate_with_provenance(
    path: &str,
    provenance: ArtifactProvenance,
) -> bool {
    classify_profile_artifact(ProfileId::NextJs, path, provenance)
        .eligibility
        .route_integration
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NextJsRouteGraph {
    selected_route: String,
    files: Vec<String>,
}

fn nextjs_route_graph(cwd: &Path, selected_route: &str) -> NextJsRouteGraph {
    let mut files = Vec::new();
    let mut seen = BTreeSet::new();
    let mut queue = vec![(selected_route.to_string(), 0usize)];
    while let Some((path, depth)) = queue.pop() {
        if files.len() >= NEXTJS_ROUTE_GRAPH_MAX_FILES || depth > NEXTJS_ROUTE_GRAPH_MAX_DEPTH {
            continue;
        }
        if !seen.insert(path.clone()) || ignored_route_graph_path(&path) {
            continue;
        }
        let absolute = cwd.join(&path);
        let Ok(text) = fs::read_to_string(&absolute) else {
            continue;
        };
        files.push(path.clone());
        if depth == NEXTJS_ROUTE_GRAPH_MAX_DEPTH {
            continue;
        }
        for import_path in static_relative_imports(&text) {
            if let Some(resolved) = resolve_route_import(cwd, &path, &import_path)
                && !seen.contains(&resolved)
            {
                queue.push((resolved, depth + 1));
            }
        }
    }
    NextJsRouteGraph {
        selected_route: selected_route.to_string(),
        files,
    }
}

fn route_graph_integrates_artifact(cwd: &Path, graph: &NextJsRouteGraph, artifact: &str) -> bool {
    if graph.files.iter().any(|file| file == artifact) {
        return true;
    }
    let stem = Path::new(artifact)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    if stem.is_empty() {
        return false;
    }
    let path_without_ext = artifact
        .strip_suffix(".tsx")
        .or_else(|| artifact.strip_suffix(".jsx"))
        .or_else(|| artifact.strip_suffix(".ts"))
        .or_else(|| artifact.strip_suffix(".js"))
        .unwrap_or(artifact);
    graph.files.iter().any(|file| {
        fs::read_to_string(cwd.join(file))
            .map(|text| text.contains(stem) || text.contains(path_without_ext))
            .unwrap_or(false)
    })
}

fn route_graph_repair_target(graph: &NextJsRouteGraph, artifact: &str) -> Option<String> {
    let artifact_dir = Path::new(artifact)
        .parent()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let component_target = graph
        .files
        .iter()
        .filter(|file| file.as_str() != graph.selected_route)
        .find(|file| {
            let lower = file.to_ascii_lowercase();
            lower.contains("/components/") || lower.starts_with("components/")
        })
        .cloned();
    if !artifact_dir.is_empty()
        && artifact_dir.contains("/hooks")
        && let Some(target) = component_target
    {
        return Some(target);
    }
    graph.files.first().cloned()
}

fn static_relative_imports(text: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if !line.starts_with("import ") {
            continue;
        }
        for quote in ['\'', '"'] {
            for value in quoted_values(line, quote) {
                if value.starts_with('.') && !imports.iter().any(|existing| existing == &value) {
                    imports.push(value);
                }
            }
        }
    }
    imports
}

fn quoted_values(line: &str, quote: char) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find(quote) {
        let after_start = &rest[start + quote.len_utf8()..];
        let Some(end) = after_start.find(quote) else {
            break;
        };
        values.push(after_start[..end].to_string());
        rest = &after_start[end + quote.len_utf8()..];
    }
    values
}

fn resolve_route_import(cwd: &Path, from_file: &str, import_path: &str) -> Option<String> {
    if !import_path.starts_with('.') {
        return None;
    }
    let base_dir = Path::new(from_file)
        .parent()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let base = normalize_route_import_path(&base_dir, import_path)?;
    for candidate in route_import_candidates(&base) {
        if ignored_route_graph_path(&candidate) {
            continue;
        }
        if cwd.join(&candidate).is_file() {
            return Some(candidate);
        }
    }
    None
}

fn normalize_route_import_path(base_dir: &str, import_path: &str) -> Option<String> {
    let combined = if base_dir.is_empty() {
        import_path.to_string()
    } else {
        format!("{base_dir}/{import_path}")
    };
    let mut parts = Vec::new();
    for part in combined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            value => parts.push(value),
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn route_import_candidates(base: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if Path::new(base).extension().is_some() {
        candidates.push(base.to_string());
    } else {
        candidates.push(base.to_string());
        for ext in ["tsx", "ts", "jsx", "js"] {
            candidates.push(format!("{base}.{ext}"));
        }
        for ext in ["tsx", "ts", "jsx", "js"] {
            candidates.push(format!("{base}/index.{ext}"));
        }
    }
    candidates
}

fn ignored_route_graph_path(path: &str) -> bool {
    let classified = classify_profile_artifact(
        ProfileId::NextJs,
        path,
        ArtifactProvenance::WorkspaceObservation,
    );
    matches!(
        classified.kind,
        crate::agent::step_runner::profile_artifact::ArtifactKind::DependencyCache
            | crate::agent::step_runner::profile_artifact::ArtifactKind::BuildOutput
            | crate::agent::step_runner::profile_artifact::ArtifactKind::GeneratedDeclaration
            | crate::agent::step_runner::profile_artifact::ArtifactKind::Manifest
            | crate::agent::step_runner::profile_artifact::ArtifactKind::Config
    )
}

fn push_optional_fact(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        lines.push(format!("{key}={}", bounded_value(value)));
    }
}

fn bounded_join(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        return empty.to_string();
    }
    values
        .iter()
        .take(8)
        .map(|value| bounded_value(value))
        .collect::<Vec<_>>()
        .join(",")
}

fn bounded_value(value: &str) -> String {
    const MAX: usize = 120;
    let mut out = value.chars().take(MAX).collect::<String>();
    if value.chars().count() > MAX {
        out.push_str("...");
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    UnknownProfile(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownProfile(profile) => write!(f, "unknown profile: {profile}"),
        }
    }
}

impl std::error::Error for ProfileError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn parses_all_mvp_profiles() {
        for profile in [
            "generic",
            "nextjs",
            "python",
            "rust",
            "investigation",
            "docs",
            "data-analysis",
            "data-pipeline",
        ] {
            assert!(ProfileId::parse(profile).is_ok(), "{profile}");
        }
    }

    #[test]
    fn nextjs_profile_preserves_honest_build_contract() {
        let contract = profile_contract(ProfileId::NextJs);

        assert!(contract.text.contains("next/react/react-dom"));
        assert!(contract.text.contains("never fake build success"));
        assert_eq!(contract.verifier_commands, vec!["npm run build"]);
    }

    #[test]
    fn rust_profile_keeps_scaffolding_in_file_tools() {
        let contract = profile_contract(ProfileId::Rust);

        assert!(contract.text.contains("Cargo.toml"));
        assert!(contract.text.contains("src/main.rs"));
        assert!(contract.text.contains("instead of cargo init or cargo new"));
        assert!(contract.text.contains("CARGO_BIN_EXE_<name>"));
        assert!(contract.text.contains("matches the Cargo binary name"));
    }

    #[test]
    fn data_profiles_protect_raw_inputs() {
        assert!(protected_by_profile("data-analysis", "data/raw/source.csv").unwrap());
        assert!(protected_by_profile("data-pipeline", "raw/source.csv").unwrap());
        assert!(!protected_by_profile("data-analysis", "derived/report.csv").unwrap());
    }

    #[test]
    fn unknown_profile_is_error() {
        let err = ProfileId::parse("legacy").unwrap_err();

        assert_eq!(err, ProfileError::UnknownProfile("legacy".to_string()));
    }

    #[test]
    fn nextjs_summary_reports_root_app() {
        let root = temp_workspace("summary-root-app");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"},"dependencies":{"next":"latest","react":"latest","react-dom":"latest"}}"#,
        )
        .unwrap();

        let summary = profile_fact_summary("nextjs", &root).unwrap();

        assert!(summary.lines.contains(&"nextjs.app_root=app".to_string()));
        assert!(
            summary
                .lines
                .contains(&"nextjs.scripts.dev=next dev -p 3011".to_string())
        );
    }

    #[test]
    fn nextjs_summary_reports_mixed_roots() {
        let root = temp_workspace("summary-mixed");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("src/app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("src/app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();

        let summary = profile_fact_summary("nextjs", &root).unwrap();

        assert!(summary.lines.contains(&"nextjs.app_root=mixed".to_string()));
    }

    #[test]
    fn nextjs_obligations_include_requested_dev_port() {
        let obligations = profile_obligations(
            "nextjs",
            &obligation_context_with_goal("Create a Next.js app on port 3011"),
        )
        .unwrap();

        assert!(
            obligations
                .iter()
                .any(|obligation| obligation.code == "nextjs_dev_port_required")
        );
    }

    #[test]
    fn nextjs_obligations_include_build_and_dependencies_for_app_artifact() {
        let mut context = obligation_context_with_goal("Create app");
        context.required_artifacts = vec!["app/page.tsx".to_string()];

        let obligations = profile_obligations("nextjs", &context).unwrap();

        assert!(
            obligations
                .iter()
                .any(|obligation| obligation.code == "nextjs_build_script_required")
        );
        assert!(
            obligations
                .iter()
                .any(|obligation| obligation.code == "nextjs_dependencies_required")
        );
    }

    #[test]
    fn nextjs_obligations_include_tailwind_dependencies_when_requested() {
        let obligations = profile_obligations(
            "nextjs",
            &obligation_context_with_goal("Create a Next.js app with Tailwind CSS"),
        )
        .unwrap();

        let obligation = obligations
            .iter()
            .find(|obligation| obligation.code == "nextjs_tailwind_dependencies_required")
            .expect("tailwind dependency obligation");
        assert_eq!(
            obligation.expected.as_deref(),
            Some("dependencies include compatible tailwindcss, postcss, autoprefixer versions")
        );
    }

    #[test]
    fn nextjs_obligations_include_route_integration_for_explicit_source_artifact() {
        let mut context = obligation_context_with_goal("Create a Next.js game");
        context.required_artifacts = vec!["app/hooks/useGame.ts".to_string()];
        context.profile_facts = vec![
            "nextjs.routes=app/page.tsx".to_string(),
            "nextjs.app_root=app".to_string(),
        ];

        let obligations = profile_obligations("nextjs", &context).unwrap();

        let obligation = obligations
            .iter()
            .find(|obligation| obligation.code == "nextjs_route_integration_required")
            .expect("route integration obligation");
        assert_eq!(
            obligation.paths,
            vec![
                "app/page.tsx".to_string(),
                "app/hooks/useGame.ts".to_string()
            ]
        );
        assert_eq!(
            obligation.expected.as_deref(),
            Some(
                "selected route `app/page.tsx` references `app/hooks/useGame.ts` or its module name"
            )
        );
    }

    #[test]
    fn nextjs_obligations_do_not_require_route_integration_for_setup_paths_only() {
        let mut context = obligation_context_with_goal("Create a Next.js app");
        context.required_artifacts = vec![
            "package.json".to_string(),
            "app/page.tsx".to_string(),
            "tailwind.config.js".to_string(),
        ];
        context.profile_facts = vec![
            "nextjs.routes=app/page.tsx".to_string(),
            "nextjs.app_root=app".to_string(),
        ];

        let obligations = profile_obligations("nextjs", &context).unwrap();

        assert!(
            obligations
                .iter()
                .all(|obligation| obligation.code != "nextjs_route_integration_required")
        );
    }

    #[test]
    fn nextjs_obligations_do_not_use_workspace_entries_for_route_integration() {
        let mut context = obligation_context_with_goal("Create a Next.js app");
        context.phase_contract_facts = vec![
            "workspace.entries=app/,components/,next-env.d.ts".to_string(),
            "workspace.entries=components/Game.tsx".to_string(),
        ];
        context.profile_facts = vec![
            "nextjs.routes=app/page.tsx".to_string(),
            "nextjs.app_root=app".to_string(),
        ];

        let obligations = profile_obligations("nextjs", &context).unwrap();

        assert!(
            obligations
                .iter()
                .all(|obligation| obligation.code != "nextjs_route_integration_required"),
            "{obligations:?}"
        );
    }

    #[test]
    fn generic_profile_has_no_profile_obligations() {
        let obligations = profile_obligations(
            "generic",
            &obligation_context_with_goal("Create a Next.js app on port 3011"),
        )
        .unwrap();

        assert!(obligations.is_empty());
    }

    #[test]
    fn renders_profile_obligations_as_bounded_fact_lines() {
        let obligations = vec![ProfileObligation {
            code: "nextjs_dev_port_required".to_string(),
            message: "x".repeat(200),
            paths: vec!["package.json".to_string()],
            expected: Some("scripts.dev contains next dev and 3011".to_string()),
        }];

        let rendered = render_profile_obligations(&obligations);

        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].starts_with("profile.obligation.nextjs_dev_port_required="));
        assert!(rendered[0].contains("paths=package.json"));
        assert!(rendered[0].contains("expected=scripts.dev contains next dev and 3011"));
        assert!(rendered[0].contains("..."));
    }

    #[test]
    fn nextjs_verification_enforces_requested_port() {
        let root = temp_workspace("verify-port");
        write_minimal_next_app(&root, r#"{"dev":"next dev","build":"next build"}"#);

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_dev_port_drift")
        );
    }

    #[test]
    fn nextjs_verification_rejects_build_script_drift() {
        let root = temp_workspace("verify-build-script");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"echo ok"}"#);

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_build_script_drift")
        );
    }

    #[test]
    fn nextjs_verification_rejects_next14_react18_0_exact_pin() {
        let root = temp_workspace("verify-peer-version");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"},"dependencies":{"next":"14.0.0","react":"18.0.0","react-dom":"18.0.0"}}"#,
        )
        .unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_dependency_version_conflict")
        );
    }

    #[test]
    fn nextjs_verification_allows_react_version_range() {
        let root = temp_workspace("verify-peer-range");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"},"dependencies":{"next":"14.0.0","react":"^18.0.0","react-dom":"^18.0.0"}}"#,
        )
        .unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_dependency_version_conflict")
        );
    }

    #[test]
    fn nextjs_verification_rejects_postcss_autoprefixer_peer_conflict() {
        let root = temp_workspace("verify-postcss-autoprefixer-conflict");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"18.2.0","react-dom":"18.2.0","tailwindcss":"3.3.0","postcss":"8.0.0","autoprefixer":"10.0.0"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| failure.code == "nextjs_dependency_version_conflict")
            .expect("dependency conflict failure");
        assert!(failure.message.contains("autoprefixer@10.0.0"));
        assert!(failure.message.contains("postcss peer range ^8.0.2"));
        assert!(failure.message.contains("postcss@8.0.0"));
        assert_eq!(failure.paths, vec!["package.json".to_string()]);
    }

    #[test]
    fn nextjs_verification_accepts_compatible_postcss_autoprefixer_versions() {
        let root = temp_workspace("verify-postcss-autoprefixer-compatible");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"18.2.0","react-dom":"18.2.0","tailwindcss":"3.3.0","postcss":"8.4.31","autoprefixer":"10.4.16"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_dependency_version_conflict"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_accepts_postcss_version_range_for_tailwind_stack() {
        let root = temp_workspace("verify-postcss-autoprefixer-range");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"18.2.0","react-dom":"18.2.0","tailwindcss":"^3.3.0","postcss":"^8.4.0","autoprefixer":"^10.4.0"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_dependency_version_conflict"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_rejects_typescript_6_for_next14_generated_app() {
        let root = temp_workspace("verify-typescript-major");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"^18.2.0","react-dom":"^18.2.0","typescript":"6.0.3","@types/react":"18.2.79"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| failure.code == "nextjs_dependency_version_conflict")
            .expect("dependency conflict failure");
        assert!(failure.message.contains("typescript@6.0.3"));
        assert!(failure.message.contains("TypeScript 5.x"));
        assert_eq!(failure.paths, vec!["package.json".to_string()]);
    }

    #[test]
    fn nextjs_verification_rejects_react19_types_for_react18_generated_app() {
        let root = temp_workspace("verify-react-types-major");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"^18.2.0","react-dom":"^18.2.0","typescript":"^5.4.0","@types/react":"19.2.17"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| failure.code == "nextjs_dependency_version_conflict")
            .expect("dependency conflict failure");
        assert!(failure.message.contains("@types/react@19.2.17"));
        assert!(failure.message.contains("React 18 type family"));
        assert_eq!(failure.paths, vec!["package.json".to_string()]);
    }

    #[test]
    fn nextjs_verification_accepts_stable_typescript_toolchain() {
        let root = temp_workspace("verify-stable-typescript-toolchain");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"^18.2.0","react-dom":"^18.2.0","typescript":"^5.4.0","@types/react":"^18.2.79"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_dependency_version_conflict"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_rejects_tailwind_contract_drift() {
        let root = temp_workspace("verify-tailwind");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(root.join("app/globals.css"), "@tailwind base;").unwrap();
        fs::write(root.join("tailwind.config.js"), "module.exports = {}").unwrap();
        fs::write(root.join("postcss.config.js"), "module.exports = {}").unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_tailwind_contract")
        );
    }

    #[test]
    fn nextjs_verification_rejects_postcss_plugin_without_declared_package() {
        let root = temp_workspace("verify-postcss-plugin-dependency");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(root.join("app/globals.css"), "@tailwind base;").unwrap();
        fs::write(root.join("tailwind.config.js"), "module.exports = {}").unwrap();
        fs::write(
            root.join("postcss.config.js"),
            "module.exports = { plugins: { '@tailwindcss/postcss': {} } }",
        )
        .unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| {
                failure.code == "nextjs_tailwind_contract"
                    && failure.message.contains("@tailwindcss/postcss")
            })
            .expect("postcss plugin dependency failure");
        assert_eq!(
            failure.paths,
            vec!["package.json".to_string(), "postcss.config.js".to_string()]
        );
    }

    #[test]
    fn nextjs_verification_rejects_direct_tailwindcss_postcss_plugin_when_v4_plugin_declared() {
        let root = temp_workspace("verify-postcss-direct-plugin");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(root.join("app/globals.css"), "@tailwind base;").unwrap();
        fs::write(root.join("tailwind.config.js"), "module.exports = {}").unwrap();
        fs::write(
            root.join("postcss.config.js"),
            "module.exports = { plugins: { tailwindcss: {}, autoprefixer: {} } }",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"},"dependencies":{"next":"14.2.0","react":"18.2.0","react-dom":"18.2.0","tailwindcss":"latest","@tailwindcss/postcss":"latest","postcss":"latest","autoprefixer":"latest"}}"#,
        )
        .unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| {
                failure.code == "nextjs_tailwind_contract"
                    && failure.message.contains("tailwindcss directly")
            })
            .expect("direct postcss plugin failure");
        assert_eq!(failure.paths, vec!["postcss.config.js".to_string()]);
    }

    #[test]
    fn nextjs_verification_rejects_mixed_roots() {
        let root = temp_workspace("verify-mixed");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::create_dir_all(root.join("src/app")).unwrap();
        fs::write(
            root.join("src/app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_app_root_ambiguous")
        );
    }

    #[test]
    fn nextjs_verification_rejects_disconnected_source_artifact() {
        let root = temp_workspace("verify-route-disconnected");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::create_dir_all(root.join("app/hooks")).unwrap();
        fs::write(
            root.join("app/hooks/useGame.ts"),
            "export function useGame() {}",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/hooks/useGame.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_reports_missing_integration_artifact_before_route_drift() {
        let root = temp_workspace("verify-missing-integration-artifact");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["components/SpaceInvaders.tsx".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        let missing = failures
            .iter()
            .find(|failure| failure.code == "nextjs_integration_artifact_missing")
            .expect("missing integration artifact failure");
        assert_eq!(
            missing.paths,
            vec![
                "components/SpaceInvaders.tsx".to_string(),
                "app/page.tsx".to_string()
            ]
        );
    }

    #[test]
    fn nextjs_missing_integration_artifact_does_not_emit_route_not_integrated() {
        let root = temp_workspace("verify-missing-not-route-drift");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        let mut context = context_with_goal("run on port 3011");
        context.required_artifacts = vec!["components/SpaceInvaders.tsx".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_integration_artifact_missing"),
            "{failures:?}"
        );
        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_accepts_source_artifact_stem_reference_from_route() {
        let root = temp_workspace("verify-route-stem-reference");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(
            root.join("app/page.tsx"),
            "import { useGame } from './hooks/useGame'; export default function Page() { useGame(); return null }",
        )
        .unwrap();
        fs::create_dir_all(root.join("app/hooks")).unwrap();
        fs::write(
            root.join("app/hooks/useGame.ts"),
            "export function useGame() {}",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/hooks/useGame.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_accepts_source_artifact_path_reference_from_route() {
        let root = temp_workspace("verify-route-path-reference");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(
            root.join("app/page.tsx"),
            "const modulePath = 'app/hooks/useGame'; export default function Page() { return modulePath }",
        )
        .unwrap();
        fs::create_dir_all(root.join("app/hooks")).unwrap();
        fs::write(
            root.join("app/hooks/useGame.ts"),
            "export function useGame() {}",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/hooks/useGame.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_accepts_transitive_route_component_integration() {
        let root = temp_workspace("verify-route-graph-transitive");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::create_dir_all(root.join("app/components")).unwrap();
        fs::create_dir_all(root.join("app/hooks")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "import GameBoard from './components/GameBoard'; export default function Page() { return <GameBoard /> }",
        )
        .unwrap();
        fs::write(
            root.join("app/components/GameBoard.tsx"),
            "import { useGame } from '../hooks/useGame'; export default function GameBoard() { useGame(); return null }",
        )
        .unwrap();
        fs::write(
            root.join("app/hooks/useGame.ts"),
            "export function useGame() {}",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/hooks/useGame.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_route_graph_failure_can_target_component() {
        let root = temp_workspace("verify-route-graph-target");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::create_dir_all(root.join("app/components")).unwrap();
        fs::create_dir_all(root.join("app/hooks")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "import GameBoard from './components/GameBoard'; export default function Page() { return <GameBoard /> }",
        )
        .unwrap();
        fs::write(
            root.join("app/components/GameBoard.tsx"),
            "export default function GameBoard() { return null }",
        )
        .unwrap();
        fs::write(
            root.join("app/hooks/useGame.ts"),
            "export function useGame() {}",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/hooks/useGame.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        let failure = failures
            .iter()
            .find(|failure| failure.code == "nextjs_route_not_integrated")
            .expect("route integration failure");
        assert_eq!(
            failure.paths,
            vec![
                "app/page.tsx".to_string(),
                "app/hooks/useGame.ts".to_string(),
                "app/components/GameBoard.tsx".to_string(),
            ]
        );
        assert!(
            failure
                .message
                .contains("repair target `app/components/GameBoard.tsx`")
        );
    }

    #[test]
    fn nextjs_verification_does_not_require_layout_import_from_page() {
        let root = temp_workspace("verify-layout-infra");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(
            root.join("app/layout.tsx"),
            "export default function Layout({ children }: { children: React.ReactNode }) { return children }",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/page.tsx".to_string(), "app/layout.tsx".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_ignores_workspace_observed_next_env_declaration() {
        let root = temp_workspace("verify-next-env-workspace-entry");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(
            root.join("next-env.d.ts"),
            "/// <reference types=\"next\" />",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.phase_contract_facts =
            vec!["workspace.entries=app/,next-env.d.ts,package.json,tsconfig.json".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_ignores_required_generated_declarations() {
        let root = temp_workspace("verify-generated-declaration");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        let mut context = context_with_goal("run on port 3011");
        context.required_artifacts =
            vec!["next-env.d.ts".to_string(), "types/routes.d.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_ignores_profile_obligation_text_as_integration_path() {
        let root = temp_workspace("verify-obligation-text");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        let mut context = context_with_goal("run on port 3011");
        context.phase_contract_facts = vec![
            "profile.obligation.nextjs_dependencies_required=package.json dependencies must include compatible runtime packages for a Next.js app; paths=package.json; expected=dependencies include next, react, react-dom with React 18.2 or newer compatibility".to_string(),
        ];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    fn context_with_goal(goal: &str) -> ProfileVerificationContext {
        ProfileVerificationContext {
            goal_excerpt: goal.to_string(),
            required_artifacts: Vec::new(),
            expected_paths: Vec::new(),
            phase_contract_facts: Vec::new(),
            profile_facts: Vec::new(),
        }
    }

    fn obligation_context_with_goal(goal: &str) -> ProfileObligationContext {
        ProfileObligationContext {
            goal_excerpt: goal.to_string(),
            required_artifacts: Vec::new(),
            phase_contract_facts: Vec::new(),
            profile_facts: Vec::new(),
        }
    }

    fn write_minimal_next_app(root: &Path, scripts: &str) {
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            format!(
                r#"{{"scripts":{scripts},"dependencies":{{"next":"latest","react":"latest","react-dom":"latest"}}}}"#
            ),
        )
        .unwrap();
    }

    fn write_next_app_with_dependencies(root: &Path, dependencies: &str) {
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            format!(
                r#"{{"scripts":{{"dev":"next dev -p 3011","build":"next build"}},"dependencies":{dependencies}}}"#
            ),
        )
        .unwrap();
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-profiles-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
