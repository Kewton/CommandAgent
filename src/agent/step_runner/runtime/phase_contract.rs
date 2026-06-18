use crate::agent::step_runner::profiles::profile_fact_summary;
use serde_json::Value;
use std::fs;
use std::path::Path;

const MAX_WORKSPACE_ENTRIES: usize = 20;
const MAX_RENDER_LINES: usize = 40;
const MAX_LINE_CHARS: usize = 180;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PhaseWorkspaceContract {
    pub(crate) workspace_entries: Vec<String>,
    pub(crate) package_manager: Option<String>,
    pub(crate) lockfiles: Vec<String>,
    pub(crate) package_scripts: Vec<String>,
    pub(crate) required_artifacts: Vec<String>,
    pub(crate) profile_summary: Vec<String>,
}

impl PhaseWorkspaceContract {
    pub(crate) fn collect(cwd: &Path, profile: &str, required_artifacts: &[String]) -> Self {
        Self {
            workspace_entries: workspace_entries(cwd),
            package_manager: package_manager(cwd),
            lockfiles: lockfiles(cwd),
            package_scripts: package_scripts(cwd),
            required_artifacts: required_artifacts.to_vec(),
            profile_summary: profile_fact_summary(profile, cwd)
                .map(|summary| summary.lines)
                .unwrap_or_default(),
        }
    }

    pub(crate) fn fact_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!(
            "workspace.entries={}",
            join_or(&self.workspace_entries, "none")
        ));
        if let Some(manager) = &self.package_manager {
            lines.push(format!("package.manager={manager}"));
        }
        lines.push(format!("lockfiles={}", join_or(&self.lockfiles, "none")));
        if !self.package_scripts.is_empty() {
            lines.extend(self.package_scripts.iter().cloned());
        }
        lines.push(format!(
            "required_artifacts={}",
            join_or(&self.required_artifacts, "none")
        ));
        lines.extend(self.profile_summary.iter().cloned());
        lines
    }

    pub(crate) fn render(&self) -> String {
        let lines = self
            .fact_lines()
            .into_iter()
            .take(MAX_RENDER_LINES)
            .map(|line| format!("- {}", bounded_line(&line)))
            .collect::<Vec<_>>();
        if lines.is_empty() {
            "- none".to_string()
        } else {
            lines.join("\n")
        }
    }
}

fn workspace_entries(cwd: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(cwd) else {
        return Vec::new();
    };
    let mut names = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "node_modules" {
                None
            } else if entry.path().is_dir() {
                Some(format!("{name}/"))
            } else {
                Some(name)
            }
        })
        .collect::<Vec<_>>();
    names.sort();
    names.truncate(MAX_WORKSPACE_ENTRIES);
    names
}

fn lockfiles(cwd: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for path in [
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "Cargo.lock",
    ] {
        if cwd.join(path).exists() {
            out.push(path.to_string());
        }
    }
    out
}

fn package_manager(cwd: &Path) -> Option<String> {
    if cwd.join("package-lock.json").exists() {
        Some("npm".to_string())
    } else if cwd.join("pnpm-lock.yaml").exists() {
        Some("pnpm".to_string())
    } else if cwd.join("yarn.lock").exists() {
        Some("yarn".to_string())
    } else {
        None
    }
}

fn package_scripts(cwd: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(cwd.join("package.json")) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let Some(scripts) = json.get("scripts").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut out = scripts
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_str()
                .map(|value| format!("package.script.{key}={value}"))
        })
        .collect::<Vec<_>>();
    out.sort();
    out.truncate(8);
    out
}

fn join_or(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        empty.to_string()
    } else {
        values
            .iter()
            .map(|value| bounded_line(value))
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn bounded_line(value: &str) -> String {
    let mut out = value.chars().take(MAX_LINE_CHARS).collect::<String>();
    if value.chars().count() > MAX_LINE_CHARS {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn renders_empty_workspace_deterministically() {
        let root = temp_workspace("empty");

        let contract = PhaseWorkspaceContract::collect(&root, "generic", &[]);

        assert!(contract.render().contains("workspace.entries=none"));
        assert!(contract.render().contains("lockfiles=none"));
    }

    #[test]
    fn renders_lockfile_and_required_artifacts() {
        let root = temp_workspace("npm");
        fs::write(root.join("package-lock.json"), "{}").unwrap();

        let contract =
            PhaseWorkspaceContract::collect(&root, "generic", &["app/page.tsx".to_string()]);

        let rendered = contract.render();
        assert!(rendered.contains("package.manager=npm"));
        assert!(rendered.contains("lockfiles=package-lock.json"));
        assert!(rendered.contains("required_artifacts=app/page.tsx"));
    }

    #[test]
    fn includes_profile_summary_without_interpreting_it() {
        let root = temp_workspace("nextjs");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"}}"#,
        )
        .unwrap();

        let contract = PhaseWorkspaceContract::collect(&root, "nextjs", &[]);

        assert!(
            contract
                .fact_lines()
                .iter()
                .any(|line| { line == "nextjs.scripts.dev=next dev -p 3011" })
        );
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-phase-contract-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
