//! Conservative source/config references are exclusions, never writer authority.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use crate::minimal_loop::completion::CompletionContract;
use regex::Regex;

pub(super) fn referenced_configs(root: &Path, contract: &CompletionContract) -> BTreeSet<String> {
    let mut pending = Vec::new();
    let mut sources = crate::minimal_loop::import_scan::nextjs_route_bound_closure(root);
    collect_configs(root, root, &mut pending, &mut sources);
    pending.extend(
        contract
            .required_paths
            .iter()
            .chain(&contract.protected_paths)
            .filter(|path| path.ends_with(".json"))
            .map(PathBuf::from),
    );
    // Unlike grant recognition, conservative textual matches here only remove
    // authority. They also protect imports in files with unsupported syntax.
    static JSON_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?:\bfrom\s*|\bimport\s*\(\s*|\brequire\s*\(\s*|\bimport\s*)["']([^"']+\.json)["']"#,
        )
        .expect("valid JSON reference pattern")
    });
    for source in sources {
        if source.ancestors().any(|part| root.join(part).is_symlink()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&source)) else {
            continue;
        };
        if source
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains(".config."))
        {
            textual_json_references(
                &text,
                source.parent().unwrap_or(Path::new("")),
                &mut pending,
            );
        }
        for captures in JSON_IMPORT.captures_iter(&text) {
            let specifier = &captures[1];
            if let Some(tail) = specifier.strip_prefix("@/") {
                let prefix = source
                    .parent()
                    .unwrap_or(Path::new(""))
                    .components()
                    .take_while(|part| part.as_os_str() != "src" && part.as_os_str() != "app")
                    .collect::<PathBuf>();
                pending.push(prefix.join("src").join(tail));
                pending.push(prefix.join(tail));
            } else if specifier.starts_with('.') {
                pending.push(source.parent().unwrap_or(Path::new("")).join(specifier));
            }
        }
    }
    let mut configs = BTreeSet::new();
    while let Some(path) = pending.pop() {
        let Some(path) = normalize(&path) else {
            continue;
        };
        let relative = path.to_string_lossy().replace('\\', "/");
        if !configs.insert(relative) {
            continue;
        }
        // Never follow a source/config symlink while examining references.
        if path.ancestors().any(|part| root.join(part).is_symlink()) {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(root.join(&path)) {
            let parent = path.parent().unwrap_or(Path::new(""));
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                json_references(&value, parent, &mut pending);
            }
            // JSONC comments/trailing commas cannot erase reference protection.
            // These matches only add exclusions, including text in comments.
            textual_json_references(&text, parent, &mut pending);
        }
    }
    configs
}

fn collect_configs(
    root: &Path,
    directory: &Path,
    out: &mut Vec<PathBuf>,
    sources: &mut BTreeSet<PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.')
            || matches!(name.as_ref(), "node_modules" | "target" | "dist" | "build")
        {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect_configs(root, &entry.path(), out, sources);
        } else if kind.is_file()
            && super::is_nextjs_config_json(&name)
            && let Ok(relative) = entry.path().strip_prefix(root)
        {
            out.push(relative.to_path_buf());
        } else if kind.is_file()
            && name.contains(".config.")
            && matches!(
                entry.path().extension().and_then(|ext| ext.to_str()),
                Some("js" | "ts" | "mjs" | "cjs")
            )
            && let Ok(relative) = entry.path().strip_prefix(root)
        {
            sources.insert(relative.to_path_buf());
        }
    }
}

fn json_references(value: &serde_json::Value, parent: &Path, out: &mut Vec<PathBuf>) {
    match value {
        serde_json::Value::String(value) if value.ends_with(".json") => {
            out.push(parent.join(value))
        }
        serde_json::Value::Array(values) => {
            for value in values {
                json_references(value, parent, out);
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                json_references(value, parent, out);
                if key == "extends"
                    && let Some(value) = value.as_str()
                    && Path::new(value).extension().is_none()
                {
                    out.push(parent.join(format!("{value}.json")));
                }
            }
        }
        _ => {}
    }
}

fn textual_json_references(text: &str, parent: &Path, out: &mut Vec<PathBuf>) {
    static STRING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#""(?:\\.|[^"\\])*"|'[^']*'"#).expect("valid quoted reference pattern")
    });
    static EXTENDS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"["']extends["']\s*:\s*(["'][^"']+["'])"#).expect("valid extends pattern")
    });
    for value in STRING.find_iter(text).map(|value| value.as_str()) {
        let value = serde_json::from_str::<String>(value)
            .unwrap_or_else(|_| value[1..value.len() - 1].to_string());
        if value.ends_with(".json") {
            out.push(parent.join(value));
        }
    }
    for capture in EXTENDS.captures_iter(text) {
        let quoted = &capture[1];
        let value = serde_json::from_str::<String>(quoted)
            .unwrap_or_else(|_| quoted[1..quoted.len() - 1].to_string());
        if Path::new(&value).extension().is_none() {
            out.push(parent.join(format!("{value}.json")));
        }
    }
}

fn normalize(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir if normalized.pop() => {}
            _ => return None,
        }
    }
    Some(normalized)
}
