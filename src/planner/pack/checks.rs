use std::path::{Path, PathBuf};

use anyhow::Context;
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use regex::Regex;

use crate::planner::capability_catalog::PackInternalCheck;
use crate::tools::path_guard::resolve_existing;
use crate::tools::workspace_policy::{WorkspacePolicy, should_skip_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackCheckResult {
    pub(crate) id: &'static str,
    pub(crate) passed: bool,
    pub(crate) reasons: Vec<String>,
}

pub(crate) fn execute(root: &Path, check: &PackInternalCheck) -> anyhow::Result<PackCheckResult> {
    match check {
        PackInternalCheck::PathLayoutConforms {
            required,
            forbidden,
        } => path_layout_conforms(root, required, forbidden),
        PackInternalCheck::DesignTokensOnly {
            css_globs,
            tokens_file,
            allow,
        } => design_tokens_only(root, css_globs, tokens_file, allow),
        PackInternalCheck::LintConfigPresent { path, must_contain } => {
            lint_config_present(root, path, must_contain)
        }
    }
}

pub(crate) fn id(check: &PackInternalCheck) -> &'static str {
    match check {
        PackInternalCheck::PathLayoutConforms { .. } => "path_layout_conforms",
        PackInternalCheck::DesignTokensOnly { .. } => "design_tokens_only",
        PackInternalCheck::LintConfigPresent { .. } => "lint_config_present",
    }
}

fn path_layout_conforms(
    root: &Path,
    required: &[String],
    forbidden: &[String],
) -> anyhow::Result<PackCheckResult> {
    let paths = workspace_paths(root)?;
    let mut reasons = Vec::new();
    for pattern in required {
        let matcher = glob(pattern)?;
        if !paths.iter().any(|path| matcher.is_match(path)) {
            reasons.push(format!("required glob matched no path: {pattern}"));
        }
    }
    for pattern in forbidden {
        let matcher = glob(pattern)?;
        if let Some(path) = paths.iter().find(|path| matcher.is_match(path)) {
            reasons.push(format!(
                "forbidden glob matched {}: {pattern}",
                display(path)
            ));
        }
    }
    Ok(result("path_layout_conforms", reasons))
}

fn design_tokens_only(
    root: &Path,
    css_globs: &[String],
    tokens_file: &str,
    allow: &[String],
) -> anyhow::Result<PackCheckResult> {
    let token_path = resolve_existing(root, tokens_file)
        .with_context(|| format!("design token file is unavailable: {tokens_file}"))?;
    let paths = workspace_paths(root)?;
    let matchers = css_globs
        .iter()
        .map(|pattern| glob(pattern))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let color = Regex::new(r"(?i)#[0-9a-f]{3,8}\b|(?:rgb|hsl)a?\s*\([^)]*\)")?;
    let mut reasons = Vec::new();
    for relative in paths.iter().filter(|path| {
        matchers.iter().any(|matcher| matcher.is_match(path)) && root.join(path).is_file()
    }) {
        let path = root.join(relative);
        if same_file(&path, &token_path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            reasons.push(format!("CSS file is not UTF-8: {}", display(relative)));
            continue;
        };
        for (line_index, line) in content.lines().enumerate() {
            for matched in color.find_iter(line) {
                if !allow.iter().any(|literal| literal == matched.as_str()) {
                    reasons.push(format!(
                        "raw color literal {} at {}:{}",
                        matched.as_str(),
                        display(relative),
                        line_index + 1
                    ));
                    if reasons.len() == 64 {
                        return Ok(result("design_tokens_only", reasons));
                    }
                }
            }
        }
    }
    Ok(result("design_tokens_only", reasons))
}

fn lint_config_present(
    root: &Path,
    path: &str,
    must_contain: &[String],
) -> anyhow::Result<PackCheckResult> {
    let config = match resolve_existing(root, path) {
        Ok(config) if config.is_file() => config,
        _ => {
            return Ok(result(
                "lint_config_present",
                vec![format!("lint config file is missing: {path}")],
            ));
        }
    };
    let content = match std::fs::read_to_string(&config) {
        Ok(content) => content,
        Err(_) => {
            return Ok(result(
                "lint_config_present",
                vec![format!("lint config file is not UTF-8: {path}")],
            ));
        }
    };
    let reasons = must_contain
        .iter()
        .filter(|literal| !content.contains(literal.as_str()))
        .map(|literal| format!("lint config is missing required literal: {literal}"))
        .collect();
    Ok(result("lint_config_present", reasons))
}

fn workspace_paths(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .parents(true)
        .follow_links(false)
        .build()
    {
        let entry = entry?;
        let path = entry.path();
        if path == root || should_skip_path(root, path, WorkspacePolicy::NormalTask) {
            continue;
        }
        if let Ok(relative) = path.strip_prefix(root) {
            paths.push(relative.to_path_buf());
        }
    }
    paths.sort();
    Ok(paths)
}

fn glob(pattern: &str) -> anyhow::Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(pattern).with_context(|| format!("invalid glob pattern: {pattern}"))?);
    builder.build().context("compile glob pattern")
}

fn result(id: &'static str, reasons: Vec<String>) -> PackCheckResult {
    PackCheckResult {
        id,
        passed: reasons.is_empty(),
        reasons,
    }
}

fn same_file(path: &Path, expected: &Path) -> bool {
    path.canonicalize().ok().as_deref() == Some(expected)
}

fn display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_layout_requires_and_forbids_glob_matches() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/app")).unwrap();
        std::fs::write(root.path().join("src/app/page.tsx"), "export default 1").unwrap();
        let pass = PackInternalCheck::PathLayoutConforms {
            required: vec!["src/app/*.tsx".to_string()],
            forbidden: vec!["pages/**".to_string()],
        };
        assert!(execute(root.path(), &pass).unwrap().passed);
        let fail = PackInternalCheck::PathLayoutConforms {
            required: vec!["components/**".to_string()],
            forbidden: vec!["src/app/**".to_string()],
        };
        let result = execute(root.path(), &fail).unwrap();
        assert!(!result.passed);
        assert_eq!(result.reasons.len(), 2);
    }

    #[test]
    fn design_tokens_rejects_raw_colors_outside_token_file_and_honors_allowlist() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/styles")).unwrap();
        std::fs::write(
            root.path().join("src/styles/tokens.css"),
            ":root{--ink:#112233}",
        )
        .unwrap();
        std::fs::write(
            root.path().join("src/styles/page.css"),
            ".a{color:var(--ink)}.b{color:#fff}.c{color:rgb(1,2,3)}",
        )
        .unwrap();
        let base = |allow| PackInternalCheck::DesignTokensOnly {
            css_globs: vec!["src/**/*.css".to_string()],
            tokens_file: "src/styles/tokens.css".to_string(),
            allow,
        };
        let failed = execute(root.path(), &base(vec!["#fff".to_string()])).unwrap();
        assert_eq!(failed.reasons.len(), 1);
        assert!(failed.reasons[0].contains("rgb("));
        assert!(
            execute(
                root.path(),
                &base(vec!["#fff".to_string(), "rgb(1,2,3)".to_string()])
            )
            .unwrap()
            .passed
        );
    }

    #[test]
    fn lint_config_requires_file_and_literals() {
        let root = tempfile::tempdir().unwrap();
        let check = PackInternalCheck::LintConfigPresent {
            path: "eslint.config.mjs".to_string(),
            must_contain: vec!["next/core-web-vitals".to_string()],
        };
        assert!(!execute(root.path(), &check).unwrap().passed);
        std::fs::write(
            root.path().join("eslint.config.mjs"),
            "export default ['next/core-web-vitals'];",
        )
        .unwrap();
        assert!(execute(root.path(), &check).unwrap().passed);
    }
}
