pub fn applies_to(command: &str) -> bool {
    !matches!(
        command,
        "/setup-interaction-probe"
            | "--setup-interaction-probe"
            | "/model-probe"
            | "--model-probe"
            | "/plan-steps"
            | "--plan-steps"
            | "/ultra-plan"
            | "--ultra-plan"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_coding_summary_is_limited_to_execution_commands() {
        for command in [
            "/setup-interaction-probe",
            "--setup-interaction-probe",
            "/model-probe",
            "--model-probe",
            "/plan-steps",
            "--plan-steps",
            "/ultra-plan",
            "--ultra-plan",
        ] {
            assert!(!applies_to(command), "{command}");
        }
        for command in [
            "/plan-run",
            "--plan-run",
            "/ultra-plan-run",
            "--ultra-plan-run",
            "/run-ultra-plan",
            "--run-ultra-plan",
        ] {
            assert!(applies_to(command), "{command}");
        }
    }
}
