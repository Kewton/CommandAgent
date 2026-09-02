//! Semantic lint for verification assertions against the frozen task input.

use std::path::Path;

pub(crate) fn existing_input_absence_failure(
    command: &str,
    goal: &str,
    work_root: Option<&Path>,
) -> Option<String> {
    let root = work_root?;
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    let path = match tokens.as_slice() {
        ["test", "!", "-f", path] => path.trim_matches(['\'', '"']),
        _ => return None,
    };
    if path.is_empty()
        || Path::new(path).is_absolute()
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
        || !root.join(path).is_file()
        || goal_requests_path_removal(goal, path)
    {
        return None;
    }
    Some(format!(
        "verify command asserts that existing frozen input `{path}` is absent, but the task does not require removing it"
    ))
}

fn goal_requests_path_removal(goal: &str, path: &str) -> bool {
    let goal = goal.to_ascii_lowercase();
    let mentions_path = goal.contains(&path.to_ascii_lowercase());
    mentions_path
        && [
            "remove",
            "delete",
            "must not exist",
            "should not exist",
            "削除",
            "消去",
        ]
        .iter()
        .any(|term| goal.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_absence_assertion_for_existing_frozen_input() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("fixture")).unwrap();
        std::fs::write(root.path().join("fixture/control.json"), "{}\n").unwrap();

        let failure = existing_input_absence_failure(
            "test ! -f fixture/control.json",
            "Fix app.py without reading fixture/control.json",
            Some(root.path()),
        )
        .unwrap();

        assert!(failure.contains("existing frozen input `fixture/control.json`"));
    }

    #[test]
    fn permits_explicit_file_removal_contract() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("obsolete.txt"), "old\n").unwrap();

        assert_eq!(
            existing_input_absence_failure(
                "test ! -f obsolete.txt",
                "Delete obsolete.txt from the project",
                Some(root.path()),
            ),
            None
        );
    }
}
