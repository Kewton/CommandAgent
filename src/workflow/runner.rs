//! Deterministic earned-edge checks for workflow circles.
use super::schema::{Route, Verdict};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeFailure { pub edge: String, pub reason: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeEvidence { pub verdict: Verdict, pub evidence: bool, pub adjudicated: bool, pub epoch: u64, pub previous_epoch: u64, pub carry_present: bool }

pub fn edge_earned(route: &Route, edge: &str, evidence: &EdgeEvidence) -> Result<(), EdgeFailure> {
    if evidence.verdict != route.on { return Err(fail(edge, "verdict")); }
    if !evidence.evidence || !evidence.adjudicated { return Err(fail(edge, "evidence")); }
    if evidence.epoch <= evidence.previous_epoch { return Err(fail(edge, "epoch")); }
    if !evidence.carry_present { return Err(fail(edge, "carry")); }
    Ok(())
}

fn fail(edge: &str, reason: &str) -> EdgeFailure { EdgeFailure { edge: edge.into(), reason: format!("edge_not_earned:{edge}:{reason}") } }

pub fn origin_recovery_yaml_present(origin: &Path) -> bool {
    origin.join("recovery.yaml").is_file() || origin.join("recovery.yml").is_file()
}

pub fn derive_goal(intent: &str, origin_goal: &str) -> Option<String> {
    match intent {
        "investigate" => Some(format!("『{origin_goal}』の実行が失敗しました。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。")),
        "fix" => Some(format!("『{origin_goal}』の実行が失敗し、原因調査が完了しています。診断（output/diagnosis.md）と再現手順に基づき修正してください。修正後も既存の検証が通ることを確認してください。")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn requires_all_edge_conditions() {
        let route = Route { from: "a".into(), on: Verdict::Full, when: None, to: "b".into(), carry: vec![] };
        let e = EdgeEvidence { verdict: Verdict::Full, evidence: true, adjudicated: true, epoch: 2, previous_epoch: 1, carry_present: true };
        assert!(edge_earned(&route, "a_to_b", &e).is_ok());
        assert!(edge_earned(&route, "a_to_b", &EdgeEvidence { epoch: 1, ..e }).is_err());
    }
}
