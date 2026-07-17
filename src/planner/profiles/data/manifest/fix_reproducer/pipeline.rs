use crate::planner::capability_catalog::{ProbeCapability, ResolvedCapability};
use crate::planner::profile::ProfileFixReproducerSuggestion;

pub(super) fn suggestion(goal: &str) -> Option<ProfileFixReproducerSuggestion> {
    let check = super::manifest_check("pipeline_probe")?;
    let ResolvedCapability::Probe(ProbeCapability::Pipeline { entry, .. }) =
        crate::planner::capability_catalog::resolve(&check.id, &check.params).ok()?
    else {
        return None;
    };
    let failure_kind_mentioned = super::contains_any(
        goal,
        &[
            "pipeline_probe",
            "pipeline failure",
            "pipeline error",
            "execution error",
            "runtime error",
            "traceback",
            "nonzero exit",
            "non-zero exit",
            "non zero exit",
            "exit非ゼロ",
            "実行エラー",
            "実行がエラー",
            "エラーで失敗",
            "失敗します",
            "トレースバック",
            "非ゼロ終了",
            "終了コードが非ゼロ",
        ],
    );
    let basis = if failure_kind_mentioned {
        "goal_failure_kind:pipeline_execution"
    } else if goal.contains(entry.as_str()) {
        "goal_path_mention"
    } else {
        return None;
    };
    Some(ProfileFixReproducerSuggestion {
        basis: basis.to_string(),
        suggestion: format!("profile_catalog:pipeline_probe(entry={entry}) => python3 -B {entry}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIPELINE_PROBE: &str =
        "profile_catalog:pipeline_probe(entry=pipeline/main.py) => python3 -B pipeline/main.py";

    #[test]
    fn measured_pipe_goal_wording_suggests_canonical_probe_command() {
        let result = suggestion(
            "data/sales.csv を処理する pipeline/main.py の実行がエラーで失敗します。原因を特定して修正してください。修正後もデータ契約の既存検証が通ることを確認してください。",
        )
        .unwrap();

        assert_eq!(result.basis, "goal_failure_kind:pipeline_execution");
        assert_eq!(result.suggestion, PIPELINE_PROBE);
    }

    #[test]
    fn canonical_pipeline_path_alone_suggests_probe_by_path_mention() {
        let result = suggestion("pipeline/main.py の挙動を修正してください。").unwrap();

        assert_eq!(result.basis, "goal_path_mention");
        assert_eq!(result.suggestion, PIPELINE_PROBE);
    }
}
