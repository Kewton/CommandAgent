//! Repair guidance for TS2305/TS2724 ("has no exported member") reported by
//! `tsc` or `next build`: lists the names the target module really exports,
//! reusing the import-scan helpers so wording and limits match.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::minimal_loop::import_scan::{
    available_export_names, format_available_exports, resolve_import_for_source,
};
use crate::planner::verify::VerificationReport;

static TSC_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"([^\s()'"]+\.(?:ts|tsx|mts|cts|js|jsx))[(:]\d+[,:]\d+\)?[: -]+error TS(?:2305|2724): (?:Module )?'"([^"]+)"' has no exported member"#,
    )
    .expect("tsc missing export format")
});

static COMPILE_MESSAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"'"([^"]+)"' has no exported member"#).expect("compile missing export format")
});

pub(crate) fn append_repair_guidance(
    prompt: &mut String,
    report: &VerificationReport,
    root: Option<&Path>,
) {
    let Some(root) = root.and_then(|root| root.canonicalize().ok()) else {
        return;
    };
    let mut occurrences: Vec<(String, String)> = Vec::new();
    for failure in &report.command_failures {
        for captures in TSC_LINE.captures_iter(&failure.reason) {
            occurrences.push((captures[1].to_string(), captures[2].to_string()));
        }
    }
    for error in &report.compile_errors {
        if let Some(captures) = COMPILE_MESSAGE.captures(&error.message) {
            occurrences.push((error.path.clone(), captures[1].to_string()));
        }
    }
    let mut seen = BTreeSet::new();
    let lines = occurrences
        .iter()
        .filter_map(|(source, specifier)| exports_line(&root, source, specifier))
        .filter(|line| seen.insert(line.clone()))
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return;
    }
    prompt.push_str("\n\nAvailable exports for unresolved imports:\n");
    for line in lines {
        prompt.push_str(&format!("- {line}\n"));
    }
}

fn exports_line(root: &Path, source: &str, specifier: &str) -> Option<String> {
    let source_path = root.join(source).canonicalize().ok()?;
    if !source_path.starts_with(root) || !source_path.is_file() {
        return None;
    }
    let definition: PathBuf = resolve_import_for_source(root, &source_path, specifier)
        .into_iter()
        .find(|candidate| candidate.is_file())?
        .canonicalize()
        .ok()?;
    let relative = definition.strip_prefix(root).ok()?;
    let display = relative.to_string_lossy().replace('\\', "/");
    Some(format_available_exports(
        &display,
        &available_export_names(&definition),
    ))
}

#[cfg(test)]
#[path = "ts_missing_export_tests.rs"]
mod tests;
