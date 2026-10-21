use std::path::Path;

use super::{ArtifactObligationEvidence, WorkspaceEvidence};

pub(super) fn verification_repair_path(
    required_paths: &[String],
    artifact_obligations: &[ArtifactObligationEvidence],
    workspace: &WorkspaceEvidence,
) -> String {
    required_paths
        .iter()
        .find(|path| super::looks_like_test_file(path))
        .cloned()
        .unwrap_or_else(|| {
            let implementation_path =
                super::implementation_repair_path(required_paths, artifact_obligations, workspace);
            let stem = implementation_path
                .rsplit('/')
                .next()
                .unwrap_or("main")
                .rsplit_once('.')
                .map_or(
                    "main",
                    |(stem, _)| if stem.is_empty() { "main" } else { stem },
                );
            match implementation_extension(&implementation_path).as_deref() {
                Some("tsx" | "ts") => format!("tests/{stem}.test.ts"),
                Some("jsx" | "js") => format!("tests/{stem}.test.js"),
                Some("rs") => format!("tests/{stem}.rs"),
                _ => format!("tests/test_{stem}.py"),
            }
        })
}

fn implementation_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repair_path(required: &[&str]) -> String {
        let required = required
            .iter()
            .map(|path| path.to_string())
            .collect::<Vec<_>>();
        verification_repair_path(&required, &[], &WorkspaceEvidence::default())
    }

    #[test]
    fn uppercase_tsx_implementation_selects_a_typescript_test_stub() {
        assert_eq!(repair_path(&["App.TSX"]), "tests/App.test.ts");
    }

    #[test]
    fn extension_matching_is_case_insensitive() {
        assert_eq!(implementation_extension("page.Ts").as_deref(), Some("ts"));
        assert_eq!(implementation_extension("main.RS").as_deref(), Some("rs"));
        assert_eq!(implementation_extension("Makefile").as_deref(), None);
    }

    #[test]
    fn non_script_implementation_keeps_the_python_fallback() {
        assert_eq!(repair_path(&["main.py"]), "tests/test_main.py");
    }
}
