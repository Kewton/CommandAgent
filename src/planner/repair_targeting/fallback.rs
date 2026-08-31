use std::path::Path;

use crate::planner::profile::ProfileId;

pub(super) fn non_next_candidates(root: &Path, profile: &str) -> Vec<String> {
    let candidates: &[&str] = match ProfileId::parse(profile) {
        ProfileId::Data => &[
            "pipeline/main.py",
            "pipeline.py",
            "scripts/repro.py",
            "scripts/transform.py",
            "main.py",
        ],
        ProfileId::Ingest => &["ingest.py", "src/ingest.py", "main.py"],
        _ => &[
            "app.py",
            "main.py",
            "cli.py",
            "src/main.py",
            "src/main.rs",
            "src/lib.rs",
        ],
    };
    candidates
        .iter()
        .filter(|candidate| root.join(candidate).is_file())
        .map(|candidate| (*candidate).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_prefers_existing_python_entrypoint() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("app.py"), "print('ok')\n").unwrap();

        assert_eq!(non_next_candidates(root.path(), "generic"), vec!["app.py"]);
    }

    #[test]
    fn data_uses_existing_pipeline_source() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("pipeline")).unwrap();
        std::fs::write(root.path().join("pipeline/main.py"), "pass\n").unwrap();

        assert_eq!(
            non_next_candidates(root.path(), "data"),
            vec!["pipeline/main.py"]
        );
    }
}
