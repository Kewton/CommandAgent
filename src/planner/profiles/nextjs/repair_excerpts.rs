use std::path::{Path, PathBuf};

pub(crate) fn profile_invariant_relevant_paths(
    root: &Path,
    profile: &str,
    reason: &str,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !super::matches_profile(profile) {
        return out;
    }
    let tailwind_failure = reason.contains("tailwind_contract_failure");
    let rels = crate::planner::repair_targeting::profile_invariant_excerpt_candidates(
        root,
        profile,
        tailwind_failure,
    );
    for project_root in profile_excerpt_project_roots(root) {
        for rel in &rels {
            let path = project_root.join(rel);
            if path.is_file() && !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

fn profile_excerpt_project_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![root.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("package.json").is_file() && !roots.contains(&path) {
                roots.push(path);
            }
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_nextjs_repair_excerpts_preserve_profile_filtering() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("nested-app");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("package.json"), "{}").unwrap();
        std::fs::write(app.join("tailwind.config.ts"), "export default {}").unwrap();

        let paths =
            profile_invariant_relevant_paths(dir.path(), "next-js", "tailwind_contract_failure");
        assert!(paths.contains(&app.join("package.json")));
        assert!(paths.contains(&app.join("tailwind.config.ts")));
        assert!(profile_invariant_relevant_paths(dir.path(), "data", "failure").is_empty());
    }
}
