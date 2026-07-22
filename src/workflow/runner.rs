//! Deterministic earned-edge checks for workflow circles.
use super::schema::{Route, Verdict};
use crate::config::Provider;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeFailure {
    pub edge: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeEvidence {
    pub verdict: Verdict,
    pub evidence: bool,
    pub adjudicated: bool,
    pub epoch: u64,
    pub previous_epoch: u64,
    pub carry_present: bool,
}

pub fn edge_earned(route: &Route, edge: &str, evidence: &EdgeEvidence) -> Result<(), EdgeFailure> {
    if evidence.verdict != route.on {
        return Err(fail(edge, "verdict"));
    }
    if !evidence.evidence || !evidence.adjudicated {
        return Err(fail(edge, "evidence"));
    }
    if evidence.epoch <= evidence.previous_epoch {
        return Err(fail(edge, "epoch"));
    }
    if !evidence.carry_present {
        return Err(fail(edge, "carry"));
    }
    Ok(())
}

fn fail(edge: &str, reason: &str) -> EdgeFailure {
    EdgeFailure {
        edge: edge.into(),
        reason: format!("edge_not_earned:{edge}:{reason}"),
    }
}

pub fn origin_recovery_yaml_present(origin: &Path) -> bool {
    fs::read_dir(origin.join(".anvil/plans"))
        .map(|entries| {
            entries.flatten().any(|e| {
                e.file_name().to_string_lossy().starts_with("recovery-")
                    && e.path().extension().is_some_and(|x| x == "yaml")
            })
        })
        .unwrap_or(false)
}

pub fn latest_failed_run_events(origin: &Path) -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    let runs = origin.join(".anvil/runs");
    for entry in fs::read_dir(runs).ok()?.flatten() {
        let path = entry.path().join("events.jsonl");
        if path.is_file() {
            candidates.push(path);
        }
    }
    candidates.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    candidates.into_iter().rev().find(|p| {
        fs::read_to_string(p)
            .map(|s| {
                s.lines().any(|l| {
                    l.contains("\"event\":\"run_stop\"") && l.contains("\"status\":\"failed\"")
                })
            })
            .unwrap_or(false)
    })
}

pub fn derive_goal(intent: &str, origin_goal: &str) -> Option<String> {
    match intent {
        "investigate" => Some(format!(
            "『{origin_goal}』の実行が失敗しました。まず output/diagnosis.md を作成し、調査の進展に応じて更新すること。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。"
        )),
        "fix" => Some(format!(
            "『{origin_goal}』の実行が失敗し、原因調査が完了しています。診断（output/diagnosis.md）と再現手順に基づき修正してください。修正後も既存の検証が通ることを確認してください。"
        )),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "event")]
pub enum WorkflowEvent {
    #[serde(rename = "workflow_started")]
    Started,
    #[serde(rename = "workflow_edge_fired")]
    EdgeFired { edge: String, checks: Vec<String> },
    #[serde(rename = "workflow_node_completed")]
    NodeCompleted { node: String },
    #[serde(rename = "workflow_adjudicated")]
    Adjudicated {
        verdict: String,
        reason: Option<String>,
    },
}

pub fn circle_adjudication(
    verify_origin_passed: bool,
    reason: Option<String>,
) -> (&'static str, Option<String>) {
    if verify_origin_passed {
        ("circle_full", None)
    } else {
        (
            "circle_failed",
            reason.or_else(|| Some("origin_verify_failed".into())),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginBinding {
    pub check_id: String,
    pub lineage: Option<String>,
}

/// Derives the origin contract set exclusively from persisted binding events.
/// An empty or malformed set is an honest underivable result; callers must not
/// substitute a smaller verification set.
pub fn derive_origin_bindings(events: &Path) -> Result<Vec<OriginBinding>, String> {
    let text = fs::read_to_string(events).map_err(|e| format!("origin_verify_underivable:{e}"))?;
    let mut out = Vec::new();
    for line in text.lines() {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("origin_verify_underivable:{e}"))?;
        if value["event"] == "verify_default_bound" {
            let Some(checks) = value["bound_checks"].as_array() else {
                return Err("origin_verify_underivable:bound_checks".into());
            };
            for check in checks {
                let Some(id) = check.as_str() else {
                    return Err("origin_verify_underivable:check_id".into());
                };
                out.push(OriginBinding {
                    check_id: id.to_string(),
                    lineage: value["lineage"].as_str().map(str::to_string),
                });
            }
        }
    }
    if out.is_empty() {
        return Err("origin_verify_underivable:no verify_default_bound events".into());
    }
    Ok(out)
}

pub fn verify_origin<F>(bindings: &[OriginBinding], mut verify: F) -> Result<(), String>
where
    F: FnMut(&OriginBinding) -> bool,
{
    if bindings.is_empty() {
        return Err("origin_verify_underivable:empty set".into());
    }
    if bindings.iter().all(&mut verify) {
        Ok(())
    } else {
        Err("origin_verify_failed".into())
    }
}

pub fn verify_origin_from_events<F>(events: &Path, verify: F) -> Result<(), String>
where
    F: FnMut(&OriginBinding) -> bool,
{
    let bindings = derive_origin_bindings(events)?;
    verify_origin(&bindings, verify)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRunRequest {
    pub node: String,
    pub intent: String,
    pub profile: String,
    pub goal: String,
    pub origin: std::path::PathBuf,
    pub reproducer_lineage: Option<String>,
    pub model: String,
    pub provider: Provider,
}

/// Integration seam for the existing single-intent executor. The callback is
/// the existing pipeline entry; this layer only supplies the derived request
/// and records the workflow-facing events.
pub fn execute_node<F>(request: &NodeRunRequest, events: &Path, execute: F) -> Result<(), String>
where
    F: FnOnce(&NodeRunRequest) -> Result<(), String>,
{
    if !request.origin.is_dir() {
        return Err("origin workspace is missing".into());
    }
    if let Some(parent) = events.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let started = serde_json::json!({
        "event":"intent_resolved",
        "intent":request.intent,
        "workflow_node":request.node,
        "profile":request.profile,
        "model":request.model,
        "provider":format!("{:?}", request.provider).to_ascii_lowercase(),
    });
    fs::write(events, format!("{}\n", started)).map_err(|e| e.to_string())?;
    execute(request)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn investigate_goal_requires_early_diagnosis_materialization() {
        assert_eq!(
            derive_goal("investigate", "起点goal").as_deref(),
            Some(
                "『起点goal』の実行が失敗しました。まず output/diagnosis.md を作成し、調査の進展に応じて更新すること。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。"
            )
        );
    }

    #[test]
    fn requires_all_edge_conditions() {
        let route = Route {
            from: "a".into(),
            on: Verdict::Full,
            when: None,
            to: "b".into(),
            carry: vec![],
        };
        let e = EdgeEvidence {
            verdict: Verdict::Full,
            evidence: true,
            adjudicated: true,
            epoch: 2,
            previous_epoch: 1,
            carry_present: true,
        };
        assert!(edge_earned(&route, "a_to_b", &e).is_ok());
        assert!(edge_earned(&route, "a_to_b", &EdgeEvidence { epoch: 1, ..e }).is_err());
    }

    #[test]
    fn origin_bindings_are_derived_from_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(
            &path,
            r#"{"event":"verify_default_bound","bound_checks":["check-a"]}"#,
        )
        .unwrap();
        assert_eq!(
            derive_origin_bindings(&path).unwrap()[0].check_id,
            "check-a"
        );
    }

    #[test]
    fn missing_origin_bindings_fail_honestly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(&path, "{}").unwrap();
        assert!(
            derive_origin_bindings(&path)
                .unwrap_err()
                .contains("underivable")
        );
    }

    #[test]
    fn origin_verification_rejects_a_failed_member_without_shrinking() {
        let bindings = vec![
            OriginBinding {
                check_id: "a".into(),
                lineage: None,
            },
            OriginBinding {
                check_id: "b".into(),
                lineage: None,
            },
        ];
        assert_eq!(
            verify_origin(&bindings, |b| b.check_id == "a").unwrap_err(),
            "origin_verify_failed"
        );
    }
}
