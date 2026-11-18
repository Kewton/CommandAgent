//! Isolation ratchet (Issue #1040).
//!
//! Tests share one process: its panic hook, its environment, the TUI's global
//! presentation state, and every descriptor open for writing when another test
//! thread forks. Stage 1 (#1010, #1012, #993, #984, #996, #1029, #995, #968,
//! #992) closed the holes that touched that state outside a guard. This test
//! keeps the kinds of line that opened them from growing: it counts them per
//! file in `src/` and `tests/` and fails when a file goes over its limit, or
//! when a file without a limit has one.
//!
//! - R1: a line naming `set_hook` or `take_hook` (the process panic hook).
//! - R2: a line calling `set_var(` or `remove_var(`, and, in test code, a line
//!   naming `RuntimeEnvironmentGuard::install` (the three production calls are
//!   production code, so they are not counted).
//! - R3: in test code, a line passing an exec mode (`0o[0-7]?[157][0-7][0-7]`)
//!   to `set_mode`/`from_mode`/`set_permissions`, and a line naming `fs::copy`.
//!   Whether a copy is an executable cannot be told from the line, so every
//!   `fs::copy` line in test code is counted (stricter than the Issue's text).
//! - R4: in test code, a line naming a test entry into the TUI global state
//!   (`render_current_plan`, `render_status_card`, `set_plan_for_test`).
//!
//! Lines are counted, not tokens; comment and doc-comment lines are not
//! counted. Test code is all of `tests/` and, in `src/`, what
//! `classified_rust_lines` (copied unchanged from `tests/generality_guardrails.rs`)
//! marks as test, plus two additions kept outside that function: a
//! `#[cfg(all(test, ...))]` attribute opens test code as `#[cfg(test)]` does,
//! and a file declared by a module item under `#[cfg(test)]` (or brought in by
//! `include!` from test code) is test code as a whole.
//!
//! The limits are the counts at the commit that added this test. Lower a limit
//! in the change that removes lines; never raise one to admit growth.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use Rule::{Env, ExecMode, FsCopy, PanicHook, TuiGlobal};

/// This file holds every token as a string literal, so counting it would only
/// count the guard itself. It is the one file left out.
const THIS_FILE: &str = "tests/test_isolation_guardrails.rs";

/// Forms that name a file the classification could not resolve. Each one would
/// leave a test file counted as production, so a new one fails the guard
/// instead of passing silently.
const KNOWN_UNRESOLVED: &[&str] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rule {
    PanicHook,
    Env,
    ExecMode,
    FsCopy,
    TuiGlobal,
}

impl Rule {
    const ALL: [Rule; 5] = [PanicHook, Env, ExecMode, FsCopy, TuiGlobal];

    fn id(self) -> &'static str {
        match self {
            PanicHook => "R1 (panic hook)",
            Env => "R2 (environment)",
            ExecMode => "R3 (exec mode)",
            FsCopy => "R3 (fs::copy)",
            TuiGlobal => "R4 (TUI global)",
        }
    }

    fn fix(self) -> &'static str {
        match self {
            PanicHook => {
                "Install a panic hook only through the shared guarded hook in src/cli_panic_boundary.rs, or run the test in a separate process."
            }
            Env => {
                "Change the environment only under the binary's environment guard (its lock), or in a child process; install RuntimeEnvironmentGuard in tests only under that guard."
            }
            ExecMode => {
                "Write the executable outside this process (a separate-process helper of the copy_a_program_elsewhere kind) or under a guard, so no descriptor open for writing reaches a forked child (ETXTBSY)."
            }
            FsCopy => {
                "If the copy is an executable, copy it in a separate process with a helper of the copy_a_program_elsewhere kind; if nothing executes it, put one copy helper in one place and call that."
            }
            TuiGlobal => {
                "Read or set the TUI global presentation state only in a separate process."
            }
        }
    }

    fn matches(self, line: &str, test_code: bool, exec_mode: &regex::Regex) -> bool {
        match self {
            PanicHook => line.contains("set_hook") || line.contains("take_hook"),
            Env => {
                line.contains("set_var(")
                    || line.contains("remove_var(")
                    || (test_code && line.contains("RuntimeEnvironmentGuard::install"))
            }
            ExecMode => {
                test_code
                    && exec_mode.is_match(line)
                    && ["set_mode", "from_mode", "set_permissions"]
                        .iter()
                        .any(|name| line.contains(name))
            }
            FsCopy => test_code && line.contains("fs::copy"),
            TuiGlobal => {
                test_code
                    && [
                        "render_current_plan",
                        "render_status_card",
                        "set_plan_for_test",
                    ]
                    .iter()
                    .any(|name| line.contains(name))
            }
        }
    }
}

/// Per-file limits: the counts at the commit that added this test. Lower them;
/// never raise them.
const LIMITS: &[(&str, Rule, usize)] = &[
    ("src/bounded_process.rs", Env, 7),
    ("src/cli_pack.rs", Env, 12),
    ("src/cli_pack.rs", FsCopy, 1),
    ("src/cli_panic_boundary.rs", PanicHook, 9),
    ("src/completion_metadata/cli.rs", FsCopy, 1),
    ("src/completion_metadata/data.rs", FsCopy, 2),
    ("src/completion_metadata/intent.rs", FsCopy, 2),
    ("src/fetch_probe/transport.rs", ExecMode, 1),
    ("src/headless_summary/golden_tests.rs", FsCopy, 2),
    ("src/minimal_loop/build_verifier.rs", ExecMode, 3),
    ("src/minimal_loop/completion.rs", ExecMode, 1),
    ("src/minimal_loop/completion/issue508_tests.rs", ExecMode, 1),
    ("src/minimal_loop/dependency_setup.rs", ExecMode, 2),
    ("src/minimal_loop/interaction_probe.rs", ExecMode, 3),
    ("src/minimal_loop/loop_run/issue420_tests.rs", FsCopy, 1),
    (
        "src/minimal_loop/stagnation_escalation/issue449_tests.rs",
        FsCopy,
        2,
    ),
    ("src/planner/artifact_contract_checks/tests.rs", FsCopy, 1),
    ("src/planner/artifact_contract_checks/tests.rs", ExecMode, 1),
    ("src/planner/auto_recovery.rs", FsCopy, 1),
    (
        "src/planner/auto_recovery/issue448_contract_tests/runtime_transport.rs",
        FsCopy,
        1,
    ),
    (
        "src/planner/auto_recovery/issue448_contract_tests/runtime_transport.rs",
        ExecMode,
        1,
    ),
    ("src/planner/auto_recovery/issue456_tests.rs", FsCopy, 1),
    (
        "src/planner/auto_recovery/issue458_safety_tests.rs",
        FsCopy,
        1,
    ),
    (
        "src/planner/auto_recovery/issue458_safety_tests.rs",
        ExecMode,
        1,
    ),
    ("src/planner/auto_recovery/issue467_tests.rs", FsCopy, 1),
    ("src/planner/auto_recovery/issue475_tests.rs", FsCopy, 2),
    ("src/planner/auto_recovery/issue475_tests.rs", ExecMode, 1),
    (
        "src/planner/auto_recovery/issue475_tests/runtime_transport.rs",
        ExecMode,
        1,
    ),
    ("src/planner/auto_recovery/issue485_tests.rs", ExecMode, 1),
    (
        "src/planner/final_acceptance/issue507_tests.rs",
        ExecMode,
        1,
    ),
    ("src/planner/final_acceptance/issue511_tests.rs", FsCopy, 1),
    (
        "src/planner/final_acceptance/issue511_tests.rs",
        ExecMode,
        1,
    ),
    ("src/planner/pack/supply/tests.rs", ExecMode, 4),
    ("src/planner/profile_manifest/supply.rs", ExecMode, 2),
    ("src/planner/profiles/community_mini_app.rs", FsCopy, 1),
    (
        "src/planner/profiles/data/step_policy/verify_default.rs",
        FsCopy,
        1,
    ),
    (
        "src/planner/profiles/nextjs/response_shape/local_flow/tests.rs",
        FsCopy,
        1,
    ),
    ("src/planner/profiles/python_cli/argv_probe.rs", FsCopy, 2),
    (
        "src/planner/profiles/python_cli/entry_cardinality.rs",
        ExecMode,
        1,
    ),
    ("src/planner/profiles/python_cli/help_binding.rs", FsCopy, 1),
    ("src/planner/recovery_json_preflight_tests.rs", FsCopy, 2),
    (
        "src/planner/recovery_observation_policy/nextjs_outputs/issue475_tests.rs",
        FsCopy,
        1,
    ),
    (
        "src/planner/recovery_repair_obligation/authority_tests.rs",
        FsCopy,
        2,
    ),
    (
        "src/planner/recovery_repair_obligation/configuration_tests.rs",
        FsCopy,
        1,
    ),
    (
        "src/planner/recovery_repair_obligation/nextjs_tests.rs",
        FsCopy,
        1,
    ),
    ("src/planner/recovery_repair_obligation/tests.rs", FsCopy, 1),
    (
        "src/planner/recovery_repair_obligation/typescript_tests.rs",
        FsCopy,
        1,
    ),
    ("src/planner/recovery_snapshot.rs", FsCopy, 1),
    ("src/planner/repair_targeting/cli.rs", FsCopy, 1),
    ("src/planner/runner/acceptance/issue507_tests.rs", Env, 1),
    ("src/planner/runner/phase/flow/issue435_tests.rs", FsCopy, 1),
    ("src/planner/runner/phase/issue490_tests.rs", FsCopy, 1),
    (
        "src/planner/runner/phase/model_verifier_disagreement/tests.rs",
        ExecMode,
        1,
    ),
    (
        "src/planner/runner/phase/model_verifier_edit/tests.rs",
        ExecMode,
        1,
    ),
    ("src/planner/runner/tests/assurance_tests.rs", ExecMode, 1),
    (
        "src/planner/runner/tests/data_pre_satisfied_tests.rs",
        FsCopy,
        1,
    ),
    (
        "src/planner/runner/tests/issue420_step_completion_tests.rs",
        FsCopy,
        1,
    ),
    (
        "src/planner/runner/tests/phase_runtime_tests.rs",
        ExecMode,
        1,
    ),
    (
        "src/planner/runner/tests/recovery_host_verification_tests.rs",
        FsCopy,
        2,
    ),
    ("src/planner/runner/tests/support/browser.rs", ExecMode, 4),
    ("src/planner/runner/tests/support/build.rs", ExecMode, 6),
    ("src/planner/verify.rs", ExecMode, 1),
    ("src/planner/verify_command_shape/tests.rs", ExecMode, 2),
    ("src/startup_credentials.rs", Env, 9),
    ("src/structured_budget/entry_tests.rs", Env, 2),
    ("src/structured_budget/entry_tests.rs", FsCopy, 1),
    ("src/structured_budget/ledger.rs", FsCopy, 1),
    ("src/tools/path_guard.rs", ExecMode, 1),
    ("src/tools/profile_tools/commit.rs", ExecMode, 1),
    ("src/tools/profile_tools/effects.rs", Env, 57),
    ("src/tools/profile_tools/effects.rs", FsCopy, 1),
    ("src/tools/profile_tools/effects.rs", ExecMode, 3),
    ("src/tools/profile_tools/isolation.rs", Env, 2),
    ("src/tools/profile_tools/isolation.rs", ExecMode, 2),
    ("src/tui/boundary_shell/pack_catalog.rs", ExecMode, 1),
    ("src/tui/presentation.rs", TuiGlobal, 5),
    ("src/tui/profile_transport_tests.rs", ExecMode, 2),
    ("tests/artifact_check_diagnostics.rs", Env, 6),
    ("tests/artifact_contract_checks.rs", Env, 10),
    ("tests/artifact_contract_checks.rs", FsCopy, 3),
    ("tests/artifact_repair_recheck.rs", Env, 1),
    ("tests/budget_closing.rs", Env, 2),
    ("tests/cli_profile_conformance.rs", FsCopy, 1),
    ("tests/completion_obligation_review.rs", Env, 3),
    ("tests/conformance/mod.rs", Env, 1),
    ("tests/conformance/mod.rs", ExecMode, 2),
    ("tests/fetch_probe_contract.rs", FsCopy, 1),
    ("tests/fetch_probe_stage2.rs", FsCopy, 2),
    ("tests/final_repair_early_completion.rs", Env, 1),
    ("tests/gui_read_only_guard.rs", ExecMode, 1),
    ("tests/gui_server.rs", FsCopy, 2),
    ("tests/gui_server.rs", ExecMode, 24),
    ("tests/issue228_plan_yaml.rs", FsCopy, 1),
    ("tests/issue428_bash_path_tokens.rs", FsCopy, 1),
    ("tests/issue441_placeholder.rs", ExecMode, 1),
    ("tests/issue459_recovery.rs", FsCopy, 3),
    ("tests/issue502_os_sandbox_network.rs", ExecMode, 1),
    ("tests/issue502_os_sandbox_read.rs", Env, 3),
    ("tests/issue521_build_report.rs", ExecMode, 1),
    ("tests/issue521_config_resolution.rs", Env, 5),
    ("tests/issue521_interaction_decode.rs", ExecMode, 1),
    ("tests/issue585_bash_quoted_expansion.rs", ExecMode, 1),
    ("tests/missing_read_paths.rs", ExecMode, 1),
    ("tests/operator_check.rs", Env, 2),
    ("tests/output_limit_feedback.rs", Env, 2),
    ("tests/pack_actions.rs", FsCopy, 2),
    ("tests/profile_tool_commit.rs", Env, 2),
    ("tests/profile_tool_commit.rs", FsCopy, 1),
    ("tests/profile_tool_commit.rs", ExecMode, 3),
    ("tests/profile_tool_registration.rs", Env, 3),
    ("tests/profile_tool_registration.rs", FsCopy, 2),
    ("tests/profile_tool_registration_tui.rs", Env, 2),
    ("tests/profile_tool_registration_tui.rs", ExecMode, 1),
    ("tests/profile_tool_schema.rs", Env, 2),
    ("tests/profile_tool_schema_feedback.rs", Env, 2),
    ("tests/profile_tool_schema_responses.rs", Env, 4),
    ("tests/protection_coverage_audit.rs", Env, 1),
    ("tests/python_cli_phase_entry_identity.rs", ExecMode, 1),
    ("tests/release_build.rs", ExecMode, 1),
    ("tests/release_distribution.rs", FsCopy, 1),
    ("tests/release_distribution.rs", ExecMode, 1),
    ("tests/requirement_plan.rs", Env, 3),
    ("tests/setup_script.rs", FsCopy, 1),
    ("tests/setup_script.rs", ExecMode, 1),
    ("tests/startup_credentials.rs", ExecMode, 2),
    ("tests/structured_run_budget.rs", ExecMode, 1),
    ("tests/structured_stage_receipts.rs", Env, 5),
    ("tests/support/issue459_replay.rs", FsCopy, 1),
    ("tests/verify_command_admission.rs", ExecMode, 1),
    ("tests/verify_environment_failures.rs", ExecMode, 1),
];

/// Which test-code additions run outside `classified_rust_lines`. The real
/// count uses both; the controls switch each off to show what it adds.
#[derive(Debug, Clone, Copy)]
struct Boundary {
    cfg_all_test: bool,
    declared_files: bool,
}

const FULL: Boundary = Boundary {
    cfg_all_test: true,
    declared_files: true,
};

type Counts = BTreeMap<(String, Rule), usize>;

#[test]
fn isolation_lines_stay_within_their_per_file_limits() {
    let tree = repository_tree();
    let (counts, unresolved) = count_tree(&tree, FULL);
    let (again, _) = count_tree(&tree, FULL);
    assert_eq!(counts, again, "the count is not deterministic");

    let known: BTreeSet<String> = KNOWN_UNRESOLVED.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        unresolved, known,
        "test-code module declarations or include! forms that name no file this guard can resolve; their lines would count as production. Make the form resolvable here or name it in KNOWN_UNRESOLVED and in docs/dev/dev-guardrails.md."
    );

    let mut seen = BTreeSet::new();
    for (path, rule, limit) in LIMITS {
        assert!(
            seen.insert((*path, *rule)),
            "duplicate limit for {path} {rule:?}"
        );
        assert!(
            *limit > 0,
            "{path} {rule:?}: drop a limit of 0 instead of keeping it"
        );
    }

    let violations = violations(&counts, &limit_map(LIMITS));
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

#[test]
fn each_rule_turns_red_when_a_line_is_added_and_green_when_it_is_removed() {
    let lines = [
        (PanicHook, "        let previous = std::panic::take_hook();"),
        (
            Env,
            "        unsafe { std::env::set_var(\"KEY\", \"value\") };",
        ),
        (
            ExecMode,
            "        std::fs::set_permissions(&path, Permissions::from_mode(0o755)).unwrap();",
        ),
        (FsCopy, "        std::fs::copy(&source, &target).unwrap();"),
        (TuiGlobal, "        let card = render_status_card(&config);"),
    ];
    for (rule, line) in lines {
        let path = "src/sample.rs";
        let base = sample_source(&[line]);
        let grown = sample_source(&[line, line]);
        let limits = limit_map(&[(path, rule, 1)]);

        let green = violations(&count_one(path, &base), &limits);
        assert!(green.is_empty(), "{rule:?} base: {green:?}");

        let red = violations(&count_one(path, &grown), &limits);
        assert_eq!(red.len(), 1, "{rule:?} grown: {red:?}");
        let message = &red[0];
        for part in [
            path,
            rule.id(),
            "2 lines",
            "over its limit of 1 by 1",
            rule.fix(),
        ] {
            assert!(message.contains(part), "{rule:?}: {message} lacks {part}");
        }
        assert!(message.contains("Never raise"), "{message}");

        let removed = violations(&count_one(path, &base), &limits);
        assert!(removed.is_empty(), "{rule:?} removed: {removed:?}");

        let comment = format!("        // {}", line.trim());
        let commented = sample_source(&[line, comment.as_str()]);
        let commented = violations(&count_one(path, &commented), &limits);
        assert!(commented.is_empty(), "{rule:?} comment: {commented:?}");
    }
}

#[test]
fn a_file_without_a_limit_turns_red() {
    let source = sample_source(&["        std::fs::copy(&source, &target).unwrap();"]);
    let limits = limit_map(&[("src/other.rs", FsCopy, 1)]);
    let red = violations(&count_one("src/new.rs", &source), &limits);
    assert_eq!(red.len(), 1, "{red:?}");
    assert!(red[0].contains("src/new.rs"), "{}", red[0]);
    assert!(red[0].contains("no limit"), "{}", red[0]);

    let green = violations(&count_one("src/other.rs", &source), &limits);
    assert!(green.is_empty(), "{green:?}");
}

#[test]
fn r3_and_r4_count_test_code_only() {
    let source = "\
fn production() {
    std::fs::copy(&source, &target).unwrap();
    std::fs::set_permissions(&path, Permissions::from_mode(0o755)).unwrap();
    let card = render_status_card(&config);
    unsafe { std::env::set_var(\"KEY\", \"value\") };
    let previous = std::panic::take_hook();
}
#[cfg(test)]
mod tests {
    fn helper() {
        std::fs::copy(&source, &target).unwrap();
        std::fs::set_permissions(&path, Permissions::from_mode(0o755)).unwrap();
        let card = render_status_card(&config);
    }
}
";
    let counts = count_one("src/sample.rs", source);
    for rule in [ExecMode, FsCopy, TuiGlobal] {
        assert_eq!(count_of(&counts, "src/sample.rs", rule), 1, "{rule:?}");
    }
    for rule in [PanicHook, Env] {
        assert_eq!(count_of(&counts, "src/sample.rs", rule), 1, "{rule:?}");
    }
    let in_tests = count_one("tests/sample.rs", source);
    assert_eq!(count_of(&in_tests, "tests/sample.rs", FsCopy), 2);

    let install = "\
fn production() {
    let _guard = RuntimeEnvironmentGuard::install(pack.as_ref());
}
#[cfg(test)]
mod tests {
    fn helper() {
        let _guard = RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
    }
}
";
    assert_eq!(
        count_of(&count_one("src/sample.rs", install), "src/sample.rs", Env),
        1
    );
}

#[test]
fn cfg_all_test_blocks_count_as_test_code() {
    let source = "\
fn production() {}
#[cfg(all(test, unix))]
mod tests {
    fn helper() {
        std::fs::copy(&source, &target).unwrap();
    }
}
";
    let tree = tree_of(&[("src/sample.rs", source)]);
    let limits = limit_map(&[]);

    let (with, _) = count_tree(&tree, FULL);
    assert_eq!(count_of(&with, "src/sample.rs", FsCopy), 1);
    assert_eq!(violations(&with, &limits).len(), 1);

    let without_addition = Boundary {
        cfg_all_test: false,
        ..FULL
    };
    let (without, _) = count_tree(&tree, without_addition);
    assert_eq!(count_of(&without, "src/sample.rs", FsCopy), 0);
    assert!(violations(&without, &limits).is_empty());

    assert!(is_cfg_all_test("#[cfg(all(unix, test))]"));
    assert!(!is_cfg_all_test("#[cfg(all(unix, not(test)))]"));
    assert!(!is_cfg_all_test("#[cfg(any(test, feature = \"x\"))]"));
}

#[test]
fn files_declared_under_cfg_test_count_as_test_code() {
    let copy = "    std::fs::copy(&source, &target).unwrap();\n";
    let lib = "\
mod production;
#[cfg(test)]
mod helper_tests;
#[cfg(test)]
#[path = \"elsewhere/renamed_tests.rs\"]
mod renamed;
#[cfg(test)]
mod tests {
    include!(\"included/body.rs\");
}
#[cfg(test)]
mod missing_tests;
";
    let nested = "\
fn production() {}
#[cfg(test)]
mod cases {
    #[path = \"pathed.rs\"]
    mod pathed;
    mod plain;
}
";
    let tree = tree_of(&[
        ("src/lib.rs", lib),
        ("src/production.rs", copy),
        ("src/helper_tests.rs", &format!("mod deeper;\n{copy}")),
        ("src/helper_tests/deeper.rs", copy),
        ("src/elsewhere/renamed_tests.rs", copy),
        (
            "src/included/body.rs",
            &format!("include!(\"inner.rs\");\n{copy}"),
        ),
        ("src/included/inner.rs", copy),
        ("src/nested.rs", nested),
        ("src/nested/cases/pathed.rs", copy),
        ("src/nested/cases/plain.rs", copy),
    ]);
    let declared = [
        "src/helper_tests.rs",
        "src/helper_tests/deeper.rs",
        "src/elsewhere/renamed_tests.rs",
        "src/included/body.rs",
        "src/included/inner.rs",
        "src/nested/cases/pathed.rs",
        "src/nested/cases/plain.rs",
    ];

    let (with, unresolved) = count_tree(&tree, FULL);
    for path in declared {
        assert_eq!(count_of(&with, path, FsCopy), 1, "{path}");
    }
    assert_eq!(count_of(&with, "src/production.rs", FsCopy), 0);
    assert_eq!(
        unresolved,
        BTreeSet::from(["src/lib.rs:12: mod missing_tests;".to_string()])
    );
    assert_eq!(violations(&with, &limit_map(&[])).len(), declared.len());

    let without_addition = Boundary {
        declared_files: false,
        ..FULL
    };
    let (without, _) = count_tree(&tree, without_addition);
    for path in declared {
        assert_eq!(count_of(&without, path, FsCopy), 0, "{path}");
    }
    assert!(violations(&without, &limit_map(&[])).is_empty());
}

fn sample_source(lines_in_test: &[&str]) -> String {
    let mut source =
        String::from("fn production() {}\n#[cfg(test)]\nmod tests {\n    fn helper() {\n");
    for line in lines_in_test {
        source.push_str(line);
        source.push('\n');
    }
    source.push_str("    }\n}\n");
    source
}

fn count_one(path: &str, text: &str) -> Counts {
    count_tree(&tree_of(&[(path, text)]), FULL).0
}

fn count_of(counts: &Counts, path: &str, rule: Rule) -> usize {
    counts.get(&(path.to_string(), rule)).copied().unwrap_or(0)
}

fn tree_of(files: &[(&str, &str)]) -> BTreeMap<String, String> {
    files
        .iter()
        .map(|(path, text)| (path.to_string(), text.to_string()))
        .collect()
}

fn limit_map(limits: &[(&str, Rule, usize)]) -> BTreeMap<(String, Rule), usize> {
    limits
        .iter()
        .map(|(path, rule, limit)| ((path.to_string(), *rule), *limit))
        .collect()
}

fn violations(counts: &Counts, limits: &BTreeMap<(String, Rule), usize>) -> Vec<String> {
    let mut out = Vec::new();
    for ((path, rule), current) in counts {
        let fix = rule.fix();
        let tail = "Or land a change that removes such lines and lowers this limit in tests/test_isolation_guardrails.rs. Never raise a limit to admit growth.";
        match limits.get(&(path.clone(), *rule)) {
            Some(limit) if current <= limit => {}
            Some(limit) => out.push(format!(
                "{path}: {} has {current} lines, over its limit of {limit} by {}. {fix} {tail}",
                rule.id(),
                current - limit
            )),
            None => out.push(format!(
                "{path}: {} has {current} lines, and this file has no limit (limit of 0, over by {current}). {fix} {tail}",
                rule.id()
            )),
        }
    }
    out
}

/// Every Rust source in `src/` and `tests/`, by repository-relative path,
/// except this file.
fn repository_tree() -> BTreeMap<String, String> {
    let mut tree = BTreeMap::new();
    for root in ["src", "tests"] {
        for path in rust_source_files(Path::new(root)) {
            let rel = path.to_string_lossy().replace('\\', "/");
            if rel == THIS_FILE {
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
            tree.insert(rel, text);
        }
    }
    tree
}

fn count_tree(tree: &BTreeMap<String, String>, boundary: Boundary) -> (Counts, BTreeSet<String>) {
    let (declared, unresolved) = if boundary.declared_files {
        declared_test_files(tree, boundary)
    } else {
        (BTreeSet::new(), BTreeSet::new())
    };
    let exec_mode = regex::Regex::new(r"0o[0-7]?[157][0-7][0-7]").unwrap();
    let mut counts = Counts::new();
    for (path, text) in tree {
        let whole_file = path.starts_with("tests/") || declared.contains(path);
        for (line, class) in boundary_lines(text, boundary) {
            if line.trim_start().starts_with("//") {
                continue;
            }
            let test_code = whole_file || class == RustLineClass::Test;
            for rule in Rule::ALL {
                if rule.matches(line, test_code, &exec_mode) {
                    *counts.entry((path.clone(), rule)).or_default() += 1;
                }
            }
        }
    }
    (counts, unresolved)
}

/// `classified_rust_lines`, with a `#[cfg(all(test, ...))]` line read as
/// `#[cfg(test)]` when the boundary asks for it. The copied function is not
/// changed; the line is rewritten before it sees it.
fn boundary_lines(text: &str, boundary: Boundary) -> Vec<(&str, RustLineClass)> {
    let rewritten: String = text
        .lines()
        .map(|line| {
            if boundary.cfg_all_test && is_cfg_all_test(line.trim()) {
                let indent = &line[..line.len() - line.trim_start().len()];
                format!("{indent}#[cfg(test)]\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    let classes: Vec<RustLineClass> = classified_rust_lines(&rewritten)
        .into_iter()
        .map(|(_, class)| class)
        .collect();
    text.lines().zip(classes).collect()
}

fn is_cfg_all_test(trimmed: &str) -> bool {
    trimmed
        .strip_prefix("#[cfg(all(")
        .and_then(|rest| rest.strip_suffix("))]"))
        .is_some_and(|args| args.split(',').any(|arg| arg.trim() == "test"))
}

/// Files in `src/` that are test code as a whole: declared by a module item
/// under a cfg(test) attribute, inside a cfg(test) inline module, or inside a
/// file that is itself test code; and files `include!`d from there. Returns
/// them with the forms that name no file in the tree.
fn declared_test_files(
    tree: &BTreeMap<String, String>,
    boundary: Boundary,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut declared = BTreeSet::new();
    loop {
        let mut unresolved = BTreeSet::new();
        let mut found = Vec::new();
        for (path, text) in tree.iter().filter(|(path, _)| path.starts_with("src/")) {
            let whole_file = declared.contains(path);
            found.extend(module_targets(
                path,
                text,
                whole_file,
                tree,
                boundary,
                &mut unresolved,
            ));
        }
        let mut added = false;
        for target in found {
            if target.starts_with("src/") {
                added |= declared.insert(target);
            }
        }
        if !added {
            return (declared, unresolved);
        }
    }
}

fn module_targets(
    path: &str,
    text: &str,
    whole_file: bool,
    tree: &BTreeMap<String, String>,
    boundary: Boundary,
    unresolved: &mut BTreeSet<String>,
) -> Vec<String> {
    let parent = parent_dir(path);
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let mod_rs = matches!(file_name, "mod.rs" | "lib.rs" | "main.rs") || parent == "src/bin";
    let module_dir = if mod_rs {
        parent.to_string()
    } else {
        path.trim_end_matches(".rs").to_string()
    };

    let mut out = Vec::new();
    // Inline modules enclosing the line: name, indentation, test code.
    let mut inline: Vec<(String, usize, bool)> = Vec::new();
    let mut attributes: Vec<&str> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if !trimmed.is_empty() {
            while inline.last().is_some_and(|(_, open, _)| indent <= *open) {
                inline.pop();
            }
        }
        let attributed = attributes.iter().any(|attr| {
            *attr == "#[cfg(test)]" || (boundary.cfg_all_test && is_cfg_all_test(attr))
        });
        let test_code = whole_file || attributed || inline.iter().any(|(_, _, test)| *test);
        let names: Vec<&str> = inline.iter().map(|(name, _, _)| name.as_str()).collect();
        let at = format!("{path}:{}", index + 1);

        if let Some((name, ';')) = module_item(trimmed)
            && test_code
        {
            let candidates = match attributes
                .iter()
                .rev()
                .find_map(|attr| path_attribute(attr))
            {
                Some(relative) => {
                    let base = if mod_rs || names.is_empty() {
                        &parent
                    } else {
                        &module_dir
                    };
                    vec![normalize(&join(base, &names, relative))]
                }
                None => vec![
                    join(&module_dir, &names, &format!("{name}.rs")),
                    join(&module_dir, &names, &format!("{name}/mod.rs")),
                ],
            };
            match candidates
                .into_iter()
                .find(|candidate| tree.contains_key(candidate))
            {
                Some(target) => out.push(target),
                None => {
                    unresolved.insert(format!("{at}: {trimmed}"));
                }
            }
        }
        if trimmed.contains("include!(") {
            let target =
                include_literal(trimmed).map(|relative| normalize(&join(&parent, &[], relative)));
            match target {
                Some(target) if test_code && tree.contains_key(&target) => out.push(target),
                _ => {
                    unresolved.insert(format!("{at}: {trimmed}"));
                }
            }
        }
        if let Some((name, '{')) = module_item(trimmed) {
            inline.push((name.to_string(), indent, test_code));
        }
        if trimmed.starts_with("#[") {
            attributes.push(trimmed);
        } else {
            attributes.clear();
        }
    }
    out
}

/// `mod name;` or `mod name {`, with an optional visibility: the name and the
/// character after it.
fn module_item(trimmed: &str) -> Option<(&str, char)> {
    let mut rest = trimmed;
    if let Some(after) = rest.strip_prefix("pub(") {
        rest = after.split_once(')')?.1;
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        rest = rest.trim_start();
    } else if let Some(after) = rest.strip_prefix("pub") {
        if !after.starts_with(char::is_whitespace) {
            return None;
        }
        rest = after.trim_start();
    }
    let after_mod = rest.strip_prefix("mod")?;
    if !after_mod.starts_with(char::is_whitespace) {
        return None;
    }
    let after_mod = after_mod.trim_start();
    let end = after_mod
        .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .unwrap_or(after_mod.len());
    if end == 0 {
        return None;
    }
    let next = after_mod[end..].trim_start().chars().next()?;
    matches!(next, ';' | '{').then_some((&after_mod[..end], next))
}

fn path_attribute(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("#[path")?.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let (value, tail) = rest.split_once('"')?;
    (tail.trim() == "]").then_some(value)
}

fn include_literal(trimmed: &str) -> Option<&str> {
    let rest = trimmed.split_once("include!(")?.1.trim_start();
    let rest = rest.strip_prefix('"')?;
    let (value, tail) = rest.split_once('"')?;
    tail.trim_start().starts_with(')').then_some(value)
}

fn parent_dir(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

fn join(base: &str, names: &[&str], last: &str) -> String {
    let mut parts = vec![base];
    parts.extend_from_slice(names);
    parts.push(last);
    parts.join("/")
}

fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

// Copied unchanged from tests/generality_guardrails.rs (not shared, by the
// Issue's scope).

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_rust_source_files(root, &mut out);
    out.sort();
    out
}

fn collect_rust_source_files(root: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", root.display()))
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RustLineClass {
    Production,
    Test,
}

fn classified_rust_lines(text: &str) -> Vec<(&str, RustLineClass)> {
    let mut out = Vec::new();
    let mut skip_next_cfg_test_item = false;
    let mut skipping_cfg_test_block = false;
    let mut test_mod_to_eof = false;
    let mut block_depth = 0i32;

    for line in text.lines() {
        let trimmed = line.trim();
        if test_mod_to_eof {
            out.push((line, RustLineClass::Test));
            continue;
        }
        if skipping_cfg_test_block {
            out.push((line, RustLineClass::Test));
            block_depth += brace_delta(line);
            if block_depth <= 0 {
                skipping_cfg_test_block = false;
                block_depth = 0;
            }
            continue;
        }
        if skip_next_cfg_test_item {
            if trimmed.starts_with("#[") {
                out.push((line, RustLineClass::Test));
                continue;
            }
            if trimmed.starts_with("mod tests") {
                out.push((line, RustLineClass::Test));
                test_mod_to_eof = true;
                skip_next_cfg_test_item = false;
                continue;
            }
            out.push((line, RustLineClass::Test));
            let delta = brace_delta(line);
            if delta > 0 {
                skipping_cfg_test_block = true;
                block_depth = delta;
                if block_depth <= 0 {
                    skipping_cfg_test_block = false;
                }
            }
            skip_next_cfg_test_item = false;
            continue;
        }
        if trimmed == "#[cfg(test)]" {
            out.push((line, RustLineClass::Test));
            skip_next_cfg_test_item = true;
            continue;
        }
        out.push((line, RustLineClass::Production));
    }
    out
}

fn brace_delta(line: &str) -> i32 {
    line.chars().fold(0, |depth, ch| match ch {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}
