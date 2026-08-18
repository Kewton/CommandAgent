use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use serde_yaml::Value;
use sha2::{Digest, Sha256};

use crate::bounded_process;
use crate::planner::profile::{DomainProfile, ProfileId, ProfileQualityExpectations};
use crate::planner::profile_behavior::ProfileRuntime;
use crate::planner::verify::VerificationReport;

mod computed;
mod promotion;

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
computed:
  - name: countPlusOne
    entity: counter
    expression: count + 1
    type: number
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

pub fn report_promotion_order(
    report: &mut crate::planner::lint::PlanQualityReport,
    context: &crate::planner::lint::PlanQualityContext,
    plan: &crate::planner::step_plan::StepPlan,
) {
    promotion::report_plan_quality(report, context, plan);
}

pub fn report_quality(
    report: &mut crate::planner::lint::PlanQualityReport,
    context: &crate::planner::lint::PlanQualityContext,
    verify_commands: &[&str],
    all_paths: &[&str],
    plan: &crate::planner::step_plan::StepPlan,
) {
    report_declared_verify(report, context, verify_commands, all_paths);
    report_promotion_order(report, context, plan);
}

pub fn ultra_phase_count_error(
    plan: &crate::planner::ultra_plan::UltraPlan,
) -> Option<&'static str> {
    if plan.profile == PROFILE_ID {
        (!(1..=8).contains(&plan.phases.len()))
            .then_some("Community UltraPlan must have 1-8 phases")
    } else {
        (!(2..=8).contains(&plan.phases.len())).then_some("UltraPlan must have 2-8 phases")
    }
}

pub fn report_ultra_plan_quality(
    report: &mut crate::planner::lint::PlanLintReport,
    plan: &crate::planner::ultra_plan::UltraPlan,
) {
    promotion::report_ultra_plan_quality(report, plan);
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

    #[test]
    fn app_zone_step_requires_a_preceding_promotion_step_for_community_only() {
        let zone = PlanStep {
            id: "implement-zone".into(),
            kind: "implement".into(),
            expected_result: "pass".into(),
            instruction: "Create src/app-zone/index.html and src/app-zone/app.ts".into(),
            expected_paths: vec![
                "src/app-zone/index.html".into(),
                "src/app-zone/app.ts".into(),
            ],
            verify: vec![
                "commandagent --offline --profile community-mini-app --prompt \"Validate app-zone\""
                    .into(),
            ],
        };
        let promotion = PlanStep {
            id: "record-promotion".into(),
            kind: "implement".into(),
            expected_result: "pass".into(),
            instruction: "Record promotion_decision after the passing L2 result".into(),
            expected_paths: vec![promotion::EVIDENCE_PATH.into()],
            verify: Vec::new(),
        };
        let context = PlanQualityContext {
            profile: PROFILE_ID.into(),
            required_artifacts: vec!["app.spec.yaml".into()],
            preferred_verify: vec!["commandagent --offline --profile community-mini-app".into()],
            ..Default::default()
        };

        let missing = StepPlan {
            goal: "Create a Community Mini App".into(),
            steps: vec![zone.clone(), promotion.clone()],
        };
        let report = step_plan_quality_report(&missing, &context);
        assert!(report.has_retryable_quality());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.category == "community_promotion_step_missing")
        );

        let ordered = StepPlan {
            steps: vec![promotion, zone],
            ..missing.clone()
        };
        let report = step_plan_quality_report(&ordered, &context);
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.category == "community_promotion_step_missing")
        );

        let l2_with_negative_boundary = StepPlan {
            goal: "Create an L2 Community Mini App".into(),
            steps: vec![PlanStep {
                id: "create-app-spec".into(),
                kind: "implement".into(),
                expected_result: "pass".into(),
                instruction:
                    "Create app.spec.yaml only. Do not create src/app-zone or L3 artifacts."
                        .into(),
                expected_paths: vec!["app.spec.yaml".into()],
                verify: vec![
                    "commandagent --offline --profile community-mini-app --prompt \"Validate app.spec.yaml\""
                        .into(),
                ],
            }],
        };
        let report = step_plan_quality_report(&l2_with_negative_boundary, &context);
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.category == "community_promotion_step_missing")
        );

        let nextjs = PlanQualityContext {
            profile: "nextjs".into(),
            ..context
        };
        let report = step_plan_quality_report(&missing, &nextjs);
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.category == "community_promotion_step_missing")
        );
    }

    #[test]
    fn app_zone_step_requires_build_materials_but_l2_plan_bytes_are_unchanged() {
        let promotion = PlanStep {
            id: "record-promotion".into(),
            kind: "implement".into(),
            expected_result: "pass".into(),
            instruction: "Record promotion_decision after passing L2".into(),
            expected_paths: vec![promotion::EVIDENCE_PATH.into()],
            verify: Vec::new(),
        };
        let zone = PlanStep {
            id: "implement-zone".into(),
            kind: "implement".into(),
            expected_result: "pass".into(),
            instruction: "Create the promoted Community app-zone and run B verification".into(),
            expected_paths: vec![
                "src/app-zone/index.html".into(),
                "src/app-zone/app.ts".into(),
            ],
            verify: vec![
                "commandagent --offline --profile community-mini-app --prompt \"Run B verification\""
                    .into(),
            ],
        };
        let context = PlanQualityContext {
            profile: PROFILE_ID.into(),
            required_artifacts: vec!["app.spec.yaml".into()],
            preferred_verify: vec!["commandagent --offline --profile community-mini-app".into()],
            ..Default::default()
        };
        let missing = StepPlan {
            goal: "Create a promoted Community Mini App".into(),
            steps: vec![promotion.clone(), zone.clone()],
        };
        let report = step_plan_quality_report(&missing, &context);
        assert!(report.has_retryable_quality());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.category == "community_build_material_step_missing")
        );

        let mut complete_zone = zone;
        complete_zone
            .expected_paths
            .extend(["package.json".into(), "package-lock.json".into()]);
        let complete = StepPlan {
            steps: vec![promotion, complete_zone],
            ..missing
        };
        let report = step_plan_quality_report(&complete, &context);
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.category == "community_build_material_step_missing")
        );

        let qwen_l2 = StepPlan {
            goal: "Create a Community Mini App at L2".into(),
            steps: vec![PlanStep {
                id: "create-spec".into(),
                kind: "implement".into(),
                expected_result: "pass".into(),
                instruction: "Write app.spec.yaml at the canonical L2 level".into(),
                expected_paths: vec!["app.spec.yaml".into()],
                verify: vec![
                    "commandagent --offline --profile community-mini-app --prompt \"Validate app.spec.yaml\""
                        .into(),
                ],
            }],
        };
        let before = serde_json::to_vec(&qwen_l2).unwrap();
        let report = step_plan_quality_report(&qwen_l2, &context);
        let after = serde_json::to_vec(&qwen_l2).unwrap();
        assert_eq!(before, after, "qwen27 L2 plan bytes must remain unchanged");
        assert!(!report.has_retryable_quality());
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
const ENTITY_ENTRY_FIELDS: &[&str] = &["name", "fields"];
const PINNED_SCHEMA_FIXTURE: &str = include_str!(
    "../../../workspace/management/bench/community/synthetic-community/schema/app-spec.schema.yaml"
);
const PINNED_CHAINED_SPEC_FIXTURE: &str = include_str!(
    "../../../workspace/management/bench/community/appspec-schema/positive/computed-chain/app.spec.yaml"
);
const FORBIDDEN_API_MARKERS: &[&str] =
    &["process.env", "eval(", "child_process", "fetch(", "import("];

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

fn chained_computed_guidance() -> String {
    let spec: Value = serde_yaml::from_str(PINNED_CHAINED_SPEC_FIXTURE)
        .expect("sealed chained-computed example must parse");
    serde_yaml::to_string(&spec["computed"])
        .expect("sealed chained-computed example must serialize")
        .replace('\n', "; ")
}

pub fn guidance() -> &'static str {
    static GUIDANCE: OnceLock<String> = OnceLock::new();
    GUIDANCE
        .get_or_init(|| {
            format!(
                "DATA-1:\n- L2 is the default canonical single-phase plan: write app.spec.yaml; run `commandagent --offline --profile community-mini-app --prompt \"Validate app.spec.yaml against the pinned schema; fail on violation.\"`; stop on pass.\n- Roots: {}. Schema-only metadata keys `schema_version`/`fields` invalid. Entity keys: `{}`; types: {}.\n- Computed keys: `{}`; local references; functions: {}; order shareAmount -> netBalance -> settlementAmount; self/mutual cycles are forbidden. Never invent `function`/`source`. Example: `{}`.\n- Minimal YAML: `{}`.\n- L3/L4 after L2 pass: write `evidence/promotion-decision.json` as `{{\"evidence_family\":\"promotion_decision\",\"attempt_id\":\"attempt-1\",\"requested_level\":\"L3\",\"decision\":\"promote\",\"reason_class\":\"ui_requirement\",\"lower_level_result\":{{\"status\":\"pass\",\"artifact_ref\":\"app.spec.yaml\"}},\"zone_path\":\"src/app-zone\"}}`; only the following step may write src/app-zone/index.html and app.ts and run B verify. reason_class: {}.\n- Pin input; core immutable. No process.env, eval, child_process, raw fetch, dynamic import, undeclared packages, build-time egress.\n",
                schema_vocabulary_guidance(),
                ENTITY_ENTRY_FIELDS.join(", "),
                ENTITY_FIELD_TYPES.join(", "),
                computed::ENTRY_FIELDS.join(", "),
                computed::ALLOWED_FUNCTIONS.join(", "),
                chained_computed_guidance(),
                MINIMAL_SPEC_EXAMPLE.replace('\n', "; "),
                promotion::REASON_CLASSES.join(", ")
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
    if value.get("schema_version").and_then(Value::as_str) != Some("community.app-spec/v0.1") {
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
    let mut entities_by_name = BTreeMap::<String, BTreeSet<String>>::new();
    if let Some(entities) = mapping[&Value::String("entities".to_string())].as_sequence() {
        for entity in entities {
            let entity_map = entity
                .as_mapping()
                .ok_or_else(|| "community_entity_invalid".to_string())?;
            if entity_map.keys().any(|key| {
                key.as_str()
                    .is_none_or(|key| !ENTITY_ENTRY_FIELDS.contains(&key))
            }) {
                return Err("community_entity_vocabulary_mismatch".to_string());
            }
            let name = entity_map
                .get(Value::String("name".to_string()))
                .and_then(Value::as_str)
                .ok_or_else(|| "community_entity_name_missing".to_string())?;
            let mut fields = BTreeSet::new();
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
            if entities_by_name.insert(name.to_string(), fields).is_some() {
                return Err(format!("community_entity_duplicate:{name}"));
            }
        }
    }
    if let Some(computed) = mapping[&Value::String("computed".to_string())].as_sequence() {
        computed::validate_graph(computed, &entities_by_name, ENTITY_FIELD_TYPES)?;
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
    promotion::verify(root)?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtifactLevel {
    L2SpecOnly,
    L3OrL4AppZone,
}

fn artifact_level(root: &Path) -> ArtifactLevel {
    if root.join("src/app-zone").exists() || root.join("app-zone").exists() {
        ArtifactLevel::L3OrL4AppZone
    } else {
        ArtifactLevel::L2SpecOnly
    }
}

fn verify_applicable_families(root: &Path) -> Result<(), String> {
    verify_spec(root).and_then(|_| verify_zone(root))?;
    if artifact_level(root) == ArtifactLevel::L3OrL4AppZone {
        verify_build_and_smoke(root)?;
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
        if let Err(reason) = verify_applicable_families(root) {
            return profile_failure(reason);
        }
        VerificationReport::pass()
    }

    fn guidance(&self, _goal: &str) -> Option<String> {
        Some(guidance().to_string())
    }

    fn runtime_contract(&self, _intent: &str, _goal: &str) -> String {
        "- Keep the Community Mini App at the lowest level that satisfies the goal.\n- Emit app.spec.yaml for L1/L2; use src/app-zone/ only with promotion_decision evidence.\n- L2 Full means S/Z/material verified; runtime smoke is covered by platform integration. L3/L4 requires S/Z/B.".to_string()
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
        for function in computed::ALLOWED_FUNCTIONS {
            assert!(text.contains(function));
        }
        assert!(text.contains("name, entity, expression, type"));
        assert!(text.contains("function`/`source"));
        assert!(text.contains("shareAmount -> netBalance -> settlementAmount"));
        assert!(text.contains("self/mutual cycles are forbidden"));
        assert!(text.contains("commandagent --offline --profile community-mini-app"));
        assert!(text.contains("canonical single-phase plan"));
        assert!(text.contains(promotion::EVIDENCE_PATH));
        assert!(text.contains("only the following step may write src/app-zone"));
        for reason in promotion::REASON_CLASSES {
            assert!(text.contains(reason));
        }
        assert!(
            text.chars().count() <= 2_000,
            "profile guidance must leave room below the 2,500-character step limit: {}",
            text.chars().count()
        );
    }

    #[test]
    fn ultra_plan_accepts_canonical_single_l2_phase_only_for_community() {
        use crate::planner::lint::lint_ultra_plan_report;
        use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

        let community = UltraPlan {
            goal: "Create a Community Mini App".to_string(),
            profile: PROFILE_ID.to_string(),
            style: "default".to_string(),
            intent: "create".to_string(),
            phases: vec![UltraPhase {
                id: "produce-l2-spec".to_string(),
                prompt: "Create and verify only app.spec.yaml as an L2 artifact.".to_string(),
            }],
        };
        assert!(lint_ultra_plan_report(&community).is_pass());

        let general = UltraPlan {
            profile: "nextjs".to_string(),
            ..community
        };
        assert_eq!(
            lint_ultra_plan_report(&general).primary_message(),
            "UltraPlan must have 2-8 phases"
        );
    }

    #[test]
    fn promoted_ultra_plan_requires_prior_spec_and_build_materials() {
        use crate::planner::lint::lint_ultra_plan_report;
        use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

        let missing = UltraPlan {
            goal: "Create a promoted Community Mini App".into(),
            profile: PROFILE_ID.into(),
            style: "default".into(),
            intent: "create".into(),
            phases: vec![UltraPhase {
                id: "zone".into(),
                prompt: "Create src/app-zone/index.html and src/app-zone/app.ts".into(),
            }],
        };
        let report = lint_ultra_plan_report(&missing);
        assert!(report.has_category("community_spec_phase_missing"));
        assert!(report.has_category("community_build_material_phase_missing"));

        let complete = UltraPlan {
            phases: vec![
                UltraPhase {
                    id: "spec".into(),
                    prompt: "Create and verify app.spec.yaml".into(),
                },
                UltraPhase {
                    id: "zone".into(),
                    prompt: "After promotion, create src/app-zone/index.html, src/app-zone/app.ts, package.json, and package-lock.json; run B verification".into(),
                },
            ],
            ..missing
        };
        let report = lint_ultra_plan_report(&complete);
        assert!(!report.has_category("community_spec_phase_missing"));
        assert!(!report.has_category("community_build_material_phase_missing"));

        let nextjs = UltraPlan {
            profile: "nextjs".into(),
            ..complete
        };
        let report = lint_ultra_plan_report(&nextjs);
        assert!(!report.has_category("community_spec_phase_missing"));
        assert!(!report.has_category("community_build_material_phase_missing"));
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
        assert_eq!(
            schema["computed_contract"]["entry_fields"],
            serde_yaml::to_value(computed::ENTRY_FIELDS).unwrap()
        );
        assert_eq!(
            schema["computed_contract"]["reference_scope"],
            "same_entity"
        );
        assert_eq!(
            schema["computed_contract"]["evaluation_order"],
            "topological"
        );
        assert_eq!(schema["computed_contract"]["cycles"], "violation");

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
    fn chained_example_and_cycle_fixtures_follow_the_v01_schema() {
        fn verify_fixture(spec: &str) -> Result<(), String> {
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
            std::fs::write(root.path().join("app.spec.yaml"), spec).unwrap();
            verify_spec(root.path())
        }

        assert_eq!(verify_fixture(PINNED_CHAINED_SPEC_FIXTURE), Ok(()));
        assert!(guidance().contains(&chained_computed_guidance()));

        let self_cycle = include_str!(
            "../../../workspace/management/bench/community/appspec-schema/negative/computed-self-cycle/app.spec.yaml"
        );
        assert_eq!(
            verify_fixture(self_cycle),
            Err("community_computed_cycle:expense.selfAmount".to_string())
        );
        let mutual_cycle = include_str!(
            "../../../workspace/management/bench/community/appspec-schema/negative/computed-mutual-cycle/app.spec.yaml"
        );
        assert_eq!(
            verify_fixture(mutual_cycle),
            Err("community_computed_cycle:expense.leftAmount,expense.rightAmount".to_string())
        );
    }

    #[test]
    fn removed_v0_schema_is_rejected_after_the_replacement_ceremony() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("schema")).unwrap();
        let old = PINNED_SCHEMA_FIXTURE.replace(
            "schema_version: community.app-spec/v0.1",
            "schema_version: community.app-spec/v1",
        );
        std::fs::write(root.path().join("schema/app-spec.schema.yaml"), &old).unwrap();
        std::fs::write(
            root.path().join("schema/app-spec.schema.sha256"),
            format!("{:x}\n", Sha256::digest(old.as_bytes())),
        )
        .unwrap();
        std::fs::write(root.path().join("app.spec.yaml"), MINIMAL_SPEC_EXAMPLE).unwrap();
        assert_eq!(
            verify_spec(root.path()),
            Err("community_schema_version_invalid".to_string())
        );
    }

    #[test]
    fn missing_core_manifest_remains_fail_closed() {
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
        assert_eq!(
            verify_zone(root.path()),
            Err("community_core_manifest_missing".to_string())
        );
    }

    fn write_l2_fixture(root: &Path) {
        std::fs::create_dir_all(root.join("schema")).unwrap();
        std::fs::create_dir_all(root.join("core")).unwrap();
        std::fs::write(
            root.join("schema/app-spec.schema.yaml"),
            PINNED_SCHEMA_FIXTURE,
        )
        .unwrap();
        std::fs::write(
            root.join("schema/app-spec.schema.sha256"),
            format!("{:x}\n", Sha256::digest(PINNED_SCHEMA_FIXTURE.as_bytes())),
        )
        .unwrap();
        std::fs::write(root.join("app.spec.yaml"), MINIMAL_SPEC_EXAMPLE).unwrap();
        std::fs::write(root.join("core/README.md"), "immutable core\n").unwrap();
        let core_digest = sha256(&root.join("core/README.md")).unwrap();
        std::fs::write(
            root.join("core.sha256sums"),
            format!("{core_digest}  core/README.md\n"),
        )
        .unwrap();
    }

    fn copy_l3_fixture(root: &Path) {
        let sealed_l3_base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("workspace/management/bench/community/synthetic-community");
        for relative in [
            "app.spec.yaml",
            "core.sha256sums",
            "core/README.md",
            "package.json",
            "package-lock.json",
            "schema/app-spec.schema.yaml",
            "schema/app-spec.schema.sha256",
            "src/app-zone/index.html",
            "src/app-zone/app.ts",
            "evidence/browser-interaction.json",
        ] {
            let destination = root.join(relative);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::copy(sealed_l3_base.join(relative), destination).unwrap();
        }
    }

    fn write_valid_promotion(root: &Path) {
        let path = root.join(promotion::EVIDENCE_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "evidence_family": "promotion_decision",
                "attempt_id": "fixture-attempt-1",
                "requested_level": "L3",
                "decision": "promote",
                "reason_class": "ui_requirement",
                "lower_level_result": {
                    "status": "pass",
                    "artifact_ref": "app.spec.yaml"
                },
                "zone_path": "src/app-zone"
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn l2_spec_only_does_not_run_build_and_smoke() {
        let root = tempfile::tempdir().unwrap();
        write_l2_fixture(root.path());

        assert_eq!(artifact_level(root.path()), ArtifactLevel::L2SpecOnly);
        assert!(
            CommunityMiniAppProfile
                .verify_final(root.path(), "")
                .is_pass()
        );
    }

    #[test]
    fn l2_single_phase_pass_projects_full_without_runtime_smoke_claim() {
        use crate::eval_events::CompletionSnapshot;
        use crate::planner::lint::{PlanQualityContext, step_plan_quality_report};
        use crate::planner::step_plan::{PlanStep, StepPlan};

        let root = tempfile::tempdir().unwrap();
        write_l2_fixture(root.path());
        let plan = StepPlan {
            goal: "Create a Community Mini App at L2".into(),
            steps: vec![PlanStep {
                id: "create-and-verify-spec".into(),
                kind: "implement".into(),
                expected_result: "pass".into(),
                instruction: "Write and validate app.spec.yaml as the complete L2 artifact".into(),
                expected_paths: vec!["app.spec.yaml".into()],
                verify: vec!["commandagent --offline --profile community-mini-app --prompt \"Validate app.spec.yaml against the pinned Community AppSpec schema and exit non-zero on violation.\"".into()],
            }],
        };
        let context = PlanQualityContext {
            profile: PROFILE_ID.into(),
            required_artifacts: vec!["app.spec.yaml".into()],
            preferred_verify: vec!["commandagent --offline --profile community-mini-app".into()],
            ..Default::default()
        };
        assert!(!step_plan_quality_report(&plan, &context).has_retryable_quality());
        assert!(
            CommunityMiniAppProfile
                .verify_final(root.path(), "")
                .is_pass()
        );

        let mut snapshot = CompletionSnapshot::empty();
        snapshot.final_acceptance_status =
            crate::planner::adjudication::final_acceptance_status_from_release_gate(
                "not_applicable",
            )
            .to_string();
        CommunityMiniAppProfile.apply_completion_snapshot(
            &ProfileId::CommunityMiniApp,
            root.path(),
            &mut snapshot,
        );
        assert_eq!(snapshot.final_acceptance_status, "full_success");
        assert_eq!(snapshot.assurance_level, "full");
        assert_eq!(snapshot.runtime_acceptance_status, "not_checked");
    }

    #[test]
    fn app_zone_without_promotion_is_a_zone_violation() {
        let root = tempfile::tempdir().unwrap();
        copy_l3_fixture(root.path());

        let report = CommunityMiniAppProfile.verify_final(root.path(), "");
        assert_eq!(report.profile_failures, vec!["community_promotion_missing"]);
    }

    #[test]
    fn l3_app_zone_keeps_build_and_smoke_mandatory() {
        let sealed_l3_base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("workspace/management/bench/community/synthetic-community");
        assert_eq!(
            artifact_level(&sealed_l3_base),
            ArtifactLevel::L3OrL4AppZone,
            "the adversarial suite base must remain an L3 fixture"
        );

        let root = tempfile::tempdir().unwrap();
        copy_l3_fixture(root.path());
        write_valid_promotion(root.path());
        std::fs::remove_file(root.path().join("src/app-zone/app.ts")).unwrap();

        let report = CommunityMiniAppProfile.verify_final(root.path(), "");
        assert_eq!(
            report.profile_failures,
            vec!["community_build_inputs_missing"]
        );
    }

    #[test]
    fn rust_and_python_reference_verdicts_match_on_the_same_fixture() {
        use std::process::Command;

        let root = tempfile::tempdir().unwrap();
        copy_l3_fixture(root.path());
        write_valid_promotion(root.path());
        let rust = CommunityMiniAppProfile.verify_s_z(root.path());
        assert!(rust.is_pass(), "Rust verifier failed: {rust:?}");
        let scripts =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workspace/management/scripts");
        let output = Command::new("python3")
            .env("PYTHONPATH", &scripts)
            .args([
                scripts.join("community_profile.py").to_str().unwrap(),
                "--spec",
                root.path().join("app.spec.yaml").to_str().unwrap(),
                "--schema",
                root.path()
                    .join("schema/app-spec.schema.yaml")
                    .to_str()
                    .unwrap(),
                "--schema-pin",
                root.path()
                    .join("schema/app-spec.schema.sha256")
                    .to_str()
                    .unwrap(),
                "--root",
                root.path().to_str().unwrap(),
                "--core-manifest",
                root.path().join("core.sha256sums").to_str().unwrap(),
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
