//! Output pins for the verify-failure feedback text (Issue #726).
//!
//! These tests go through the `completion` entry points only, so the same
//! expectations hold before and after the feedback generation moved here.

#[cfg(test)]
mod tests {
    use super::super::*;

    const HEADER: &str =
        "Deterministic completion verification failed. Fix the implementation and retry.";
    const GUIDANCE: &str = "Repair guidance: inspect the smallest relevant file range, then make a concrete Write/Edit change to the implementation, setup, or generated test artifact that actually owns the failure. Do not bypass verification or weaken assertions without evidence that the generated test contradicts the requested behavior.";
    const IMPL_TARGET: &str = "Repair target: implementation. Fix the implementation files that should satisfy the requested behavior.";

    fn s(value: &str) -> String {
        value.to_string()
    }

    fn compile_error(path: &str, line: usize, column: usize, message: &str) -> CompileError {
        CompileError {
            path: s(path),
            line,
            column,
            message: s(message),
            excerpt: String::new(),
            symbol: None,
            route_bound: None,
        }
    }

    fn contract(required_paths: &[&str]) -> CompletionContract {
        CompletionContract {
            required_paths: required_paths.iter().map(|p| s(p)).collect(),
            protected_paths: Vec::new(),
            verify_commands: Vec::new(),
            fix_reproducer_command: None,
            profile: None,
            goal: None,
            required_capabilities: Vec::new(),
            deterministic_oracles: Vec::new(),
            required_evidence: Vec::new(),
            evidence_hint_tokens: Vec::new(),
            required_obligations: Vec::new(),
            deferred_verify_requirements: Vec::new(),
            verify_repair_cap: 2,
        }
    }

    fn join(lines: &[&str]) -> String {
        lines.join("\n")
    }

    #[test]
    fn pass_report_yields_header_unknown_target_and_guidance() {
        let expected = join(&[
            HEADER,
            "Repair target: unknown. Inspect the verification reason and edit the smallest relevant file set.",
            GUIDANCE,
        ]);
        assert_eq!(
            format_verify_feedback(&VerificationReport::pass()),
            expected
        );
    }

    #[test]
    fn no_contract_wrapper_equals_with_contract_none() {
        let mut report = VerificationReport::pass();
        report.push_missing_path("src/lib.rs");
        report.push_dependency_missing("node_modules missing");
        assert_eq!(
            format_verify_feedback(&report),
            format_verify_feedback_with_contract(&report, None)
        );
    }

    #[test]
    fn missing_paths_use_contract_required_paths_as_targets() {
        let mut report = VerificationReport::pass();
        report.push_missing_path("src/lib.rs");
        report.push_missing_path("README.md");
        let expected = join(&[
            HEADER,
            IMPL_TARGET,
            "Target implementation files: src/main.rs",
            "Missing required paths: src/lib.rs, README.md",
            GUIDANCE,
        ]);
        let with_contract = contract(&["src/main.rs", "docs/a.md"]);
        assert_eq!(
            format_verify_feedback_with_contract(&report, Some(&with_contract)),
            expected
        );
    }

    #[test]
    fn missing_paths_without_contract_fall_back_to_missing_paths() {
        let mut report = VerificationReport::pass();
        report.push_missing_path("src/lib.rs");
        report.push_missing_path("README.md");
        let expected = join(&[
            HEADER,
            IMPL_TARGET,
            "Target implementation files: src/lib.rs",
            "Missing required paths: src/lib.rs, README.md",
            GUIDANCE,
        ]);
        assert_eq!(format_verify_feedback(&report), expected);
    }

    #[test]
    fn missing_entrypoint_omits_target_implementation_files() {
        let mut report = VerificationReport::pass();
        report.push_missing_path("src/app/page.tsx");
        let expected = join(&[
            HEADER,
            "Repair target: missing_entrypoint. Create or restore the executable entrypoint required by the selected profile.",
            "Missing required paths: src/app/page.tsx",
            GUIDANCE,
        ]);
        assert_eq!(format_verify_feedback(&report), expected);
    }

    #[test]
    fn compile_errors_render_repair_section_and_command_failure() {
        let mut report = VerificationReport::pass();
        report.push_compile_errors(
            "npm run build",
            vec![
                compile_error("src/app/page.tsx", 3, 5, "Cannot find name 'foo'."),
                compile_error("README.md", 0, 0, "Type error: bad"),
            ],
        );
        let expected = join(&[
            HEADER,
            IMPL_TARGET,
            "Target implementation files: src/app/page.tsx",
            "Compile repair details:",
            "- Compile error: src/app/page.tsx:3:5 Cannot find name 'foo'.",
            "- Compile error location: src/app/page.tsx:3:5",
            "- Compile error message: Cannot find name 'foo'.",
            "- You MUST modify src/app/page.tsx using the edit tool; a reply without file edits fails this repair.",
            "- Compile error: README.md Type error: bad",
            "- Compile error location: README.md",
            "- Compile error message: Type error: bad",
            "- You MUST modify README.md using the edit tool; a reply without file edits fails this repair.",
            "- Compile repair edit mandate: edit one of these source files using the edit tool: README.md, src/app/page.tsx.",
            "- Do not answer in prose only; a repair response without a source edit fails this compile repair.",
            "Command failed: `npm run build`",
            "implementation_compile_error: src/app/page.tsx:3:5 Cannot find name 'foo'.; README.md Type error: bad",
            "Likely target files: src/app/page.tsx",
            GUIDANCE,
        ]);
        assert_eq!(format_verify_feedback(&report), expected);
    }

    #[test]
    fn dependency_missing_reports_reason_without_target_files() {
        let mut report = VerificationReport::pass();
        report.push_dependency_missing("node_modules missing");
        let expected = join(&[
            HEADER,
            "Repair target: dependency_setup. Install or restore project dependencies before retrying build verification.",
            "Dependency missing: node_modules missing",
            GUIDANCE,
        ]);
        assert_eq!(format_verify_feedback(&report), expected);
    }

    #[test]
    fn command_failure_reports_framework_mismatch_targets_and_excerpt() {
        let mut report = VerificationReport::pass();
        report.push_command_failure(
            "python3 -m unittest tests/test_calc.py",
            "test_framework_mismatch:pytest_style_under_unittest\nAssertionError: expected 3 actual 4\n  File \"src/calc.py\", line 3",
        );
        let expected = join(&[
            HEADER,
            "Repair target: test_or_evidence. Add or strengthen deterministic evidence that verifies the requested behavior.",
            "Test framework mismatch: `python3 -m unittest` does not discover pytest-style free functions. Convert tests to `unittest.TestCase` methods or update the allowed verify command.",
            "Command failed: `python3 -m unittest tests/test_calc.py`\ntest_framework_mismatch:pytest_style_under_unittest AssertionError: expected 3 actual 4 File \"src/calc.py\", line 3",
            "Likely target files: src/calc.py, tests/test_calc.py",
            "Failure excerpt: AssertionError: expected 3 actual 4",
            GUIDANCE,
        ]);
        assert_eq!(format_verify_feedback(&report), expected);
    }

    #[test]
    fn verifier_false_negatives_distinguish_timeout_from_malformed() {
        let mut report = VerificationReport::pass();
        report.push_verifier_command_false_negative("sleep 100", "verify_command_timeout: hung");
        report.push_verifier_command_false_negative("grep -x", "bad");
        let expected = join(&[
            HEADER,
            "Repair target: verifier_command. The verify command is malformed; the artifact may already satisfy the requirement.",
            "Verifier command false negative: `sleep 100`\nverify_command_timeout: hung",
            "The verify command hangs - replace it with a bounded check. Do not spend implementation-edit turns on this OracleError unless a separate artifact failure is present.",
            "Verifier command false negative: `grep -x`\nbad",
            "The verify command is malformed; the artifact may already satisfy the requirement.",
            GUIDANCE,
        ]);
        assert_eq!(format_verify_feedback(&report), expected);
    }

    #[test]
    fn profile_failures_add_matching_repair_guidance_only() {
        let evidence = "Capability evidence guidance: add the smallest concrete test/check/UI evidence requested by the task, then keep the existing verification strong enough to exercise that evidence.";
        let mut report = VerificationReport::pass();
        report.push_profile_failure("CSS side-effect imports require declare module \"*.css\"");
        report.push_profile_failure("page must start with \"use client\"");
        report.push_profile_failure("missing_required_capabilities: todo");
        report.push_profile_failure("weak_verification_evidence: x");
        let expected = join(&[
            HEADER,
            "Repair target: framework_config. Fix framework configuration, routing boundaries, or generated type declarations.",
            "Profile contract failed: CSS side-effect imports require declare module \"*.css\"",
            "Next.js CSS repair: replace or create `src/app/global.d.ts` with exactly `declare module \"*.css\";` on its own line.",
            "Profile contract failed: page must start with \"use client\"",
            "Next.js client component repair: put `\"use client\";` as the first non-empty statement in the interactive app page before imports.",
            "Profile contract failed: missing_required_capabilities: todo",
            evidence,
            "Profile contract failed: weak_verification_evidence: x",
            evidence,
            GUIDANCE,
        ]);
        assert_eq!(format_verify_feedback(&report), expected);
    }

    #[test]
    fn partial_css_markers_get_no_css_guidance() {
        let mut report = VerificationReport::pass();
        report.push_profile_failure("CSS side-effect imports require");
        report.push_profile_failure("declare module \"*.css\"");
        let expected = join(&[
            HEADER,
            "Repair target: framework_config. Fix framework configuration, routing boundaries, or generated type declarations.",
            "Profile contract failed: CSS side-effect imports require",
            "Profile contract failed: declare module \"*.css\"",
            GUIDANCE,
        ]);
        assert_eq!(format_verify_feedback(&report), expected);
    }

    #[test]
    fn target_implementation_files_follow_source_priority() {
        let mut from_command = VerificationReport::pass();
        from_command.push_command_failure("pytest tests/test_x.py", "AssertionError in src/x.py");
        assert_eq!(
            target_implementation_files(&from_command, None),
            vec![s("src/x.py")]
        );
        assert_eq!(
            target_implementation_files(&VerificationReport::pass(), None),
            vec![s("src/app/page.tsx")]
        );
        assert_eq!(
            target_implementation_files(
                &VerificationReport::pass(),
                Some(&contract(&["src/b.rs", "a.md", "src/a.rs"]))
            ),
            vec![s("src/a.rs"), s("src/b.rs")]
        );
    }
}
