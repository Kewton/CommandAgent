use std::collections::VecDeque;
use std::path::Path;

use crate::config::Config;

const MAX_WORKSPACE_FILES: usize = 64;
const MAX_SCANNED_ENTRIES: usize = 1_024;
const MAX_SCAN_DEPTH: usize = 8;

const CLAIM_FORMAT_GUIDANCE: &str = r#"診断の主張は次の形式で書くこと（以下の値は形式例であり、必ず再現Rの実出力と実在ファイルから得た実観測値で置き換える）:
エラー引用: `ValueError: could not convert string to float: ''`
位置: pipeline/main.py:53
コード引用（実在コードのみ）:
```python
amount = float(row["amount"])
```
修正案・例示コードはコードブロックにせず、
『修正方針:』以下に文章で書くこと（照合対象外となる）。"#;

pub(super) fn diagnose_instruction(config: &Config, goal: &str) -> String {
    let files = workspace_files(&config.workspace_root);
    let observation = super::observed_failure::render(config);
    format!(
        "Read only existing workspace files and the executed reproducer output for {goal}; write output/diagnosis.md and do not modify the subject. This step exclusively owns output/diagnosis.md.\n\n{observation}\n\n{CLAIM_FORMAT_GUIDANCE}\n\n実在ファイル一覧（決定的な辞書順、最大{MAX_WORKSPACE_FILES}件。読み取り参照はこの一覧に限定）:\n{}\n一覧にないファイルを参照しないこと。",
        render_files(&files)
    )
}

fn workspace_files(root: &Path) -> Vec<String> {
    let mut pending = VecDeque::from([(root.to_path_buf(), 0usize)]);
    let mut files = Vec::new();
    let mut scanned = 0usize;
    'scan: while let Some((dir, depth)) = pending.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries = entries.flatten().collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if scanned >= MAX_SCANNED_ENTRIES {
                break 'scan;
            }
            scanned += 1;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                if depth < MAX_SCAN_DEPTH && !excluded_directory(&name) {
                    pending.push_back((path, depth + 1));
                }
            } else if file_type.is_file()
                && let Ok(relative) = path.strip_prefix(root)
            {
                files.push(normalized_path(relative));
            }
        }
    }
    files.sort();
    files.truncate(MAX_WORKSPACE_FILES);
    files
}

fn excluded_directory(name: &str) -> bool {
    matches!(name, "node_modules" | "target" | "vendor")
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn render_files(files: &[String]) -> String {
    if files.is_empty() {
        return "- none".to_string();
    }
    files
        .iter()
        .map(|path| format!("- {path}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn guidance_contains_literal_claim_form_and_only_existing_sorted_files() {
        let root = tempfile::tempdir().unwrap();
        for path in ["data", "evidence", "output", "pipeline", ".anvil/runs"] {
            std::fs::create_dir_all(root.path().join(path)).unwrap();
        }
        for (path, contents) in [
            ("pipeline/main.py", "amount = float(row[\"amount\"])\n"),
            ("output/results.json", "{}\n"),
            ("data/sales.csv", "amount\ninvalid\n"),
            ("evidence/investigation-run.json", "{}\n"),
            (".anvil/runs/events.jsonl", "{}\n"),
        ] {
            std::fs::write(root.path().join(path), contents).unwrap();
        }
        let instruction = diagnose_instruction(&config(root.path()), "pipeline failure");

        for expected in [
            "エラー引用: `ValueError: could not convert string to float: ''`",
            "位置: pipeline/main.py:53",
            "コード引用（実在コードのみ）:",
            "amount = float(row[\"amount\"])",
            "『修正方針:』以下に文章で書くこと",
            "- data/sales.csv",
            "- evidence/investigation-run.json",
            "- output/results.json",
            "- pipeline/main.py",
            "一覧にないファイルを参照しないこと。",
        ] {
            assert!(
                instruction.contains(expected),
                "missing {expected:?}\n{instruction}"
            );
        }
        assert!(!instruction.contains("output/inspection.json"));
        assert!(!instruction.contains(".anvil/runs/events.jsonl"));
        let data = instruction.find("- data/sales.csv").unwrap();
        let evidence = instruction
            .find("- evidence/investigation-run.json")
            .unwrap();
        let output = instruction.find("- output/results.json").unwrap();
        let pipeline = instruction.find("- pipeline/main.py").unwrap();
        assert!(data < evidence && evidence < output && output < pipeline);
    }

    #[test]
    fn workspace_inventory_is_bounded() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..80 {
            std::fs::write(root.path().join(format!("file-{index:03}.txt")), "x").unwrap();
        }
        let files = workspace_files(root.path());
        assert_eq!(files.len(), MAX_WORKSPACE_FILES);
        assert!(files.windows(2).all(|pair| pair[0] < pair[1]));
    }

    fn config(root: &Path) -> Config {
        Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            root.to_str().unwrap(),
            "--intent",
            "investigate",
            "--profile",
            "data",
        ]))
        .unwrap()
    }
}
