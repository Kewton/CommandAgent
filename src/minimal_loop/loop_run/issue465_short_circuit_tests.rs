use super::*;
use crate::planner::recovery_repair_obligation::tests::{options, plan, setup};

#[test]
fn issue465_iteration_short_circuit_requires_fresh_target_confirmation() {
    let (_root, config) = setup();
    let options = options(&config, &plan(), 0);
    let paths = vec!["src/api.js".to_string()];
    let implementation = ImplementationCompletion::capture(&config, &options, &paths);
    let contract = CompletionContract::load_for_config(&config)
        .unwrap()
        .unwrap();
    let mut attempts = 0;
    for repaired in [false, true] {
        if repaired {
            let path = config.workspace_root.join(&paths[0]);
            let source = std::fs::read_to_string(&path).unwrap();
            std::fs::write(
                path,
                source.replace("validateShift(input)", "validateShift(input, policy)"),
            )
            .unwrap();
        }
        let result = maybe_short_circuit_satisfied_step(
            &config,
            &options,
            "Repair shift validation",
            &paths,
            Some(&contract),
            ShortCircuitContext {
                verify_attempts: &mut attempts,
                at: StepShortCircuitAt::Iteration,
                write_or_edit_seen: false,
                implementation: &implementation,
                changed_paths: &[],
            },
        )
        .unwrap();
        assert_eq!(result.is_some(), repaired);
    }
}
