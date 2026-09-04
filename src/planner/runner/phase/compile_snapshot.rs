use super::{BuildVerifierLifecycleObservation, BuildVerifierStatus};

pub(super) fn production_build_lifecycle_passed(
    lifecycles: &[BuildVerifierLifecycleObservation],
) -> bool {
    lifecycles.iter().any(|lifecycle| {
        lifecycle.final_status == BuildVerifierStatus::Passed
            && crate::planner::profile::is_production_build_command(
                lifecycle.requirement.profile.as_deref(),
                &lifecycle.requirement.command,
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimal_loop::build_verifier::{BuildVerifierObservation, BuildVerifierRequirement};

    fn passed_lifecycle(command: &str) -> BuildVerifierLifecycleObservation {
        let requirement = BuildVerifierRequirement {
            command: command.to_string(),
            profile: Some("nextjs".to_string()),
            reason: "test".to_string(),
            authority: "test".to_string(),
            status: "required".to_string(),
            requires_dependency_setup: false,
            required_for_completion: true,
        };
        BuildVerifierLifecycleObservation {
            before_setup: BuildVerifierObservation {
                command: requirement.command.clone(),
                profile: requirement.profile.clone(),
                authority: requirement.authority.clone(),
                required_for_completion: true,
                requires_dependency_setup: false,
                dependency_ready: true,
                attempted: true,
                duration_ms: Some(1),
                status: BuildVerifierStatus::Passed,
                primary_reason: "passed".to_string(),
                output_snippet: String::new(),
                output_path: String::new(),
                compile_errors: Vec::new(),
                foreign_toolchain: None,
            },
            requirement,
            setup: None,
            after_setup: None,
            final_status: BuildVerifierStatus::Passed,
            final_reason: "passed".to_string(),
        }
    }

    #[test]
    fn passed_verifier_inspection_does_not_authorize_compile_snapshot() {
        let inspection = passed_lifecycle(
            r#"node -p "String(require('./package.json').scripts.build)=='next build' ? true : process.exit(1)""#,
        );
        let build = passed_lifecycle("npm run build");

        assert!(!production_build_lifecycle_passed(&[inspection]));
        assert!(production_build_lifecycle_passed(&[build]));
    }
}
