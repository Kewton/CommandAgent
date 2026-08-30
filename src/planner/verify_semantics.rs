use crate::planner::verify::VerifyCommandOracleRepair;

pub(crate) fn checked_runtime_repair(
    repair: Option<VerifyCommandOracleRepair>,
) -> anyhow::Result<Option<VerifyCommandOracleRepair>> {
    let Some(repair) = repair else {
        return Ok(None);
    };
    if matches!(
        repair.kind,
        "fallback_true_stripped" | "success_failure_echo_stripped" | "exit_code_echo_stripped"
    ) {
        anyhow::bail!(
            "verify_command_rewritten_with_semantic_change:{}; regenerate one deterministic assertion instead of changing its exit polarity",
            repair.kind
        );
    }
    Ok(Some(repair))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repair(kind: &'static str) -> VerifyCommandOracleRepair {
        VerifyCommandOracleRepair {
            normalized: "true".to_string(),
            reason: "test".to_string(),
            kind,
        }
    }

    #[test]
    fn rejects_exit_polarity_changes_but_allows_output_only_repair() {
        for kind in [
            "fallback_true_stripped",
            "success_failure_echo_stripped",
            "exit_code_echo_stripped",
        ] {
            assert!(
                checked_runtime_repair(Some(repair(kind))).is_err(),
                "{kind}"
            );
        }
        assert!(checked_runtime_repair(Some(repair("stderr_merge_stripped"))).is_ok());
    }
}
