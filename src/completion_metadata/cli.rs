use std::path::Path;

use serde::Deserialize;

use crate::eval_events::{CompletionProjection, CompletionSnapshot};
use crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN;
#[cfg(test)]
use crate::planner::profile::canonical_profile_name;
use crate::planner::profiles::python_cli::runtime::{
    CliAssurance, CliCheckSummary, EVIDENCE_PATH, classify,
};

const CLI_PROBE_NOT_RUN: &str = "cli_probe_not_run";
const CLI_CLAIMS_ABSENT: &str = "cli_claims_absent";
const CLI_ASSURANCE_FAILED: &str = "cli_assurance_failed";
const FALLBACK_BEHAVIOR_EVIDENCE_PATH: &str = ".commandagent/evidence/python-cli-behavior.json";
const LEGACY_FALLBACK_BEHAVIOR_EVIDENCE_PATH: &str = ".anvil/evidence/python-cli-behavior.json";

#[derive(Deserialize)]
struct FallbackBehaviorEvidence {
    profile: String,
    status: String,
    ok: bool,
    reasons: Vec<String>,
    details: FallbackBehaviorDetails,
}

#[derive(Deserialize)]
struct FallbackBehaviorDetails {
    entrypoint: String,
    first_exit_code: Option<i32>,
    second_exit_code: Option<i32>,
    first_stdout: String,
    second_stdout: String,
    changed_by_input: bool,
}

pub(crate) fn apply_snapshot_runtime(root: &Path, snapshot: &mut CompletionSnapshot) {
    let (assurance, reason) = terminal_completion_assurance(
        root,
        &snapshot.assurance_level,
        &snapshot.runtime_acceptance_status,
        &snapshot.final_acceptance_status,
        &snapshot.release_gate_status,
        &snapshot.profile_behavior_probe_status,
        &snapshot.profile_behavior_probe_evidence_path,
    );
    snapshot.assurance_level = assurance.as_str().to_string();
    snapshot.assurance_reason = reason.to_string();
}

pub(crate) fn apply_terminal_projection_runtime(
    root: &Path,
    projection: &mut CompletionProjection,
) {
    if projection.contract_origin == FIX_CONTRACT_ORIGIN {
        return;
    }
    let (assurance, reason) = terminal_completion_assurance(
        root,
        &projection.assurance_level,
        &projection.runtime_acceptance,
        &projection.final_acceptance,
        &projection.release_gate,
        &projection.profile_behavior_probe_status,
        &projection.profile_behavior_probe_evidence_path,
    );
    projection.assurance_level = assurance.as_str().to_string();
    projection.assurance_reason = reason.to_string();
}

#[cfg(test)]
fn is_cli_profile(profile: &str) -> bool {
    matches!(
        canonical_profile_name(profile).as_str(),
        "python-cli" | "cli"
    )
}

#[cfg(test)]
pub(crate) fn apply_snapshot(root: &Path, snapshot: &mut CompletionSnapshot) -> bool {
    if !is_cli_profile(&snapshot.effective_profile) {
        return false;
    }
    apply_snapshot_runtime(root, snapshot);
    true
}

#[cfg(test)]
pub(crate) fn apply_terminal_projection(root: &Path, projection: &mut CompletionProjection) {
    if !is_cli_profile(&projection.effective_profile) {
        return;
    }
    apply_terminal_projection_runtime(root, projection);
}

pub(crate) fn completion_assurance(root: &Path) -> (CliAssurance, &'static str) {
    let assurance = std::fs::read(root.join(EVIDENCE_PATH))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<CliCheckSummary>(&bytes).ok())
        .map(|summary| classify(&summary.evidence))
        .unwrap_or(CliAssurance::Static);
    (assurance, assurance_reason(assurance))
}

fn terminal_completion_assurance(
    root: &Path,
    assurance_level: &str,
    runtime_acceptance: &str,
    final_acceptance: &str,
    release_gate: &str,
    profile_behavior_probe_status: &str,
    profile_behavior_probe_evidence_path: &str,
) -> (CliAssurance, &'static str) {
    let canonical_path = root.join(EVIDENCE_PATH);
    let (canonical, reason) = completion_assurance(root);
    if canonical == CliAssurance::Static
        && !canonical_path.exists()
        && assurance_level == "full"
        && runtime_acceptance == "pass"
        && final_acceptance == "full_success"
        && release_gate == "pass"
        && profile_behavior_probe_status == "pass"
        && (profile_behavior_probe_evidence_path == FALLBACK_BEHAVIOR_EVIDENCE_PATH
            || profile_behavior_probe_evidence_path == LEGACY_FALLBACK_BEHAVIOR_EVIDENCE_PATH)
        && passing_fallback_behavior_evidence(root)
    {
        return (CliAssurance::Full, "");
    }
    (canonical, reason)
}

fn passing_fallback_behavior_evidence(root: &Path) -> bool {
    let evidence = std::fs::read(root.join(FALLBACK_BEHAVIOR_EVIDENCE_PATH))
        .or_else(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                std::fs::read(root.join(LEGACY_FALLBACK_BEHAVIOR_EVIDENCE_PATH))
            } else {
                Err(error)
            }
        })
        .ok()
        .and_then(|bytes| serde_json::from_slice::<FallbackBehaviorEvidence>(&bytes).ok());
    let Some(evidence) = evidence else {
        return false;
    };
    evidence.profile == "python-cli"
        && evidence.status == "pass"
        && evidence.ok
        && evidence.reasons.is_empty()
        && evidence.details.first_exit_code == Some(0)
        && evidence.details.second_exit_code == Some(0)
        && !evidence.details.first_stdout.trim().is_empty()
        && !evidence.details.second_stdout.trim().is_empty()
        && evidence.details.first_stdout != evidence.details.second_stdout
        && evidence.details.changed_by_input
        && fallback_entrypoint_is_bound(root, &evidence.details.entrypoint)
}

fn fallback_entrypoint_is_bound(root: &Path, entrypoint: &str) -> bool {
    let entrypoint = Path::new(entrypoint);
    let entrypoint = if entrypoint.is_absolute() {
        entrypoint.to_path_buf()
    } else {
        root.join(entrypoint)
    };
    let Ok(relative) = entrypoint.strip_prefix(root) else {
        return false;
    };
    let parts = relative
        .iter()
        .filter_map(|part| part.to_str())
        .collect::<Vec<_>>();
    entrypoint.is_file()
        && parts.len() == 3
        && parts[0] == "src"
        && !parts[1].is_empty()
        && parts[2] == "main.py"
}

fn assurance_reason(assurance: CliAssurance) -> &'static str {
    match assurance {
        CliAssurance::Full => "",
        CliAssurance::Partial => CLI_CLAIMS_ABSENT,
        CliAssurance::Static => CLI_PROBE_NOT_RUN,
        CliAssurance::Failed => CLI_ASSURANCE_FAILED,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::cli::Cli;
    use crate::config::Config;
    use crate::eval_events::{latest_completion_snapshot, project_completion};
    use crate::planner::profiles::python_cli::runtime::{
        C1, C2, C3, C4, CheckStatus, EvidenceState,
    };
    use clap::Parser;
    use serde_json::{Value, json};

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/test0725_cli_completion_projection/fixtures/projections.jsonl"
    );
    const LUNA_CLAIMS_ABSENT_FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/test0725_cli_completion_projection/fixtures/stats_luna_002-cli-assurance.json"
    );

    #[test]
    fn measured_unexecuted_shapes_project_static_at_both_boundaries() {
        for line in std::fs::read_to_string(FIXTURE).unwrap().lines() {
            let fixture: Value = serde_json::from_str(line).unwrap();
            let name = fixture["fixture"].as_str().unwrap();
            let root = tempfile::tempdir().unwrap();
            let events = root.path().join("events.jsonl");
            std::fs::write(&events, format!("{line}\n")).unwrap();
            let config = cli_config(root.path(), events.clone());
            let mut snapshot = latest_completion_snapshot(Some(&events));

            crate::completion_metadata::apply_config_completion_metadata(&config, &mut snapshot);
            assert_eq!(snapshot.assurance_level, "static", "{name}");
            assert_eq!(snapshot.assurance_reason, CLI_PROBE_NOT_RUN, "{name}");

            let mut projection = project_completion(false, &snapshot);
            crate::completion_metadata::apply_config_completion_projection(
                &config,
                &mut projection,
            );
            assert_eq!(projection.assurance_level, "static", "{name}");
            assert_eq!(projection.assurance_reason, CLI_PROBE_NOT_RUN, "{name}");
        }
    }

    #[test]
    fn contract_assurance_mapping_is_rederived_from_c1_through_c4() {
        let cases = [
            (
                "not-executed",
                state(false, true, CheckStatus::NotExecuted),
                "static",
                CLI_PROBE_NOT_RUN,
            ),
            (
                "polarity-violation",
                state(true, true, CheckStatus::Failed),
                "failed",
                CLI_ASSURANCE_FAILED,
            ),
            (
                "binding-violation",
                state(true, false, CheckStatus::Pass),
                "failed",
                CLI_ASSURANCE_FAILED,
            ),
            (
                "c3-violation-after-other-checks-pass",
                checks_state(
                    CheckStatus::Pass,
                    CheckStatus::Pass,
                    CheckStatus::Failed,
                    CheckStatus::Pass,
                ),
                "failed",
                CLI_ASSURANCE_FAILED,
            ),
            (
                "c4-rerun-violation-after-other-checks-pass",
                checks_state(
                    CheckStatus::Pass,
                    CheckStatus::Pass,
                    CheckStatus::Pass,
                    CheckStatus::Failed,
                ),
                "failed",
                CLI_ASSURANCE_FAILED,
            ),
            (
                "claims-absent",
                claims_absent_state(),
                "partial",
                CLI_CLAIMS_ABSENT,
            ),
            ("all-pass", state(true, true, CheckStatus::Pass), "full", ""),
        ];
        for (name, evidence, expected_level, expected_reason) in cases {
            let root = tempfile::tempdir().unwrap();
            write_evidence(root.path(), evidence);
            let mut snapshot = cli_snapshot();

            apply_snapshot(root.path(), &mut snapshot);
            let mut projection = project_completion(true, &snapshot);
            apply_terminal_projection(root.path(), &mut projection);

            assert_eq!(projection.assurance_level, expected_level, "{name}");
            assert_eq!(projection.assurance_reason, expected_reason, "{name}");
        }
    }

    #[test]
    fn passing_src_package_behavior_evidence_preserves_earned_full_assurance() {
        let root = tempfile::tempdir().unwrap();
        write_src_entrypoint(root.path());
        write_fallback_behavior(root.path(), passing_fallback_behavior(root.path()));
        let mut snapshot = cli_snapshot();

        apply_snapshot(root.path(), &mut snapshot);

        assert_eq!(snapshot.assurance_level, "full");
        assert_eq!(snapshot.assurance_reason, "");
        let mut projection = project_completion(true, &snapshot);
        apply_terminal_projection(root.path(), &mut projection);
        assert_eq!(projection.assurance_level, "full");
        assert_eq!(projection.assurance_reason, "");
    }

    #[test]
    fn nonpassing_fallback_behavior_evidence_never_elevates_assurance() {
        for name in ["missing", "failed", "malformed", "unexecuted"] {
            let root = tempfile::tempdir().unwrap();
            write_src_entrypoint(root.path());
            match name {
                "missing" => {}
                "failed" => {
                    let mut evidence = passing_fallback_behavior(root.path());
                    evidence["status"] = json!("failed");
                    evidence["ok"] = json!(false);
                    evidence["reasons"] = json!(["python_cli_behavior_probe_failed"]);
                    write_fallback_behavior(root.path(), evidence);
                }
                "malformed" => {
                    let path = root.path().join(FALLBACK_BEHAVIOR_EVIDENCE_PATH);
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(path, b"{not-json").unwrap();
                }
                "unexecuted" => {
                    let mut evidence = passing_fallback_behavior(root.path());
                    evidence["details"]["first_exit_code"] = Value::Null;
                    evidence["details"]["first_stdout"] = json!("");
                    evidence["details"]["changed_by_input"] = json!(false);
                    write_fallback_behavior(root.path(), evidence);
                }
                _ => unreachable!(),
            }
            let mut snapshot = cli_snapshot();

            apply_snapshot(root.path(), &mut snapshot);

            assert_eq!(snapshot.assurance_level, "static", "{name}");
            assert_eq!(snapshot.assurance_reason, CLI_PROBE_NOT_RUN, "{name}");
            let mut projection = project_completion(true, &snapshot);
            apply_terminal_projection(root.path(), &mut projection);
            assert_eq!(projection.assurance_level, "static", "{name}");
            assert_eq!(projection.assurance_reason, CLI_PROBE_NOT_RUN, "{name}");
        }
    }

    #[test]
    fn passing_fallback_evidence_cannot_override_failed_current_gates() {
        let root = tempfile::tempdir().unwrap();
        write_src_entrypoint(root.path());
        write_fallback_behavior(root.path(), passing_fallback_behavior(root.path()));
        let mut snapshot = cli_snapshot();
        snapshot.runtime_acceptance_status = "failed".into();
        snapshot.final_acceptance_status = "incomplete".into();
        snapshot.release_gate_status = "failed".into();

        apply_snapshot(root.path(), &mut snapshot);

        assert_eq!(snapshot.assurance_level, "static");
        assert_eq!(snapshot.assurance_reason, CLI_PROBE_NOT_RUN);
    }

    #[test]
    fn passing_fallback_evidence_requires_current_probe_binding() {
        for (name, status, evidence_path) in [
            ("unexecuted", "not_applicable", ""),
            ("failed", "failed", FALLBACK_BEHAVIOR_EVIDENCE_PATH),
            ("wrong-path", "pass", "evidence/other.json"),
        ] {
            let root = tempfile::tempdir().unwrap();
            write_src_entrypoint(root.path());
            write_fallback_behavior(root.path(), passing_fallback_behavior(root.path()));
            let mut snapshot = cli_snapshot();
            snapshot.profile_behavior_probe_status = status.into();
            snapshot.profile_behavior_probe_evidence_path = evidence_path.into();

            apply_snapshot(root.path(), &mut snapshot);

            assert_eq!(snapshot.assurance_level, "static", "{name}");
            assert_eq!(snapshot.assurance_reason, CLI_PROBE_NOT_RUN, "{name}");
        }
    }

    #[test]
    fn measured_luna_c1_c2_c4_pass_and_c3_claims_absent_projects_partial() {
        let measured: CliCheckSummary =
            serde_json::from_str(&std::fs::read_to_string(LUNA_CLAIMS_ABSENT_FIXTURE).unwrap())
                .unwrap();
        assert_eq!(measured.assurance, CliAssurance::Failed);
        assert_eq!(measured.evidence.checks[C1], CheckStatus::Pass);
        assert_eq!(measured.evidence.checks[C2], CheckStatus::Pass);
        assert_eq!(measured.evidence.checks[C3], CheckStatus::ClaimsAbsent);
        assert_eq!(measured.evidence.checks[C4], CheckStatus::Pass);

        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("evidence")).unwrap();
        std::fs::copy(LUNA_CLAIMS_ABSENT_FIXTURE, root.path().join(EVIDENCE_PATH)).unwrap();
        let mut snapshot = cli_snapshot();

        apply_snapshot(root.path(), &mut snapshot);
        assert_eq!(snapshot.assurance_level, "partial");
        assert_eq!(snapshot.assurance_reason, CLI_CLAIMS_ABSENT);

        let mut projection = project_completion(false, &snapshot);
        apply_terminal_projection(root.path(), &mut projection);
        assert_eq!(projection.assurance_level, "partial");
        assert_eq!(projection.assurance_reason, CLI_CLAIMS_ABSENT);
    }

    #[test]
    fn non_cli_projection_is_byte_compatible() {
        for profile in ["generic", "data", "nextjs"] {
            let root = tempfile::tempdir().unwrap();
            let mut snapshot = CompletionSnapshot::empty();
            snapshot.profile = profile.into();
            snapshot.effective_profile = profile.into();
            snapshot.assurance_level = "partial".into();
            snapshot.assurance_reason = "existing_reason".into();
            let mut projection = project_completion(false, &snapshot);
            let expected = projection.clone();

            apply_terminal_projection(root.path(), &mut projection);

            assert_eq!(projection, expected, "{profile}");
        }
    }

    fn state(probe_attempted: bool, binding_intact: bool, status: CheckStatus) -> EvidenceState {
        EvidenceState {
            probe_attempted,
            binding_intact,
            checks: BTreeMap::from([
                (C1.to_string(), status),
                (C2.to_string(), status),
                (C3.to_string(), status),
                (C4.to_string(), status),
            ]),
        }
    }

    fn claims_absent_state() -> EvidenceState {
        checks_state(
            CheckStatus::Pass,
            CheckStatus::ClaimsAbsent,
            CheckStatus::ClaimsAbsent,
            CheckStatus::Pass,
        )
    }

    fn checks_state(
        c1: CheckStatus,
        c2: CheckStatus,
        c3: CheckStatus,
        c4: CheckStatus,
    ) -> EvidenceState {
        EvidenceState {
            probe_attempted: true,
            binding_intact: true,
            checks: BTreeMap::from([
                (C1.to_string(), c1),
                (C2.to_string(), c2),
                (C3.to_string(), c3),
                (C4.to_string(), c4),
            ]),
        }
    }

    fn write_evidence(root: &Path, evidence: EvidenceState) {
        std::fs::create_dir_all(root.join("evidence")).unwrap();
        let assurance = classify(&evidence);
        let summary = CliCheckSummary {
            status: assurance.as_str().into(),
            assurance,
            evidence,
            reasons: Vec::new(),
        };
        std::fs::write(
            root.join(EVIDENCE_PATH),
            serde_json::to_vec_pretty(&summary).unwrap(),
        )
        .unwrap();
    }

    fn write_src_entrypoint(root: &Path) {
        std::fs::create_dir_all(root.join("src/anvil_app")).unwrap();
        std::fs::write(root.join("src/anvil_app/main.py"), "print('ok')\n").unwrap();
    }

    fn write_fallback_behavior(root: &Path, evidence: Value) {
        let path = root.join(FALLBACK_BEHAVIOR_EVIDENCE_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
    }

    fn passing_fallback_behavior(root: &Path) -> Value {
        json!({
            "profile": "python-cli",
            "status": "pass",
            "ok": true,
            "reasons": [],
            "details": {
                "entrypoint": root.join("src/anvil_app/main.py"),
                "first_exit_code": 0,
                "second_exit_code": 0,
                "first_stdout": "Hello, anvil!",
                "second_stdout": "Hello, profile!",
                "changed_by_input": true
            }
        })
    }

    fn cli_config(root: &Path, events: std::path::PathBuf) -> Config {
        let mut config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            root.to_str().unwrap(),
            "--profile",
            "cli",
        ]))
        .unwrap();
        config.eval_events_path = Some(events);
        config
    }

    fn cli_snapshot() -> CompletionSnapshot {
        let mut snapshot = CompletionSnapshot::empty();
        snapshot.profile = "cli".into();
        snapshot.effective_profile = "cli".into();
        snapshot.assurance_level = "full".into();
        snapshot.runtime_acceptance_status = "pass".into();
        snapshot.final_acceptance_status = "full_success".into();
        snapshot.release_gate_status = "pass".into();
        snapshot.profile_behavior_probe_status = "pass".into();
        snapshot.profile_behavior_probe_evidence_path = FALLBACK_BEHAVIOR_EVIDENCE_PATH.into();
        snapshot.completion_contract_verification_enabled = true;
        snapshot
    }
}
