use crate::agent::step_runner::profiles::{
    ProfileObligation, ProfileObligationContext, profile_fact_summary, profile_obligations,
    render_profile_obligations,
};
use serde_json::Value;
use std::collections::BTreeSet;
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
    pub(crate) profile_obligations: Vec<ProfileObligation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ActiveStepContract {
    pub(crate) profile: String,
    pub(crate) base_phase_contract_facts: Vec<String>,
    pub(crate) profile_obligations: Vec<ProfileObligation>,
    pub(crate) current_profile_facts: Vec<String>,
}

impl ActiveStepContract {
    #[cfg(test)]
    pub(crate) fn empty(profile: &str) -> Self {
        Self {
            profile: profile.to_string(),
            ..Self::default()
        }
    }

    pub(crate) fn from_phase_contract(
        profile: &str,
        phase_contract: &PhaseWorkspaceContract,
        current_profile_facts: Vec<String>,
    ) -> Self {
        Self {
            profile: profile.to_string(),
            base_phase_contract_facts: phase_contract.base_fact_lines(),
            profile_obligations: phase_contract.profile_obligations.clone(),
            current_profile_facts,
        }
    }

    pub(crate) fn with_current_profile_facts(&self, current_profile_facts: Vec<String>) -> Self {
        Self {
            current_profile_facts,
            ..self.clone()
        }
    }

    pub(crate) fn rendered_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let mut seen = BTreeSet::new();
        for line in self
            .base_phase_contract_facts
            .iter()
            .cloned()
            .chain(render_profile_obligations(&self.profile_obligations))
            .chain(self.current_profile_facts.iter().cloned())
        {
            push_unique_bounded(&mut lines, &mut seen, line);
            if lines.len() >= MAX_RENDER_LINES {
                break;
            }
        }
        lines
    }
}

impl PhaseWorkspaceContract {
    #[cfg(test)]
    pub(crate) fn collect(cwd: &Path, profile: &str, required_artifacts: &[String]) -> Self {
        Self::collect_with_goal(cwd, profile, required_artifacts, "")
    }

    pub(crate) fn collect_with_goal(
        cwd: &Path,
        profile: &str,
        required_artifacts: &[String],
        goal_excerpt: &str,
    ) -> Self {
        let mut contract = Self {
            workspace_entries: workspace_entries(cwd),
            package_manager: package_manager(cwd),
            lockfiles: lockfiles(cwd),
            package_scripts: package_scripts(cwd),
            required_artifacts: required_artifacts.to_vec(),
            profile_summary: profile_fact_summary(profile, cwd)
                .map(|summary| summary.lines)
                .unwrap_or_default(),
            profile_obligations: Vec::new(),
        };
        let obligation_context = ProfileObligationContext {
            goal_excerpt: goal_excerpt.to_string(),
            required_artifacts: required_artifacts.to_vec(),
            phase_contract_facts: contract.base_fact_lines(),
            profile_facts: contract.profile_summary.clone(),
        };
        contract.profile_obligations =
            profile_obligations(profile, &obligation_context).unwrap_or_default();
        contract
    }

    pub(crate) fn fact_lines(&self) -> Vec<String> {
        let mut lines = self.base_fact_lines();
        lines.extend(render_profile_obligations(&self.profile_obligations));
        lines
    }

    pub(crate) fn base_fact_lines(&self) -> Vec<String> {
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

pub(crate) fn current_profile_facts(profile: &str, cwd: &Path) -> Vec<String> {
    profile_fact_summary(profile, cwd)
        .map(|summary| summary.lines)
        .unwrap_or_default()
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

fn push_unique_bounded(out: &mut Vec<String>, seen: &mut BTreeSet<String>, line: String) {
    let line = bounded_line(&line);
    if seen.insert(line.clone()) {
        out.push(line);
    }
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

    #[test]
    fn base_fact_lines_exclude_rendered_obligations() {
        let root = temp_workspace("base-facts-obligation");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"}}"#,
        )
        .unwrap();

        let contract = PhaseWorkspaceContract::collect_with_goal(
            &root,
            "nextjs",
            &["app/page.tsx".to_string()],
            "Create a Next.js app on port 3011",
        );

        assert!(
            contract
                .base_fact_lines()
                .iter()
                .any(|line| { line == "nextjs.scripts.dev=next dev -p 3011" })
        );
        assert!(
            !contract
                .base_fact_lines()
                .iter()
                .any(|line| line.starts_with("profile.obligation."))
        );
        assert!(
            contract
                .fact_lines()
                .iter()
                .any(|line| line.starts_with("profile.obligation.nextjs_dev_port_required="))
        );
    }

    #[test]
    fn includes_nextjs_profile_obligations_for_requested_port() {
        let root = temp_workspace("nextjs-obligation");

        let contract = PhaseWorkspaceContract::collect_with_goal(
            &root,
            "nextjs",
            &["app/page.tsx".to_string()],
            "Create a Next.js app on port 3011",
        );

        assert!(
            contract
                .profile_obligations
                .iter()
                .any(|obligation| { obligation.code == "nextjs_dev_port_required" })
        );
        assert!(
            contract
                .fact_lines()
                .iter()
                .any(|line| { line.starts_with("profile.obligation.nextjs_dev_port_required=") })
        );
    }

    #[test]
    fn generic_profile_renders_no_obligations() {
        let root = temp_workspace("generic-obligation");

        let contract = PhaseWorkspaceContract::collect_with_goal(
            &root,
            "generic",
            &["README.md".to_string()],
            "Create docs",
        );

        assert!(contract.profile_obligations.is_empty());
        assert!(
            !contract
                .fact_lines()
                .iter()
                .any(|line| line.starts_with("profile.obligation."))
        );
    }

    #[test]
    fn active_contract_renders_obligation_once() {
        let root = temp_workspace("active-contract-once");
        let contract = PhaseWorkspaceContract::collect_with_goal(
            &root,
            "nextjs",
            &["app/page.tsx".to_string()],
            "Create a Next.js app on port 3011",
        );

        let active = ActiveStepContract::from_phase_contract(
            "nextjs",
            &contract,
            vec![
                "nextjs.app_root=app".to_string(),
                "nextjs.app_root=app".to_string(),
            ],
        );

        let rendered = active.rendered_lines();
        assert_eq!(
            rendered
                .iter()
                .filter(|line| line.starts_with("profile.obligation.nextjs_dev_port_required="))
                .count(),
            1
        );
        assert_eq!(
            rendered
                .iter()
                .filter(|line| line.as_str() == "nextjs.app_root=app")
                .count(),
            1
        );
    }

    #[test]
    fn active_contract_empty_renders_no_lines() {
        let active = ActiveStepContract::empty("generic");

        assert!(active.rendered_lines().is_empty());
    }

    #[test]
    fn active_contract_preserves_first_seen_order() {
        let active = ActiveStepContract {
            profile: "generic".to_string(),
            base_phase_contract_facts: vec!["a=1".to_string(), "b=2".to_string()],
            profile_obligations: Vec::new(),
            current_profile_facts: vec!["c=3".to_string()],
        };

        assert_eq!(active.rendered_lines(), vec!["a=1", "b=2", "c=3"]);
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
