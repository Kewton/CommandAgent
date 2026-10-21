#[cfg(test)]
mod regression {
    use super::super::*;

    #[test]
    fn issue457_executor_summary_is_not_a_path_and_colocated_errors_are_retained() {
        let output = format!(
            "{MARKER}\nsummary: src/a.ts(1,2): error TS2339: first\nstdout:\nsrc/a.ts(1,2): error TS2339: first\nsrc/a.ts(1,2): error TS2345: second\nstderr:\n"
        );
        let mut errors = Vec::new();
        append_diagnostics(&output, &mut errors);
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|e| e.path == "src/a.ts"));
    }

    #[test]
    #[ignore = "requires an isolated archived-source workspace with the frozen Node toolchain"]
    fn issue457_installed_toolchain_reaches_repair_context() {
        let root = std::path::PathBuf::from(
            std::env::var("ISSUE457_TOOLCHAIN_ROOT").expect("set isolated original workspace"),
        );
        let requirement = BuildVerifierRequirement {
            command: "npm run build".into(),
            profile: Some("nextjs".into()),
            reason: "archive reproduction".into(),
            authority: "test".into(),
            status: "required".into(),
            requires_dependency_setup: true,
            required_for_completion: true,
        };
        let observed =
            crate::minimal_loop::build_verifier::observe_requirement(&root, &requirement);
        assert_eq!(
            observed.status,
            crate::minimal_loop::build_verifier::BuildVerifierStatus::Failed
        );
        assert_eq!(observed.compile_errors.len(), 33, "{observed:?}");
        let output = std::fs::read_to_string(&observed.output_path).unwrap();
        let original = output.split_once(MARKER).unwrap().0;
        assert_eq!(
            crate::minimal_loop::build_verifier::parse_compile_errors_text(original).len(),
            1
        );
        let mut report = crate::planner::verify::VerificationReport::pass();
        report.push_compile_errors("npm run build", observed.compile_errors);
        let context = crate::planner::repair::RepairContext {
            profile: Some("nextjs".into()),
            workspace_root: Some(root),
            verify_commands: vec![
                "npm run build".into(),
                "registered functional verification".into(),
            ],
            ..Default::default()
        };
        for prompt in [
            crate::planner::repair::build_repair_prompt_with_context("repair", &report, &context),
            crate::planner::repair::build_compact_compile_repair_prompt_with_context(
                "repair", &report, &context,
            ),
        ] {
            for retained in [
                "33 retained diagnostics across 4 files",
                "src/lib/store.ts",
                "src/lib/types.ts",
                "assignTask",
                "Member",
                "registered functional verification",
                "not a proven failure",
            ] {
                assert!(prompt.contains(retained), "missing {retained}: {prompt}");
            }
        }
    }

    #[test]
    fn issue457_retains_whole_project_diagnostics_beyond_five() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/corpus/apps/issue457-nextjs-contracts/diagnostics.json"
        ))
        .unwrap();
        for (case, count) in cases.as_array().unwrap().iter().zip([33, 32]) {
            let mut output = MARKER.to_string();
            for d in case["diagnostics"].as_array().unwrap() {
                output.push_str(&format!(
                    "\n{}({},{}): error TS{}: {}",
                    d["file"].as_str().unwrap(),
                    d["line"],
                    d["column"],
                    d["code"],
                    d["message"].as_str().unwrap().replace('\n', " ")
                ));
            }
            let mut errors = Vec::new();
            append_diagnostics(&output, &mut errors);
            assert_eq!(errors.len(), count);
            append_diagnostics(&output, &mut errors);
            assert_eq!(errors.len(), count);
            assert!(errors.iter().any(|e| e.path == "src/lib/store.ts"));
            assert!(
                errors
                    .iter()
                    .any(|e| e.path == "src/app/api/tasks/[id]/route.ts")
            );
        }
    }

    #[test]
    fn issue457_no_toolchain_or_non_nextjs_retains_original_failure() {
        let root = tempfile::tempdir().unwrap();
        let requirement = BuildVerifierRequirement {
            command: "npm run build".into(),
            profile: Some("nextjs".into()),
            reason: "test".into(),
            authority: "test".into(),
            status: "required".into(),
            requires_dependency_setup: false,
            required_for_completion: true,
        };
        assert_eq!(
            supplement(root.path(), &requirement, "Type error: original".into()),
            "Type error: original"
        );
    }
}
