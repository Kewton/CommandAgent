//! Map diagnosed callers to local shared declaration files. These are source
//! relationships to inspect, not a claim that each import caused a failure.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::minimal_loop::build_verifier::CompileError;
use crate::minimal_loop::import_scan::resolve_import_for_source;
use crate::tools::path_guard::{resolve_existing, validate_workspace_relative};
use crate::tools::workspace_policy::{WorkspacePolicy, ensure_tool_path_allowed};

pub(super) fn render(root: &Path, errors: &[CompileError]) -> String {
    let Ok(root) = root.canonicalize() else {
        return String::new();
    };
    let imports =
        regex::Regex::new(r#"(?m)^\s*import\s+(?:type\s+)?\{([^}]+)\}\s+from\s+["']([^"']+)["']"#)
            .expect("named import declaration");
    let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
    for source in errors.iter().map(|e| &e.path).collect::<BTreeSet<_>>() {
        if !allowed(&root, source) {
            continue;
        }
        let Ok(file) = resolve_existing(&root, source) else {
            continue;
        };
        if !file
            .metadata()
            .is_ok_and(|m| m.is_file() && m.len() <= 128 * 1024)
        {
            continue;
        }
        if !file
            .strip_prefix(&root)
            .ok()
            .is_some_and(|p| allowed(&root, &p.to_string_lossy()))
        {
            continue;
        }
        if !matches!(
            file.extension().and_then(|s| s.to_str()),
            Some("ts" | "tsx")
        ) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for c in imports.captures_iter(&text) {
            // Resolve paths without reading or copying imported file contents.
            for candidate in resolve_import_for_source(&root, &file, &c[2]) {
                let Ok(candidate) = candidate.canonicalize() else {
                    continue;
                };
                let Ok(relative) = candidate.strip_prefix(&root) else {
                    continue;
                };
                let relative = relative.to_string_lossy();
                if !allowed(&root, &relative)
                    || !matches!(
                        candidate.extension().and_then(|s| s.to_str()),
                        Some("ts" | "tsx")
                    )
                {
                    continue;
                }
                let names = c[1].split_whitespace().collect::<Vec<_>>().join(" ");
                groups
                    .entry(relative.to_string())
                    .or_default()
                    .insert(format!("{source}: import {{{names}}}"));
                break;
            }
        }
    }
    let mut out = String::new();
    for (definition, callers) in groups {
        out.push_str(&format!(
            "- Shared declaration file to inspect: {definition}; {}\n",
            callers.into_iter().collect::<Vec<_>>().join("; ")
        ));
    }
    out
}

fn allowed(root: &Path, relative: &str) -> bool {
    validate_workspace_relative(relative).is_ok()
        && !Path::new(relative)
            .components()
            .any(|p| p.as_os_str().to_string_lossy().starts_with(".env"))
        && ensure_tool_path_allowed(root, &root.join(relative), WorkspacePolicy::NormalTask).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn issue457_shared_import_hints_confine_source_and_definition_paths() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::fs::write(
            root.path().join("src/types.ts"),
            "export interface Shape { value: string }",
        )
        .unwrap();
        std::fs::write(
            outside.path().join("private.ts"),
            "export interface Private { secret: string }",
        )
        .unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            outside.path().join("private.ts"),
            root.path().join("src/private.ts"),
        )
        .unwrap();
        std::fs::write(
            root.path().join("src/page.ts"),
            "import { Shape } from './types';\nimport { Private } from './private';",
        )
        .unwrap();
        let error = CompileError {
            path: "src/page.ts".into(),
            line: 1,
            column: 1,
            message: "Type error".into(),
            excerpt: String::new(),
            symbol: None,
            route_bound: None,
        };
        let hints = render(root.path(), &[error]);
        assert!(hints.contains("src/types.ts"));
        assert!(hints.contains("Shape"));
        assert!(!hints.contains("private.ts"));
        assert!(!hints.contains("Private"));
    }
}
