use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use serde_yaml::Value;
use sha2::{Digest, Sha256};

use crate::bounded_process;
use crate::planner::profile::{DomainProfile, ProfileId, ProfileQualityExpectations};
use crate::planner::profile_behavior::ProfileRuntime;
use crate::planner::verify::VerificationReport;

pub const PROFILE_ID: &str = "community-mini-app";
pub const PROMOTION_DECISION_EVIDENCE_FAMILY: &str = "promotion_decision";
pub fn is_strong_verify_command(command: &str) -> bool {
    command
        .trim()
        .to_ascii_lowercase()
        .starts_with("commandagent --offline --profile community-mini-app")
}
pub const MINIMAL_SPEC_EXAMPLE: &str = r#"entities:
  - name: counter
    fields:
      count: number
views:
  - name: count
    entity: counter
actions:
  - name: increment
    entity: counter
validations: []
computed: []
permissions:
  - name: read
    subject: minIdentity
minIdentity:
  mode: anonymous
"#;

pub fn declared_verify_missing(
    profile: &str,
    preferred_verify: &[String],
    verify_commands: &[&str],
    all_paths: &[&str],
) -> bool {
    profile == PROFILE_ID
        && !preferred_verify.is_empty()
        && all_paths.contains(&"app.spec.yaml")
        && !verify_commands.iter().any(|command| {
            preferred_verify
                .iter()
                .any(|preferred| command.contains(preferred))
        })
}

pub fn enforce_declared_verify(
    report: &mut crate::planner::lint::PlanQualityReport,
    profile: &str,
    preferred_verify: &[String],
    verify_commands: &[&str],
    all_paths: &[&str],
) {
    if declared_verify_missing(profile, preferred_verify, verify_commands, all_paths) {
        report.push(
            crate::planner::lint::PlanQualitySeverity::RetryableQuality,
            "profile_verify_missing",
            "community profile requires the declared schema verification command after app.spec.yaml",
            None,
            Some(preferred_verify.join(", ")),
        );
    }
}

pub fn report_declared_verify(
    report: &mut crate::planner::lint::PlanQualityReport,
    context: &crate::planner::lint::PlanQualityContext,
    verify_commands: &[&str],
    all_paths: &[&str],
) {
    enforce_declared_verify(
        report,
        &context.profile,
        &context.preferred_verify,
        verify_commands,
        all_paths,
    );
}

#[cfg(test)]
mod planner_quality_tests {
    use super::*;
    use crate::planner::lint::{PlanQualityContext, step_plan_quality_report};
    use crate::planner::step_plan::{PlanStep, StepPlan};

    #[test]
    fn declared_schema_verify_is_required_and_strong() {
        let weak = StepPlan {
            goal: "Create a Community Mini App".into(),
            steps: vec![PlanStep {
                id: "spec".into(),
                kind: "implement".into(),
                expected_result: "pass".into(),
                instruction: "Write app.spec.yaml".into(),
                expected_paths: vec!["app.spec.yaml".into()],
                verify: vec!["test -f app.spec.yaml".into()],
            }],
        };
        let context = PlanQualityContext {
            profile: PROFILE_ID.into(),
            required_artifacts: vec!["app.spec.yaml".into()],
            preferred_verify: vec!["commandagent --offline --profile community-mini-app".into()],
            ..Default::default()
        };
        let report = step_plan_quality_report(&weak, &context);
        assert!(
            report.has_retryable_quality()
                && report
                    .issues
                    .iter()
                    .any(|i| i.category == "profile_verify_missing")
        );
        let strong = StepPlan {
            steps: vec![PlanStep {
                verify: vec!["commandagent --offline --profile community-mini-app".into()],
                ..weak.steps[0].clone()
            }],
            ..weak
        };
        let report = step_plan_quality_report(&strong, &context);
        assert!(!report.issues.iter().any(|i| i.category == "profile_verify_missing" || i.category == "weak_code_verify"));
    }
}

pub struct CommunityMiniAppProfile;

const ROOT_FIELDS: &[&str] = &[
    "entities",
    "views",
    "actions",
    "validations",
    "computed",
    "permissions",
    "minIdentity",
];
const ENTITY_FIELD_TYPES: &[&str] = &["number", "string", "boolean", "list"];
const ALLOWED_COMPUTED_FUNCTIONS: &[&str] = &["min", "max", "len"];
const PINNED_SCHEMA_FIXTURE: &str = include_str!(
    "../../../workspace/management/bench/community/synthetic-community/schema/app-spec.schema.yaml"
);
const FORBIDDEN_API_MARKERS: &[&str] =
    &["process.env", "eval(", "child_process", "fetch(", "import("];
const MAX_COMPUTED_NODES: usize = 64;

fn schema_vocabulary_guidance() -> String {
    let schema: Value = serde_yaml::from_str(PINNED_SCHEMA_FIXTURE)
        .expect("pinned Community AppSpec schema fixture must parse");
    let fields = schema
        .get("fields")
        .and_then(Value::as_mapping)
        .expect("pinned Community AppSpec schema fixture must declare fields");
    ROOT_FIELDS
        .iter()
        .map(|field| {
            let kind = fields
                .get(Value::String((*field).to_string()))
                .and_then(Value::as_str)
                .expect("every Community AppSpec root field must declare its kind");
            format!("{field}:{kind}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn guidance() -> &'static str {
    static GUIDANCE: OnceLock<String> = OnceLock::new();
    GUIDANCE
        .get_or_init(|| {
            format!(
                "Community Mini App generation rules (DATA-1):\n- L2 is the default and must be attempted first; generate exactly app.spec.yaml with entities/views/actions/validations/computed/permissions/minIdentity.\n- Closed root vocabulary generated from the pinned schema fixture: {}. These seven keys are the entire app.spec.yaml root. Schema-only metadata keys `schema_version` and `fields` belong to the injected schema and must never be written at the app.spec.yaml root.\n- Entity field types are the verifier-registered closed set: {}. View and action names are goal-defined identifiers in v0; v0 declares no separate kind enum. Computed registered pure functions are: {}. Do not invent another type, kind enum, or function.\n- The canonical L2 plan shape is: write app.spec.yaml, then verify it with the product-internal, workspace-self-contained command `commandagent --offline --profile community-mini-app --prompt \"Validate app.spec.yaml against the pinned Community AppSpec schema and exit non-zero on violation.\"`. This command performs the pinned schema and AppSpec verification without dependency setup; do not use a file-existence-only check.\n- Minimal complete YAML字義例 (the exact bytes are machine-checked by the product verifier): `{}`.\n- Promote to L3/L4 only under src/app-zone/ and record a machine-readable promotion_decision with the lower-level result and reason; the promoted plan adds an app-zone implementation step and a verify step.\n- The platform-owned schema is a pinned input; never replace, weaken, or infer it.\n- Core paths are immutable. Do not use process.env, eval, child_process, raw fetch, dynamic import, undeclared packages, or build-time egress.\n- Keep computed expressions bounded, statically typed, and inside the registered pure-function set.\n",
                schema_vocabulary_guidance(),
                ENTITY_FIELD_TYPES.join(", "),
                ALLOWED_COMPUTED_FUNCTIONS.join(", "),
                MINIMAL_SPEC_EXAMPLE.replace('\n', "; ")
            )
        })
        .as_str()
}

fn sha256(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(format!("{:x}", Sha256::digest(bytes)))
}

fn profile_failure(reason: impl Into<String>) -> VerificationReport {
    crate::planner::profile::profile_failure(reason)
}

fn schema_paths(root: &Path) -> (PathBuf, PathBuf) {
    let hidden = root.join(".community/schema/app-spec.schema.yaml");
    if hidden.is_file() {
        return (
            hidden,
            root.join(".community/schema/app-spec.schema.sha256"),
        );
    }
    (
        root.join("schema/app-spec.schema.yaml"),
        root.join("schema/app-spec.schema.sha256"),
    )
}

fn verify_schema_pin(root: &Path) -> Result<(), String> {
    let (schema, pin) = schema_paths(root);
    let observed = sha256(&schema).ok_or_else(|| "community_schema_missing".to_string())?;
    let expected = std::fs::read_to_string(&pin)
        .map_err(|_| "community_schema_pin_missing".to_string())?
        .trim()
        .to_string();
    if expected != observed {
        return Err(format!(
            "community_schema_pin_mismatch:{expected}!={observed}"
        ));
    }
    let value: Value = serde_yaml::from_str(
        &std::fs::read_to_string(schema).map_err(|_| "community_schema_unreadable".to_string())?,
    )
    .map_err(|_| "community_schema_invalid".to_string())?;
    if value.get("schema_version").and_then(Value::as_str) != Some("community.app-spec/v1") {
        return Err("community_schema_version_invalid".to_string());
    }
    Ok(())
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Sequence(_) => "list",
        Value::Mapping(_) => "mapping",
        Value::Tagged(_) => "unknown",
    }
}

fn validate_computed(expression: &str, fields: &BTreeSet<String>) -> Result<(), String> {
    let tokens = expression
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.len() > MAX_COMPUTED_NODES {
        return Err("community_computed_ast_limit".to_string());
    }
    for token in tokens {
        if token == "eval" || token == "fetch" || token == "process" || token == "import" {
            return Err(format!("community_computed_forbidden:{token}"));
        }
        if token
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic())
            && !fields.contains(token)
            && !matches!(token, "true" | "false")
            && !ALLOWED_COMPUTED_FUNCTIONS.contains(&token)
        {
            return Err(format!("community_computed_unregistered:{token}"));
        }
    }
    Ok(())
}

fn verify_spec(root: &Path) -> Result<(), String> {
    verify_schema_pin(root)?;
    let path = root.join("app.spec.yaml");
    let value: Value = serde_yaml::from_str(
        &std::fs::read_to_string(path).map_err(|_| "community_spec_missing".to_string())?,
    )
    .map_err(|_| "community_spec_invalid_yaml".to_string())?;
    let mapping = value
        .as_mapping()
        .ok_or_else(|| "community_spec_not_mapping".to_string())?;
    let keys = mapping
        .keys()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let expected = ROOT_FIELDS.iter().copied().collect::<BTreeSet<_>>();
    if keys != expected {
        return Err("community_spec_closed_vocabulary".to_string());
    }
    for field in ROOT_FIELDS {
        let expected_kind = if *field == "minIdentity" {
            "mapping"
        } else {
            "list"
        };
        if value_kind(&mapping[&Value::String((*field).to_string())]) != expected_kind {
            return Err(format!("community_spec_type:{field}"));
        }
    }
    let mut fields = BTreeSet::new();
    if let Some(entities) = mapping[&Value::String("entities".to_string())].as_sequence() {
        for entity in entities {
            let entity_map = entity
                .as_mapping()
                .ok_or_else(|| "community_entity_invalid".to_string())?;
            let name = entity_map
                .get(Value::String("name".to_string()))
                .and_then(Value::as_str)
                .ok_or_else(|| "community_entity_name_missing".to_string())?;
            if let Some(entity_fields) = entity_map
                .get(Value::String("fields".to_string()))
                .and_then(Value::as_mapping)
            {
                for (field, field_type) in entity_fields {
                    let field = field
                        .as_str()
                        .ok_or_else(|| "community_field_name_invalid".to_string())?;
                    if !field_type
                        .as_str()
                        .is_some_and(|kind| ENTITY_FIELD_TYPES.contains(&kind))
                    {
                        return Err(format!("community_field_type:{field}"));
                    }
                    fields.insert(field.to_string());
                }
            }
            fields.insert(name.to_string());
        }
    }
    if let Some(computed) = mapping[&Value::String("computed".to_string())].as_sequence() {
        for item in computed {
            let item = item
                .as_mapping()
                .ok_or_else(|| "community_computed_invalid".to_string())?;
            let expression = item
                .get(Value::String("expression".to_string()))
                .and_then(Value::as_str)
                .ok_or_else(|| "community_computed_expression_missing".to_string())?;
            validate_computed(expression, &fields)?;
            if item
                .get(Value::String("type".to_string()))
                .and_then(Value::as_str)
                .is_none()
            {
                return Err("community_computed_type_missing".to_string());
            }
        }
    }
    Ok(())
}

fn verify_zone(root: &Path) -> Result<(), String> {
    let hidden = root.join(".community/core.sha256sums");
    let manifest = if hidden.is_file() {
        hidden
    } else {
        root.join("core.sha256sums")
    };
    let expected = std::fs::read_to_string(&manifest)
        .map_err(|_| "community_core_manifest_missing".to_string())?;
    for line in expected.lines().filter(|line| !line.trim().is_empty()) {
        let mut parts = line.split_whitespace();
        let digest = parts
            .next()
            .ok_or_else(|| "community_core_manifest_invalid".to_string())?;
        let relative = parts
            .next()
            .ok_or_else(|| "community_core_manifest_invalid".to_string())?;
        if sha256(&root.join(relative)).as_deref() != Some(digest) {
            return Err(format!("community_core_diff:{relative}"));
        }
    }
    for path in walk_sources(root) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if let Some(marker) = FORBIDDEN_API_MARKERS
            .iter()
            .find(|marker| text.contains(**marker))
        {
            return Err(format!(
                "community_forbidden_api:{marker}:{}",
                path.display()
            ));
        }
    }
    let package = root.join("package.json");
    if package.is_file() {
        let value: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(package)
                .map_err(|_| "community_package_unreadable".to_string())?,
        )
        .map_err(|_| "community_package_invalid".to_string())?;
        if value
            .get("dependencies")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|dependencies| !dependencies.is_empty())
        {
            return Err("community_dependency_allowlist_empty".to_string());
        }
        if !root.join("package-lock.json").is_file() {
            return Err("community_lockfile_missing".to_string());
        }
    }
    Ok(())
}

fn verify_build_and_smoke(root: &Path) -> Result<(), String> {
    let zone = if root.join("src/app-zone").is_dir() {
        root.join("src/app-zone")
    } else {
        root.join("app-zone")
    };
    let html = zone.join("index.html");
    let source = zone.join("app.ts");
    let evidence = root.join("evidence/browser-interaction.json");
    if !html.is_file() || !source.is_file() {
        return Err("community_build_inputs_missing".to_string());
    }
    let package = root.join("package.json");
    let package_value: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(package).map_err(|_| "community_package_missing".to_string())?,
    )
    .map_err(|_| "community_package_invalid".to_string())?;
    let build = package_value
        .get("scripts")
        .and_then(serde_json::Value::as_object)
        .and_then(|scripts| scripts.get("build"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !build.contains("esbuild") {
        return Err("community_esbuild_script_missing".to_string());
    }
    let output = std::env::temp_dir().join(format!(
        "commandagent-community-{}-bundle.js",
        std::process::id()
    ));
    let mut command = Command::new("esbuild");
    command
        .arg(&source)
        .arg("--bundle")
        .arg("--format=esm")
        .arg(format!("--outfile={}", output.display()));
    let result =
        bounded_process::run_with_timeout(&mut command, std::time::Duration::from_secs(30))
            .map_err(|_| "community_esbuild_unavailable".to_string())?;
    if !result.success() {
        return Err("community_esbuild_failed".to_string());
    }
    let _ = std::fs::remove_file(output);
    let browser: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(evidence)
            .map_err(|_| "community_browser_evidence_missing".to_string())?,
    )
    .map_err(|_| "community_browser_evidence_invalid".to_string())?;
    if browser.get("status").and_then(serde_json::Value::as_str) != Some("pass")
        || browser
            .get("managed_probe")
            .and_then(serde_json::Value::as_str)
            != Some("managed_interaction_probe")
    {
        return Err("community_browser_smoke_not_proven".to_string());
    }
    for selector in browser
        .get("assertions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "community_browser_assertions_missing".to_string())?
    {
        let selector = selector
            .as_str()
            .ok_or_else(|| "community_browser_assertion_invalid".to_string())?;
        if !std::fs::read_to_string(&html)
            .map_err(|_| "community_html_unreadable".to_string())?
            .contains(selector)
        {
            return Err(format!("community_appspec_assertion_missing:{selector}"));
        }
    }
    Ok(())
}

fn walk_sources(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && path.file_name().and_then(|name| name.to_str()) != Some("node_modules")
            {
                pending.push(path);
            } else if path.is_file()
                && matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("ts" | "tsx" | "js" | "jsx" | "mjs")
                )
            {
                paths.push(path);
            }
        }
    }
    paths
}

impl DomainProfile for CommunityMiniAppProfile {
    fn id(&self) -> &'static str {
        PROFILE_ID
    }

    fn expected_scaffold_paths(&self, _root: &Path, _goal: &str) -> Vec<String> {
        vec!["app.spec.yaml".to_string()]
    }

    fn setup_scaffold_paths(&self, _root: &Path) -> Vec<String> {
        vec!["app.spec.yaml".to_string()]
    }

    fn verify_final(&self, root: &Path, _goal: &str) -> VerificationReport {
        if let Err(reason) = verify_spec(root)
            .and_then(|_| verify_zone(root))
            .and_then(|_| verify_build_and_smoke(root))
        {
            return profile_failure(reason);
        }
        VerificationReport::pass()
    }

    fn guidance(&self, _goal: &str) -> Option<String> {
        Some(guidance().to_string())
    }

    fn runtime_contract(&self, _intent: &str, _goal: &str) -> String {
        "- Keep the Community Mini App at the lowest level that satisfies the goal.\n- Emit app.spec.yaml for L1/L2; use src/app-zone/ only with promotion_decision evidence.".to_string()
    }

    fn generation_rules(&self, _intent: &str) -> Option<&'static str> {
        Some(guidance())
    }

    fn quality_expectations(&self, _root: &Path, _goal: &str) -> ProfileQualityExpectations {
        ProfileQualityExpectations {
            required_artifacts: vec!["app.spec.yaml".to_string()],
            preferred_verify: vec![
                "commandagent --offline --profile community-mini-app".to_string(),
            ],
            forbidden_verify: vec!["npm install".to_string()],
            dependency_order_hint: Some("app.spec.yaml before app-zone promotion".to_string()),
        }
    }
}

impl CommunityMiniAppProfile {
    #[cfg(test)]
    fn verify_s_z(&self, root: &Path) -> VerificationReport {
        match verify_spec(root).and_then(|_| verify_zone(root)) {
            Ok(()) => VerificationReport::pass(),
            Err(reason) => profile_failure(reason),
        }
    }
}

impl ProfileRuntime for CommunityMiniAppProfile {
    fn profile_id(&self) -> ProfileId {
        ProfileId::CommunityMiniApp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::profile::{ProfileRuntimeRegistry, profile_names};

    #[test]
    fn profile_is_registered_at_typed_dispatch_boundary() {
        assert!(profile_names().contains(&PROFILE_ID));
        assert_eq!(
            ProfileRuntimeRegistry::resolve(&ProfileId::CommunityMiniApp).profile_id(),
            ProfileId::CommunityMiniApp
        );
    }

    #[test]
    fn guidance_binds_lowest_level_and_promotion_decision() {
        let text = guidance();
        assert!(text.contains("L2 is the default"));
        assert!(text.contains("src/app-zone/"));
        assert!(text.contains(PROMOTION_DECISION_EVIDENCE_FAMILY));
        assert!(text.contains("process.env"));
        assert!(text.contains("Schema-only metadata keys"));
        assert!(text.contains("entities:list"));
        assert!(text.contains("computed:list"));
        for function in ALLOWED_COMPUTED_FUNCTIONS {
            assert!(text.contains(function));
        }
        assert!(text.contains("commandagent --offline --profile community-mini-app"));
    }

    #[test]
    fn minimal_spec_example_matches_pinned_schema_fixture() {
        let example: Value = serde_yaml::from_str(MINIMAL_SPEC_EXAMPLE).unwrap();
        let schema: Value =
            serde_yaml::from_str(
                &std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                    "workspace/management/bench/community/synthetic-community/schema/app-spec.schema.yaml",
                ))
                .unwrap(),
            )
            .unwrap();
        assert!(example.get("schema_version").is_none());
        assert!(example.get("fields").is_none());
        assert_eq!(
            example.as_mapping().unwrap().len(),
            schema["fields"].as_mapping().unwrap().len()
        );
        for key in ROOT_FIELDS {
            assert!(example.get(*key).is_some(), "example missing {key}");
            assert!(guidance().contains(&format!(
                "{key}:{}",
                schema["fields"][*key].as_str().unwrap()
            )));
        }

        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("schema")).unwrap();
        std::fs::write(
            root.path().join("schema/app-spec.schema.yaml"),
            PINNED_SCHEMA_FIXTURE,
        )
        .unwrap();
        std::fs::write(
            root.path().join("schema/app-spec.schema.sha256"),
            format!("{:x}\n", Sha256::digest(PINNED_SCHEMA_FIXTURE.as_bytes())),
        )
        .unwrap();
        std::fs::write(root.path().join("app.spec.yaml"), MINIMAL_SPEC_EXAMPLE).unwrap();
        assert_eq!(verify_spec(root.path()), Ok(()));
    }

    #[test]
    fn rust_and_python_reference_verdicts_match_on_the_same_fixture() {
        use std::process::Command;

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("workspace/management/bench/community/synthetic-community");
        let rust = CommunityMiniAppProfile.verify_s_z(&root);
        assert!(rust.is_pass(), "Rust verifier failed: {rust:?}");
        let scripts =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workspace/management/scripts");
        let output = Command::new("python3")
            .env("PYTHONPATH", &scripts)
            .args([
                scripts.join("community_profile.py").to_str().unwrap(),
                "--spec",
                root.join("app.spec.yaml").to_str().unwrap(),
                "--schema",
                root.join("schema/app-spec.schema.yaml").to_str().unwrap(),
                "--schema-pin",
                root.join("schema/app-spec.schema.sha256").to_str().unwrap(),
                "--root",
                root.to_str().unwrap(),
                "--core-manifest",
                root.join("core.sha256sums").to_str().unwrap(),
            ])
            .output()
            .expect("Python reference implementation must be runnable");
        assert!(
            output.status.success(),
            "Python reference failed: {:?}",
            output
        );
        let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["verdict"], "pass");
        assert_eq!(document["zone"]["verdict"], "pass");
    }
}
