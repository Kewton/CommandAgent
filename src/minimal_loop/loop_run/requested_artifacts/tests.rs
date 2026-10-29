use std::path::Path;

use super::super::{RunSessionOptions, effective_required_path_sources};
use super::{extract_requested_artifact_paths, looks_like_artifact_path};

/// The e-mid instruction the generic pilot recorded (Issue #905, IC-46).
const E_MID_INSTRUCTION: &str = include_str!(
    "../../../../tests/corpus/apps/requested-artifact-code-expressions/fixtures/recorded/instruction.ja.txt"
);

/// Backticked spans that read as code or quoted data, not as file paths.
const CODE_OR_DATA_SPANS: &[&str] = &[
    r#"parse_date("03/04/2026")"#,
    r#""31/12/2026""#,
    "03/04/2026",
    "a/b == c",
    r#"re.compile(r"\d+/\d+")"#,
    "curl http://x/y",
    "GET /api/items",
    "foo(bar/baz)",
    "$HOME/x",
    "*.py",
];

/// Backticked spans that name real paths, existing or still to be created.
const PATH_SPANS: &[&str] = &[
    "dates.py",
    "tests/test_dates.py",
    "src/app/page.tsx",
    "cli/main.py",
    "docs/日本語.md",
];

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

#[test]
fn e_mid_instruction_extracts_only_the_named_source_and_test_files() {
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(
        extract_requested_artifact_paths(empty.path(), E_MID_INSTRUCTION),
        vec!["dates.py", "tests/test_dates.py"]
    );

    let workspace = tempfile::tempdir().unwrap();
    write(
        workspace.path(),
        "dates.py",
        "def parse_date(text):\n    pass\n",
    );
    write(workspace.path(), "tests/test_dates.py", "import unittest\n");
    assert_eq!(
        extract_requested_artifact_paths(workspace.path(), E_MID_INSTRUCTION),
        vec!["dates.py", "tests/test_dates.py"]
    );
}

#[test]
fn backticked_code_expressions_and_quoted_data_are_not_inferred() {
    let dir = tempfile::tempdir().unwrap();
    let inferred: Vec<(&str, Vec<String>)> = CODE_OR_DATA_SPANS
        .iter()
        .map(|span| {
            let prompt = format!("Fix the bug shown by `{span}` in the workspace.");
            (*span, extract_requested_artifact_paths(dir.path(), &prompt))
        })
        .filter(|(_, paths)| !paths.is_empty())
        .collect();
    assert!(inferred.is_empty(), "inferred as paths: {inferred:?}");
}

#[test]
fn code_or_data_spans_pass_the_path_shape_check_alone() {
    // The shape check alone (a `/` or a known extension) takes every one of
    // these spans for a path; the code-or-data rule is what keeps them out.
    let shaped: Vec<&str> = CODE_OR_DATA_SPANS
        .iter()
        .copied()
        .filter(|span| looks_like_artifact_path(span))
        .collect();
    assert_eq!(shaped, CODE_OR_DATA_SPANS);
}

#[test]
fn urls_stay_excluded_from_backticked_inference() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = "See `https://example.com/docs/a.md` and `http://x/y`, then edit `src/lib.rs`.";
    assert_eq!(
        extract_requested_artifact_paths(dir.path(), prompt),
        vec!["src/lib.rs"]
    );
}

#[test]
fn backticked_real_paths_stay_obligations_whether_or_not_they_exist() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "dates.py", "");
    write(dir.path(), "tests/test_dates.py", "");
    for span in PATH_SPANS {
        let prompt = format!("Create or update `{span}`.");
        assert_eq!(
            extract_requested_artifact_paths(dir.path(), &prompt),
            vec![span.to_string()],
            "{span}"
        );
    }
    let new_file = "src/reports/summary_table.rs";
    assert!(!dir.path().join(new_file).exists());
    let prompt = format!("Add a new module `{new_file}` next to `src/app/page.tsx`.");
    assert_eq!(
        extract_requested_artifact_paths(dir.path(), &prompt),
        vec![new_file, "src/app/page.tsx"]
    );
}

#[test]
fn code_expressions_are_dropped_beside_real_paths_on_the_same_line() {
    let dir = tempfile::tempdir().unwrap();
    let prompt =
        "Make `cli/main.py` print `len(rows)/2` and keep `docs/日本語.md`; `1/2` is a ratio.";
    assert_eq!(
        extract_requested_artifact_paths(dir.path(), prompt),
        vec!["cli/main.py", "docs/日本語.md"]
    );
}

#[test]
fn required_final_artifacts_block_keeps_special_character_paths() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = "\
Required final artifacts:
- src/app/notes/[id]/page.tsx
- src/app/(shop)/cart/page.tsx
- `docs/日本語.md`
- 03/04/2026
";
    assert_eq!(
        extract_requested_artifact_paths(dir.path(), prompt),
        vec![
            "src/app/notes/[id]/page.tsx",
            "src/app/(shop)/cart/page.tsx",
            "docs/日本語.md",
            "03/04/2026",
        ]
    );
}

#[test]
fn free_text_inference_does_not_reach_explicit_or_contract_paths() {
    let dir = tempfile::tempdir().unwrap();
    let explicit = vec!["out/report (final).txt".to_string()];
    let contract = vec!["data/[raw]/rows,v2.csv".to_string()];
    let sources = effective_required_path_sources(
        dir.path(),
        &explicit,
        E_MID_INSTRUCTION,
        &contract,
        &RunSessionOptions::default(),
    );
    assert_eq!(sources.explicit_required_paths, explicit);
    assert_eq!(
        sources.prompt_extracted_paths,
        vec!["dates.py", "tests/test_dates.py"]
    );
    assert_eq!(sources.completion_contract_paths, contract);
    assert_eq!(
        sources.effective_required_paths,
        vec![
            "out/report (final).txt",
            "dates.py",
            "tests/test_dates.py",
            "data/[raw]/rows,v2.csv",
        ]
    );
}

#[test]
fn requested_artifact_path_extraction_rejects_escape_and_metadata_paths() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = "\
Required final artifacts:
- ../outside.txt
- /tmp/out.txt
- .anvil/session.json
- target/debug/app
- node_modules/pkg/index.js
- package.json
- src/app/page.tsx
";
    let paths = extract_requested_artifact_paths(dir.path(), prompt);
    assert_eq!(paths, vec!["package.json", "src/app/page.tsx"]);
}

#[test]
fn requested_artifact_path_extraction_rejects_backticked_escape() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = "Create `src/main.rs`, not `../main.rs` or `.anvil/log.json`.";
    let paths = extract_requested_artifact_paths(dir.path(), prompt);
    assert_eq!(paths, vec!["src/main.rs"]);
}

#[test]
#[cfg(unix)]
fn requested_artifact_path_extraction_rejects_symlink_escape() {
    let dir = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/tmp", dir.path().join("out")).unwrap();
    let prompt = "\
Required final artifacts:
- out/file.txt
- safe/file.txt
";
    let paths = extract_requested_artifact_paths(dir.path(), prompt);
    assert_eq!(paths, vec!["safe/file.txt"]);
}
