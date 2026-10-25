use std::path::Path;

use tempfile::TempDir;

use super::append_repair_guidance;
use crate::planner::repair::{RepairContext, build_repair_prompt_with_context};
use crate::planner::verify::VerificationReport;
use crate::verification::diagnostics::CompileError;

const GAME_LOGIC: &str =
    "export const CONFIG = {};\nexport function createGameState() {}\nexport function tick() {}\n";
const HOOK: &str = "import { createInitialState } from '../lib/gameLogic';\n";

fn workspace() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/lib")).unwrap();
    std::fs::create_dir_all(dir.path().join("src/hooks")).unwrap();
    std::fs::write(dir.path().join("src/lib/gameLogic.ts"), GAME_LOGIC).unwrap();
    std::fs::write(dir.path().join("src/hooks/useGameLoop.ts"), HOOK).unwrap();
    dir
}

fn tsc_report(reason: &str) -> VerificationReport {
    let mut report = VerificationReport::pass();
    report.push_command_failure("npx tsc --noEmit", reason);
    report
}

fn compile_report(path: &str, message: &str) -> VerificationReport {
    let mut report = VerificationReport::pass();
    report.compile_errors.push(CompileError {
        path: path.to_string(),
        line: 1,
        column: 1,
        message: message.to_string(),
        excerpt: String::new(),
        symbol: None,
        route_bound: None,
    });
    report
}

fn guidance(report: &VerificationReport, root: &Path) -> String {
    let mut prompt = String::new();
    append_repair_guidance(&mut prompt, report, Some(root));
    prompt
}

const TS2305: &str = "src/hooks/useGameLoop.ts(1,10): error TS2305: Module '\"../lib/gameLogic\"' has no exported member 'createInitialState'.";

#[test]
fn ts2305_tsc_line_lists_target_exports() {
    let dir = workspace();
    let prompt = guidance(&tsc_report(TS2305), dir.path());
    assert!(
        prompt.contains("src/lib/gameLogic.ts exports: CONFIG, createGameState, tick"),
        "{prompt}"
    );
}

#[test]
fn ts2724_named_member_lists_target_exports() {
    let dir = workspace();
    let reason = "src/hooks/useGameLoop.ts(1,10): error TS2724: '\"../lib/gameLogic\"' has no exported member named 'createInitialState'. Did you mean 'createGameState'?";
    let prompt = guidance(&tsc_report(reason), dir.path());
    assert!(
        prompt.contains("src/lib/gameLogic.ts exports: CONFIG, createGameState, tick"),
        "{prompt}"
    );
}

#[test]
fn next_build_compile_error_lists_target_exports() {
    let dir = workspace();
    let report = compile_report(
        "src/hooks/useGameLoop.ts",
        "TS2305: Module '\"../lib/gameLogic\"' has no exported member 'createInitialState'.",
    );
    let prompt = guidance(&report, dir.path());
    assert!(
        prompt.contains("src/lib/gameLogic.ts exports: CONFIG, createGameState, tick"),
        "{prompt}"
    );
}

#[test]
fn more_than_thirty_exports_are_truncated() {
    let dir = workspace();
    let many = (0..35)
        .map(|index| format!("export const name{index:02} = {index};\n"))
        .collect::<String>();
    std::fs::write(dir.path().join("src/lib/gameLogic.ts"), many).unwrap();
    let prompt = guidance(&tsc_report(TS2305), dir.path());
    assert!(prompt.contains("(+5 more)"), "{prompt}");
    assert!(prompt.contains("name29"), "{prompt}");
    assert!(!prompt.contains("name30"), "{prompt}");
}

#[test]
fn unresolvable_specifiers_add_nothing() {
    let dir = workspace();
    for specifier in [
        "react",
        "../lib/missing",
        "../../../outside",
        "@/lib/gameLogic",
    ] {
        let reason = format!(
            "src/hooks/useGameLoop.ts(1,10): error TS2305: Module '\"{specifier}\"' has no exported member 'x'."
        );
        assert_eq!(
            guidance(&tsc_report(&reason), dir.path()),
            "",
            "{specifier}"
        );
    }
}

#[test]
fn target_outside_root_is_skipped() {
    let outer = tempfile::tempdir().unwrap();
    std::fs::write(outer.path().join("secret.ts"), "export const hidden = 1;\n").unwrap();
    let root = outer.path().join("work");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/a.ts"), "import { x } from '../../secret';\n").unwrap();
    let reason =
        "src/a.ts(1,10): error TS2305: Module '\"../../secret\"' has no exported member 'x'.";
    assert_eq!(guidance(&tsc_report(reason), &root), "");
}

#[test]
fn duplicate_diagnostics_are_listed_once() {
    let dir = workspace();
    let reason = format!("{TS2305}\nsummary: {TS2305}\n{TS2305}");
    let mut report = tsc_report(&reason);
    report.compile_errors = compile_report(
        "src/hooks/useGameLoop.ts",
        "TS2305: Module '\"../lib/gameLogic\"' has no exported member 'other'.",
    )
    .compile_errors;
    let prompt = guidance(&report, dir.path());
    assert_eq!(
        prompt.matches("src/lib/gameLogic.ts exports:").count(),
        1,
        "{prompt}"
    );
}

#[test]
fn repair_prompt_carries_exports_guidance() {
    let dir = workspace();
    let context = RepairContext {
        workspace_root: Some(dir.path().to_path_buf()),
        ..RepairContext::default()
    };
    let prompt = build_repair_prompt_with_context("verify", &tsc_report(TS2305), &context);
    assert!(
        prompt.contains("src/lib/gameLogic.ts exports: CONFIG, createGameState, tick"),
        "{prompt}"
    );
}
