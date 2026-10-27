//! Repair guidance for a failed `node <script>` verify command: names the one
//! file the script statically loads (resolved with Node's lookup order) as the
//! write-pressure target, so the host does not fall back to an unrelated
//! first-changed path. Names nothing unless exactly one JavaScript file is
//! the answer; grep arguments are deliberately never used.

use std::collections::BTreeSet;
use std::path::{Component, Path};
use std::sync::LazyLock;

use regex::Regex;

use crate::planner::verify::{VerificationReport, shell_words_with_spans};

const SCRIPT_BYTE_LIMIT: u64 = 256 * 1024;
const RESOLUTION_SUFFIXES: [&str; 6] = ["", ".js", ".cjs", ".mjs", ".json", "/index.js"];
const SHELL_OPERATORS: [&str; 6] = ["&&", "||", ";", "|", "&", ">"];

static STATIC_SPECIFIER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:\brequire\s*\(\s*|\bimport\s*\(\s*|\bimport\s+|\bfrom\s+)['"]([^'"\n]+)['"]"#)
        .expect("static module specifier")
});

pub(crate) fn append_repair_guidance(
    prompt: &mut String,
    report: &VerificationReport,
    root: Option<&Path>,
) {
    let Some(root) = root else {
        return;
    };
    if prompt
        .lines()
        .any(|line| line.trim().starts_with("- write-pressure target: "))
    {
        return;
    }
    let Some(target) = resolve_target(root, report) else {
        return;
    };
    prompt.push_str(&format!(
        "\n\nVerify failure target:\n- write-pressure target: {target} (selection_reason=diagnosis_mapped)\n"
    ));
}

pub(crate) fn resolve_target(root: &Path, report: &VerificationReport) -> Option<String> {
    if report.command_failures.is_empty()
        || !report.missing_paths.is_empty()
        || !report.compile_errors.is_empty()
    {
        return None;
    }
    let root = root.canonicalize().ok()?;
    let targets = report
        .command_failures
        .iter()
        .map(|failure| node_script(&failure.command))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .map(|script| loaded_file(&root, &script))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut targets = targets.into_iter();
    let target = targets.next()?;
    targets.next().is_none().then_some(target)
}

/// The script of a plain `node [flags] <script> [args]` command; `None` for
/// anything else (inline code, compound commands, other programs).
fn node_script(command: &str) -> Option<String> {
    let words = shell_words_with_spans(command)?;
    if words
        .iter()
        .any(|word| SHELL_OPERATORS.contains(&word.value.as_str()))
    {
        return None;
    }
    let mut words = words.into_iter().map(|word| word.value);
    if words.next()? != "node" {
        return None;
    }
    loop {
        let word = words.next()?;
        match word.as_str() {
            "-e" | "-p" | "--eval" | "--print" | "-" => return None,
            "-r" | "--require" | "--import" | "--loader" | "--experimental-loader" => {
                words.next()?;
            }
            flag if flag.starts_with('-') => {}
            _ => return Some(word),
        }
    }
}

/// The single workspace-relative file the script statically loads, or `None`
/// when any relative specifier does not resolve to a nameable file.
fn loaded_file(root: &Path, script: &str) -> Option<String> {
    crate::tools::path_guard::validate_workspace_relative(script).ok()?;
    let script_path = root.join(script).canonicalize().ok()?;
    if !script_path.starts_with(root) || !script_path.is_file() {
        return None;
    }
    if std::fs::metadata(&script_path).ok()?.len() > SCRIPT_BYTE_LIMIT {
        return None;
    }
    let source = std::fs::read_to_string(&script_path).ok()?;
    let base = script_path.parent()?;
    let mut resolved = BTreeSet::new();
    for captures in STATIC_SPECIFIER.captures_iter(&source) {
        let specifier = &captures[1];
        if !(specifier.starts_with("./") || specifier.starts_with("../")) {
            continue;
        }
        let file = resolve_specifier(base, specifier)?;
        if file == script_path {
            return None;
        }
        resolved.insert(file);
    }
    let mut resolved = resolved.into_iter();
    let file = resolved.next()?;
    if resolved.next().is_some() {
        return None;
    }
    let relative = file.strip_prefix(root).ok()?;
    if relative.components().any(|component| {
        matches!(component, Component::Normal(name)
            if name == ".commandagent" || name == "node_modules")
    }) {
        return None;
    }
    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn resolve_specifier(base: &Path, specifier: &str) -> Option<std::path::PathBuf> {
    RESOLUTION_SUFFIXES.iter().find_map(|suffix| {
        let candidate = base.join(format!("{specifier}{suffix}"));
        candidate
            .is_file()
            .then(|| candidate.canonicalize().ok())
            .flatten()
    })
}

#[cfg(test)]
mod tests;
