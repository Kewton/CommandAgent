use std::path::Path;

use crate::config::Config;
use crate::minimal_loop::build_verifier;
use crate::minimal_loop::dependency_setup;
use crate::minimal_loop::evidence::{SatisfactionChannel, evidence_satisfaction_channel};
use crate::planner::contract_attribute_repair;
use crate::planner::profile::is_nextjs_profile;
use crate::planner::verify::VerificationReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairTarget {
    DependencySetup,
    PackageConfig,
    FrameworkConfig,
    MissingEntrypoint,
    ContractAttributeMissing,
    EmptyApp,
    CapabilityMissing,
    RequiredEvidenceMissing,
    VerifierCommand,
    Implementation,
    TestOrEvidence,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairFollowThrough {
    NoChange,
    TargetMatched,
    TargetNotFollowed,
    UnrelatedChange,
}

impl RepairFollowThrough {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoChange => "no_change",
            Self::TargetMatched => "target_matched",
            Self::TargetNotFollowed => "target_not_followed",
            Self::UnrelatedChange => "unrelated_change",
        }
    }

    /// Telemetry label for follow-through outcomes.
    ///
    /// This classifier is heuristic and must not gate repair-loop termination;
    /// termination belongs to deterministic verification and repair-progress
    /// verdicts.
    pub fn failure_kind(self) -> Option<&'static str> {
        match self {
            Self::NoChange => Some("verify_repair_no_change"),
            Self::TargetNotFollowed => Some("repair_target_not_followed"),
            Self::UnrelatedChange => Some("repair_unrelated_change"),
            Self::TargetMatched => None,
        }
    }

    pub fn followed(self) -> bool {
        matches!(self, Self::TargetMatched)
    }
}

impl RepairTarget {
    pub fn reclassify_no_change(
        config: &Config,
        step_id: &str,
        report: &mut VerificationReport,
    ) -> bool {
        reclassify_dependency_no_change_at_root(
            &config.workspace_root,
            &config.profile,
            config.eval_events_path.as_deref(),
            step_id,
            report,
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DependencySetup => "dependency_setup",
            Self::PackageConfig => "package_config",
            Self::FrameworkConfig => "framework_config",
            Self::MissingEntrypoint => "missing_entrypoint",
            Self::ContractAttributeMissing => contract_attribute_repair::missing_kind(),
            Self::EmptyApp => "empty_app",
            Self::CapabilityMissing => "capability_missing",
            Self::RequiredEvidenceMissing => "required_evidence_missing",
            Self::VerifierCommand => "verifier_command",
            Self::Implementation => "implementation",
            Self::TestOrEvidence => "test_or_evidence",
            Self::Unknown => "unknown",
        }
    }

    pub fn guidance(self) -> &'static str {
        match self {
            Self::DependencySetup => {
                "Install or restore project dependencies before retrying build verification."
            }
            Self::PackageConfig => {
                "Fix dependency versions, scripts, and package metadata before editing app code."
            }
            Self::FrameworkConfig => {
                "Fix framework configuration, routing boundaries, or generated type declarations."
            }
            Self::MissingEntrypoint => {
                "Create or restore the executable entrypoint required by the selected profile."
            }
            Self::ContractAttributeMissing => {
                "Add the missing data-anvil contract attribute to the source file read by the verifier."
            }
            Self::EmptyApp => {
                "Replace metadata-only or static shell output with real application behavior."
            }
            Self::CapabilityMissing => {
                "Implement the missing user-facing capability required by the goal or contract."
            }
            Self::RequiredEvidenceMissing => {
                "Add deterministic source, test, or verification evidence for the requested behavior."
            }
            Self::VerifierCommand => {
                "The verify command is malformed; the artifact may already satisfy the requirement."
            }
            Self::Implementation => {
                "Fix the implementation files that should satisfy the requested behavior."
            }
            Self::TestOrEvidence => {
                "Add or strengthen deterministic evidence that verifies the requested behavior."
            }
            Self::Unknown => {
                "Inspect the verification reason and edit the smallest relevant file set."
            }
        }
    }

    pub fn allowed_action(self) -> &'static str {
        match self {
            Self::DependencySetup => "edit_setup_manifest_or_installable_dependency_artifact",
            Self::PackageConfig => "edit_package_manifest_or_lockfile",
            Self::FrameworkConfig => "edit_framework_configuration_or_route_boundary",
            Self::MissingEntrypoint => "create_missing_entrypoint_artifact",
            Self::ContractAttributeMissing => "edit_contract_attribute_source",
            Self::EmptyApp | Self::CapabilityMissing | Self::Implementation => {
                "edit_task_implementation_artifact"
            }
            Self::RequiredEvidenceMissing | Self::TestOrEvidence => {
                "edit_or_create_verification_evidence"
            }
            Self::VerifierCommand => "fix_malformed_verify_command_only",
            Self::Unknown => "edit_smallest_relevant_workspace_artifact",
        }
    }
}

fn reclassify_dependency_no_change_at_root(
    root: &Path,
    profile: &str,
    eval_events_path: Option<&Path>,
    step_id: &str,
    report: &mut VerificationReport,
) -> bool {
    if classify_repair_target(report) != RepairTarget::DependencySetup
        || !dependencies_ready_for_reclassification(root, profile)
    {
        return false;
    }
    let reason = report.dependency_missing.join("\n");
    let errors = build_verifier::internal_module_compile_errors(root, profile, &reason);
    if errors.is_empty() {
        return false;
    }
    for error in &errors {
        if !report.compile_errors.contains(error) {
            report.compile_errors.push(error.clone());
        }
    }
    crate::eval_events::emit(
        eval_events_path,
        serde_json::json!({
            "event": "repair_target_reclassified",
            "step_id": step_id,
            "previous_repair_target": RepairTarget::DependencySetup.as_str(),
            "repair_target": RepairTarget::Implementation.as_str(),
            "reason": "dependency_ready_internal_module_no_change",
            "compile_errors": errors,
        }),
    );
    true
}

fn dependencies_ready_for_reclassification(root: &Path, profile: &str) -> bool {
    if is_nextjs_profile(profile) {
        dependency_setup::next_build_dependencies_ready(root)
    } else if crate::planner::profile::canonical_profile_name(profile)
        == crate::planner::profile_descriptor::PYTHON_CLI_PROFILE_ID
    {
        dependency_setup::python_cli_dependencies_ready(root)
    } else {
        dependency_setup::node_declared_dependencies_ready(root)
    }
}

pub fn classify_repair_target(report: &VerificationReport) -> RepairTarget {
    if !report.verifier_command_false_negatives.is_empty() {
        return RepairTarget::VerifierCommand;
    }
    if contract_attribute_repair::is_contract_attribute_missing(report) {
        return RepairTarget::ContractAttributeMissing;
    }
    if !report.compile_errors.is_empty() {
        return RepairTarget::Implementation;
    }
    if !report.dependency_missing.is_empty() {
        return RepairTarget::DependencySetup;
    }
    if report.profile_failures.iter().any(|reason| {
        contains_any(
            reason,
            &[
                "probe_dependency_missing",
                "probe_infrastructure_failed",
                "app interaction untested",
            ],
        )
    }) {
        return RepairTarget::Unknown;
    }
    if report
        .profile_failures
        .iter()
        .any(|reason| app_behavior_probe_failure(reason))
    {
        return RepairTarget::Implementation;
    }
    if report.profile_failures.iter().any(|reason| {
        contains_any(
            reason,
            &[
                "package.json",
                "dependency",
                "dependencies",
                "devdependencies",
                "scripts.build",
                "scripts.dev",
                "react",
                "typescript",
                "next 15",
                "build-safe",
            ],
        )
    }) {
        return RepairTarget::PackageConfig;
    }
    if report.profile_failures.iter().any(|reason| {
        contains_any(
            reason,
            &[
                "tsconfig",
                "tailwind",
                "tailwind_dev_pipeline_failure",
                "css",
                "http_500",
                "layout",
                "use client",
                "global.d.ts",
                "module resolution",
                "app router",
                "alias",
                "declare module",
            ],
        )
    }) {
        return RepairTarget::FrameworkConfig;
    }
    if report.profile_failures.iter().any(|reason| {
        contains_any(
            reason,
            &[
                "entrypoint missing",
                "next entrypoint missing",
                "missing entrypoint",
                "src/app/page",
                "pages/index",
            ],
        )
    }) || report
        .missing_paths
        .iter()
        .any(|path| contains_any(path, &["src/app/page", "app/page", "pages/index"]))
    {
        return RepairTarget::MissingEntrypoint;
    }
    if report.profile_failures.iter().any(|reason| {
        contains_any(
            reason,
            &[
                "empty app",
                "metadata-only",
                "static shell",
                "static_title_only",
                "build-only",
            ],
        )
    }) {
        return RepairTarget::EmptyApp;
    }
    if report
        .profile_failures
        .iter()
        .any(|reason| missing_required_evidence_includes_behavior_depth_key(reason))
    {
        return RepairTarget::Implementation;
    }
    if report
        .profile_failures
        .iter()
        .any(|reason| missing_required_evidence_includes_source_scan(reason))
    {
        return RepairTarget::CapabilityMissing;
    }
    if report.profile_failures.iter().any(|reason| {
        contains_any(
            reason,
            &[
                "interactive_ui_source_evidence",
                "non_static_screen_evidence",
                "browser_interaction_failed",
                "missing_required_capabilities",
                "plan_output_missing_required_capabilities",
                "capability missing",
            ],
        )
    }) {
        return RepairTarget::CapabilityMissing;
    }
    if report.profile_failures.iter().any(|reason| {
        contains_any(
            reason,
            &[
                "missing_required_evidence",
                "weak_verification_evidence",
                "inconclusive_acceptance",
                "required evidence missing",
                "release gate partial",
                "browser_readiness_missing",
                "browser_readiness_evidence_missing",
                "browser_readiness_or_interaction_evidence_required",
            ],
        )
    }) {
        return RepairTarget::RequiredEvidenceMissing;
    }
    if report.profile_failures.iter().any(|reason| {
        contains_any(
            reason,
            &[
                "required_capabilities",
                "non_zero_test",
                "test_artifact",
                "bound_verify_command",
                "release gate failed",
                "browser_readiness_failed",
            ],
        )
    }) {
        return RepairTarget::TestOrEvidence;
    }
    if report.command_failures.iter().any(|failure| {
        contains_any(
            &format!("{} {}", failure.command, failure.reason),
            &["src/app/page", "app/page", "pages/index", "src/pages/index"],
        )
    }) {
        return RepairTarget::MissingEntrypoint;
    }
    if report.command_failures.iter().any(|failure| {
        contains_any(
            &format!("{} {}", failure.command, failure.reason),
            &[
                "no tests ran",
                "ran 0 tests",
                "test_discovery_failure",
                "test_framework_mismatch",
                "non_zero_test",
            ],
        )
    }) {
        return RepairTarget::TestOrEvidence;
    }
    if report.command_failures.iter().any(|failure| {
        contains_any(
            &format!("{} {}", failure.command, failure.reason),
            &[
                "npm run build",
                "next build",
                "cargo build",
                "compile",
                "syntax",
                "type error",
                "tsc",
            ],
        )
    }) {
        return RepairTarget::Implementation;
    }
    if report.missing_paths.iter().any(|path| {
        contains_any(
            path,
            &[
                "package.json",
                "tsconfig",
                "next.config",
                "tailwind",
                "global.d.ts",
                "layout.tsx",
            ],
        )
    }) {
        return RepairTarget::FrameworkConfig;
    }
    if !report.missing_paths.is_empty()
        || !report.command_failures.is_empty()
        || !report.profile_failures.is_empty()
    {
        return RepairTarget::Implementation;
    }
    RepairTarget::Unknown
}

pub fn repair_target_matches_changed_path(target: RepairTarget, path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    match target {
        RepairTarget::DependencySetup => {
            lower == "package.json"
                || lower == "package-lock.json"
                || lower == "pnpm-lock.yaml"
                || lower == "yarn.lock"
                || lower == "cargo.toml"
                || lower == "cargo.lock"
                || lower == "pyproject.toml"
                || lower == "requirements.txt"
                || lower.ends_with("/package.json")
                || lower.ends_with("/package-lock.json")
                || lower.ends_with("/pnpm-lock.yaml")
                || lower.ends_with("/yarn.lock")
                || lower.ends_with("/cargo.toml")
                || lower.ends_with("/cargo.lock")
                || lower.ends_with("/pyproject.toml")
                || lower.ends_with("/requirements.txt")
                || lower.contains("node_modules/.bin/")
        }
        RepairTarget::PackageConfig => {
            lower.ends_with("package.json") || lower.ends_with("package-lock.json")
        }
        RepairTarget::FrameworkConfig => contains_any(
            &lower,
            &[
                "tsconfig",
                "next.config",
                "tailwind",
                "postcss",
                "global.d.ts",
                "globals.css",
                "layout.tsx",
                "layout.jsx",
            ],
        ),
        RepairTarget::MissingEntrypoint => contains_any(
            &lower,
            &[
                "src/app/page",
                "app/page",
                "pages/index",
                "src/pages/index",
                "main.rs",
                "lib.rs",
                "main.py",
                "app.py",
            ],
        ),
        RepairTarget::ContractAttributeMissing => {
            contains_any(
                &lower,
                &["src/", "app/", "pages/", ".ts", ".tsx", ".js", ".jsx"],
            ) && !lower.ends_with("package.json")
        }
        RepairTarget::EmptyApp | RepairTarget::CapabilityMissing => {
            contains_any(
                &lower,
                &[
                    "src/", "app/", "pages/", ".rs", ".py", ".ts", ".tsx", ".js", ".jsx",
                ],
            ) && !lower.ends_with("package.json")
        }
        RepairTarget::RequiredEvidenceMissing => contains_any(
            &lower,
            &[
                "test",
                "spec",
                "__tests__",
                ".snap",
                "evidence",
                "README",
                "readme",
            ],
        ),
        RepairTarget::VerifierCommand => false,
        RepairTarget::Implementation => {
            contains_any(
                &lower,
                &[
                    "src/", "app/", "pages/", ".rs", ".py", ".ts", ".tsx", ".js", ".jsx",
                ],
            ) && !lower.ends_with("package.json")
        }
        RepairTarget::TestOrEvidence => contains_any(
            &lower,
            &[
                "test",
                "spec",
                "__tests__",
                ".snap",
                "evidence",
                "README",
                "readme",
            ],
        ),
        RepairTarget::Unknown => true,
    }
}

pub fn classify_repair_follow_through(
    target: RepairTarget,
    changed_paths: &[String],
) -> RepairFollowThrough {
    if changed_paths.is_empty() {
        return RepairFollowThrough::NoChange;
    }
    if changed_paths
        .iter()
        .any(|path| repair_target_matches_changed_path(target, path))
    {
        RepairFollowThrough::TargetMatched
    } else if changed_paths
        .iter()
        .any(|path| repair_change_is_related_to_task_artifact(path))
    {
        RepairFollowThrough::TargetNotFollowed
    } else {
        RepairFollowThrough::UnrelatedChange
    }
}

pub fn repair_target_followed(target: RepairTarget, changed_paths: &[String]) -> bool {
    classify_repair_follow_through(target, changed_paths).followed()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let lower = value.to_ascii_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

fn missing_required_evidence_includes_source_scan(reason: &str) -> bool {
    missing_required_evidence_keys(reason)
        .into_iter()
        .any(|key| evidence_satisfaction_channel(key) == SatisfactionChannel::SourceScan)
}

fn missing_required_evidence_includes_behavior_depth_key(reason: &str) -> bool {
    missing_required_evidence_keys(reason)
        .into_iter()
        .any(|key| {
            matches!(
                key,
                "challenge_or_adversary_evidence"
                    | "failure_or_collision_evidence"
                    | "score_or_progression_evidence"
                    | "restart_or_recoverable_state_evidence"
            )
        })
}

fn app_behavior_probe_failure(reason: &str) -> bool {
    contains_any(
        reason,
        &[
            "browser_interaction_failed:interaction_state_change_missing",
            "browser_interaction_failed:input_state_change_missing_after_start",
            "browser_interaction_failed:input_state_change_not_evaluated_after_start",
            "browser_interaction_failed:start_transition_missing",
            "browser_interaction_failed:primary_start_transition_missing",
            "browser_interaction_failed:app_route_unstable",
            "browser_interaction_failed:surface_missing",
            "browser_interaction_failed:surface_visible_missing",
            "browser_interaction_failed:interactive_surface_missing",
            "browser_interaction_failed:canvas_unavailable",
            "interaction evidence status: failed:interaction_state_change_missing",
            "interaction evidence status: failed:input_state_change_missing_after_start",
            "interaction evidence status: failed:input_state_change_not_evaluated_after_start",
            "interaction evidence status: failed:start_transition_missing",
            "interaction evidence status: failed:primary_start_transition_missing",
            "interaction evidence status: failed:app_route_unstable",
            "interaction evidence status: failed:surface_missing",
            "interaction evidence status: failed:surface_visible_missing",
            "interaction evidence status: failed:interactive_surface_missing",
            "interaction evidence status: failed:canvas_unavailable",
        ],
    )
}

fn missing_required_evidence_keys(reason: &str) -> Vec<&str> {
    let Some((_, rest)) = reason.split_once("missing_required_evidence:") else {
        return Vec::new();
    };
    rest.split(',')
        .map(|key| {
            key.trim()
                .trim_matches(|ch: char| matches!(ch, '.' | ';' | ')' | ']'))
        })
        .filter(|key| !key.is_empty())
        .collect()
}

fn repair_change_is_related_to_task_artifact(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if lower.starts_with(".commandagent/")
        || lower.starts_with(".anvil/")
        || lower.starts_with(".git/")
        || lower.starts_with("docs/")
        || lower.ends_with(".md")
    {
        return false;
    }
    contains_any(
        &lower,
        &[
            "src/",
            "app/",
            "pages/",
            "tests/",
            "__tests__/",
            "test",
            "spec",
            "package.json",
            "package-lock.json",
            "pnpm-lock.yaml",
            "yarn.lock",
            "cargo.toml",
            "cargo.lock",
            "pyproject.toml",
            "requirements.txt",
            "tsconfig",
            "next.config",
            "tailwind",
            "postcss",
            ".rs",
            ".py",
            ".ts",
            ".tsx",
            ".js",
            ".jsx",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::verify::VerificationReport;

    #[test]
    fn dependency_missing_targets_setup() {
        let mut report = VerificationReport::pass();
        report.push_dependency_missing("node_modules/.bin/next missing");
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::DependencySetup
        );
    }

    #[test]
    fn dependency_ready_internal_module_no_change_reclassifies_to_implementation() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/.bin")).unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/next")).unwrap();
        std::fs::write(dir.path().join("node_modules/.bin/next"), "").unwrap();
        std::fs::write(
            dir.path().join("src/app/page.tsx"),
            "import { readTasks } from '@/lib/tasks';",
        )
        .unwrap();
        let events = dir.path().join("events.jsonl");
        let mut report = VerificationReport::dependency_missing(
            "dependency_setup_missing: command failed: npm run build summary: Failed to compile. Module not found: Can't resolve '@/lib/tasks'",
        );

        assert!(reclassify_dependency_no_change_at_root(
            dir.path(),
            "nextjs",
            Some(&events),
            "implement-app",
            &mut report,
        ));
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::Implementation
        );
        let event = std::fs::read_to_string(events).unwrap();
        assert!(event.contains("\"event\":\"repair_target_reclassified\""));
        assert!(event.contains("dependency_ready_internal_module_no_change"));

        let mut external = VerificationReport::dependency_missing(
            "Failed to compile.\n./src/app/page.tsx\nModule not found: Can't resolve 'react'",
        );
        assert!(!reclassify_dependency_no_change_at_root(
            dir.path(),
            "nextjs",
            None,
            "implement-app",
            &mut external,
        ));
        assert_eq!(
            classify_repair_target(&external),
            RepairTarget::DependencySetup
        );
    }

    #[test]
    fn package_failures_target_package_config() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure("scripts.build must run next build");
        assert_eq!(classify_repair_target(&report), RepairTarget::PackageConfig);
    }

    #[test]
    fn evidence_failures_target_required_evidence() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure("missing_required_evidence:non_zero_test");
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::RequiredEvidenceMissing
        );
    }

    #[test]
    fn release_gate_missing_browser_evidence_targets_required_evidence() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure(
            "release gate partial: browser_readiness_or_interaction_evidence_required:browser_readiness_evidence_missing",
        );
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::RequiredEvidenceMissing
        );
    }

    #[test]
    fn tailwind_dev_route_failure_targets_framework_config() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure(
            "release gate failed: browser_readiness_failed:tailwind_dev_pipeline_failure",
        );
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::FrameworkConfig
        );
    }

    #[test]
    fn browser_http_500_route_failure_targets_framework_config() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure("release gate failed: browser_readiness_failed:http_500");
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::FrameworkConfig
        );
    }

    #[test]
    fn browser_route_failure_targets_test_or_evidence() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure("release gate failed: browser_readiness_failed:route_render");
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::TestOrEvidence
        );
    }

    #[test]
    fn browser_interaction_probe_failure_targets_capability_implementation() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure(
            "release gate failed: browser_interaction_failed:start_transition_missing",
        );
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::Implementation
        );
        assert_eq!(
            classify_repair_target(&report).allowed_action(),
            "edit_task_implementation_artifact"
        );
    }

    #[test]
    fn browser_interaction_probe_infrastructure_failure_is_not_app_repair_target() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure(
            "release gate failed: probe_dependency_missing:playwright_module_missing; app interaction untested"
                .to_string(),
        );

        assert_eq!(classify_repair_target(&report), RepairTarget::Unknown);
    }

    #[test]
    fn repair_target_followed_accepts_package_config_paths() {
        assert!(repair_target_followed(
            RepairTarget::PackageConfig,
            &["package.json".to_string()]
        ));
        assert!(!repair_target_followed(
            RepairTarget::PackageConfig,
            &["src/app/page.tsx".to_string()]
        ));
    }

    #[test]
    fn repair_follow_through_distinguishes_no_change() {
        assert_eq!(
            classify_repair_follow_through(RepairTarget::MissingEntrypoint, &[]),
            RepairFollowThrough::NoChange
        );
        assert!(!repair_target_followed(
            RepairTarget::MissingEntrypoint,
            &[]
        ));
    }

    #[test]
    fn repair_follow_through_distinguishes_target_not_followed() {
        assert_eq!(
            classify_repair_follow_through(
                RepairTarget::MissingEntrypoint,
                &["src/app/widget.tsx".to_string()]
            ),
            RepairFollowThrough::TargetNotFollowed
        );
    }

    #[test]
    fn repair_follow_through_distinguishes_unrelated_change() {
        assert_eq!(
            classify_repair_follow_through(
                RepairTarget::MissingEntrypoint,
                &["README.md".to_string()]
            ),
            RepairFollowThrough::UnrelatedChange
        );
    }

    #[test]
    fn repair_follow_through_accepts_missing_entrypoint_artifact() {
        assert_eq!(
            classify_repair_follow_through(
                RepairTarget::MissingEntrypoint,
                &["src/app/page.tsx".to_string()]
            ),
            RepairFollowThrough::TargetMatched
        );
    }

    #[test]
    fn dependency_setup_target_accepts_manifest_or_lockfile_changes() {
        assert!(repair_target_followed(
            RepairTarget::DependencySetup,
            &["package.json".to_string()]
        ));
        assert!(repair_target_followed(
            RepairTarget::DependencySetup,
            &["package-lock.json".to_string()]
        ));
    }

    #[test]
    fn classifies_missing_entrypoint() {
        let report = VerificationReport::missing_path("src/app/page.tsx");
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::MissingEntrypoint
        );
    }

    #[test]
    fn classifies_contract_attribute_missing_before_missing_entrypoint() {
        let mut report = VerificationReport::pass();
        report.push_command_failure(
            r#"node -p 'String(require("fs").readFileSync("src/app/page.tsx")).includes("data-anvil-state") ? true : process.exit(1)'"#,
            "command failed",
        );

        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::ContractAttributeMissing
        );
        assert_eq!(
            RepairTarget::ContractAttributeMissing.as_str(),
            "contract_attribute_missing"
        );
        assert!(repair_target_followed(
            RepairTarget::ContractAttributeMissing,
            &["src/app/page.tsx".to_string()]
        ));
    }

    #[test]
    fn classifies_missing_implementation_obligation_target_as_entrypoint() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure(
            "missing_required_obligation_target:implementation:src/app/page.tsx",
        );
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::MissingEntrypoint
        );
    }

    #[test]
    fn classifies_capability_missing() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure("missing_required_capabilities:player_control");
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::CapabilityMissing
        );
    }

    #[test]
    fn classifies_interactive_source_evidence_as_capability_missing() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure("missing_required_evidence:interactive_ui_source_evidence");
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::CapabilityMissing
        );
    }

    #[test]
    fn source_scanned_missing_gameplay_evidence_targets_capability_implementation() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure(
            "missing_required_evidence:failure_or_collision_evidence,restart_or_recoverable_state_evidence",
        );
        let target = classify_repair_target(&report);
        assert_eq!(target, RepairTarget::Implementation);
        assert_eq!(
            classify_repair_follow_through(target, &["src/app/page.tsx".to_string()]),
            RepairFollowThrough::TargetMatched
        );
    }

    #[test]
    fn test_artifact_only_missing_evidence_keeps_evidence_target() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure("missing_required_evidence:test_artifact,bound_verify_command");
        let target = classify_repair_target(&report);
        assert_eq!(target, RepairTarget::RequiredEvidenceMissing);
        assert_eq!(
            classify_repair_follow_through(target, &["src/app/page.tsx".to_string()]),
            RepairFollowThrough::TargetNotFollowed
        );
    }

    #[test]
    fn repair_target_followed_accepts_framework_config_paths() {
        assert!(repair_target_followed(
            RepairTarget::FrameworkConfig,
            &["src/app/global.d.ts".to_string()]
        ));
    }
}
