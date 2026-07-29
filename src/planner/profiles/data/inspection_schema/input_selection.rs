use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::planner::failure_vocabulary::ViolationId;

use super::input_table::{self, InputTable};

const GOAL_SCAN_SKIPS: [&str; 5] = [".anvil", ".git", ".next", "node_modules", "target"];
const NAME_INPUT_GUIDANCE: &str = "goalで入力を名指しせよ";

pub(super) fn load(root: &Path, goal: Option<&str>) -> Result<InputTable, String> {
    let named = goal
        .filter(|goal| !goal.trim().is_empty())
        .map(|goal| goal_inputs(root, goal))
        .transpose()?
        .unwrap_or_default();
    let paths = if named.is_empty() {
        fallback_input(root)?
    } else {
        named
    };
    load_set(root, paths)
}

fn goal_inputs(root: &Path, goal: &str) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();
    collect_tables(root, root, &mut candidates, true).map_err(|error| {
        ViolationId::inspection_schema(format!("input_scan:{error}")).to_string()
    })?;
    candidates.sort();
    candidates.dedup();
    Ok(candidates
        .into_iter()
        .filter(|path| goal_names_path(root, goal, path))
        .collect())
}

fn fallback_input(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for directory in ["data", "input"] {
        collect_tables(root, &root.join(directory), &mut files, false).map_err(|error| {
            ViolationId::inspection_schema(format!("input_scan:{error}")).to_string()
        })?;
    }
    files.sort();
    files.dedup();
    match files.len() {
        1 => Ok(files),
        0 => Err("inspection_schema_violation:input_missing".to_string()),
        _ => Err(ViolationId::inspection_schema(format!(
            "multiple_inputs:{}:guidance={NAME_INPUT_GUIDANCE}",
            display_paths(root, &files).join(",")
        ))
        .to_string()),
    }
}

fn load_set(root: &Path, paths: Vec<PathBuf>) -> Result<InputTable, String> {
    let mut relative_paths = Vec::new();
    let mut headers = Vec::new();
    let mut seen_headers = BTreeSet::new();
    let mut row_count = 0u64;
    for path in paths {
        let table = input_table::load(root, path)?;
        relative_paths.push(table.relative_path);
        row_count = row_count
            .checked_add(table.row_count)
            .ok_or_else(|| "inspection_schema_violation:input_row_count_overflow".to_string())?;
        for header in table.headers {
            if seen_headers.insert(header.clone()) {
                headers.push(header);
            }
        }
    }
    Ok(InputTable {
        relative_path: relative_paths.join(","),
        headers,
        row_count,
    })
}

fn collect_tables(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
    skip_runtime_directories: bool,
) -> std::io::Result<()> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if skip_runtime_directories
                && GOAL_SCAN_SKIPS.contains(&entry.file_name().to_string_lossy().as_ref())
            {
                continue;
            }
            collect_tables(root, &entry.path(), files, skip_runtime_directories)?;
        } else if file_type.is_file() && table_path(&entry.path()) {
            let path = entry.path();
            if path.starts_with(root) {
                files.push(path);
            }
        }
    }
    Ok(())
}

fn table_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "csv" | "tsv"))
}

fn goal_names_path(root: &Path, goal: &str, path: &Path) -> bool {
    let relative = crate::tools::path_guard::relative_display(root, path);
    let absolute = path.to_string_lossy();
    [relative.as_str(), absolute.as_ref()]
        .into_iter()
        .any(|candidate| path_span(goal, candidate))
        || path_span(goal, &format!("./{relative}"))
}

fn path_span(goal: &str, candidate: &str) -> bool {
    goal.match_indices(candidate).any(|(start, matched)| {
        let before = goal[..start].chars().next_back();
        let after = goal[start + matched.len()..].chars().next();
        before.is_none_or(|ch| !ascii_path_char(ch)) && after.is_none_or(|ch| !ascii_path_char(ch))
    })
}

fn ascii_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '/' | '\\')
}

fn display_paths(root: &Path, paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| crate::tools::path_guard::relative_display(root, path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN5_INPUT: &str = include_str!(
        "../../../../../tests/corpus/apps/test0716_data13_goal_input/fixtures/data8_ts_qwen35_none_002/data/sales.csv"
    );
    const RUN5_DERIVED: &str = include_str!(
        "../../../../../tests/corpus/apps/test0716_data13_goal_input/fixtures/data8_ts_qwen35_none_002/data/sales_clean.csv"
    );
    const RUN5_VALIDATION: &str = include_str!(
        "../../../../../tests/corpus/apps/test0716_data13_goal_input/fixtures/data8_ts_qwen35_none_002/data/validation_log.csv"
    );
    const RUN5_INSPECTION: &str = include_str!(
        "../../../../../tests/corpus/apps/test0716_data13_goal_input/fixtures/data8_ts_qwen35_none_002/output/inspection.json"
    );
    const RUN5_GOAL: &str = "data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。";

    #[test]
    fn measured_run5_derived_tables_do_not_displace_goal_input() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data")).unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(dir.path().join("data/sales.csv"), RUN5_INPUT).unwrap();
        std::fs::write(dir.path().join("data/sales_clean.csv"), RUN5_DERIVED).unwrap();
        std::fs::write(dir.path().join("data/validation_log.csv"), RUN5_VALIDATION).unwrap();
        std::fs::write(
            dir.path().join(super::super::INSPECTION_PATH),
            RUN5_INSPECTION,
        )
        .unwrap();

        let step = crate::planner::step_plan::PlanStep {
            id: "verify-inspection".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify inspection schema.".to_string(),
            expected_paths: vec![super::super::INSPECTION_PATH.to_string()],
            verify: vec![super::super::super::step_policy::catalog_check_command(
                "data_inspection_schema",
            )],
        };
        let (report, _) = crate::planner::verify::verify_step_with_context(
            dir.path(),
            &step,
            Some("data"),
            Some(RUN5_GOAL),
            crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority::None,
            true,
            None,
        );
        let evidence: super::super::InspectionSchemaEvidence = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(super::super::EVIDENCE_PATH)).unwrap(),
        )
        .unwrap();

        assert_eq!(evidence.input_path.as_deref(), Some("data/sales.csv"));
        assert!(!report.primary_reason().contains("multiple_inputs"));
        assert!(
            evidence
                .failure_kinds
                .iter()
                .all(|failure| !failure.contains("multiple_inputs")
                    && !failure.contains("input_row_count_mismatch")),
            "{evidence:?}"
        );
    }

    #[test]
    fn multiple_named_inputs_form_one_checked_input_set() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data")).unwrap();
        std::fs::write(dir.path().join("data/a.csv"), "left,value\nA,1\n").unwrap();
        std::fs::write(dir.path().join("data/b.tsv"), "right\tvalue\nB\t2\nC\t3\n").unwrap();

        let input = load(dir.path(), Some("Join data/a.csv and data/b.tsv.")).unwrap();

        assert_eq!(input.relative_path, "data/a.csv,data/b.tsv");
        assert_eq!(input.headers, ["left", "value", "right"]);
        assert_eq!(input.row_count, 3);
    }

    #[test]
    fn unnamed_multiple_inputs_keep_deterministic_failure() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data")).unwrap();
        std::fs::write(dir.path().join("data/a.csv"), "value\n1\n").unwrap();
        std::fs::write(dir.path().join("data/b.csv"), "value\n2\n").unwrap();

        let Err(failure) = load(dir.path(), Some("Inspect the available tables.")) else {
            panic!("unnamed input set must remain ambiguous");
        };
        assert_eq!(
            failure,
            "inspection_schema_violation:multiple_inputs:data/a.csv,data/b.csv:guidance=goalで入力を名指しせよ"
        );
    }
}
