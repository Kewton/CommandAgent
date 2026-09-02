use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ignore::WalkBuilder;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::eval_events;
use crate::minimal_loop::completion::CompletionContract;
use crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
use crate::planner::verify::VerificationReport;

const MAX_SOURCE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Default)]
pub(crate) struct RecoveryFixSafety {
    enabled: bool,
    contract: Option<CompletionContract>,
    referenced_api: ReferencedApiSnapshot,
    initial_artifacts: ArtifactSnapshot,
}

#[derive(Debug, Clone, Default)]
struct ReferencedApiSnapshot {
    requirements: Vec<ReferencedApi>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ArtifactSnapshot {
    files: BTreeMap<String, ArtifactFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactFingerprint {
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ReferencedApi {
    owner_path: String,
    symbol: String,
    caller_paths: Vec<String>,
}

impl RecoveryFixSafety {
    pub(crate) fn capture(config: &Config, enabled: bool) -> anyhow::Result<Self> {
        if !enabled {
            return Ok(Self::default());
        }
        Ok(Self {
            enabled: true,
            contract: CompletionContract::load_for_config(config)?,
            referenced_api: ReferencedApiSnapshot::capture(&config.workspace_root),
            initial_artifacts: ArtifactSnapshot::capture(&config.workspace_root),
        })
    }

    pub(crate) fn verify(
        &self,
        config: &Config,
        fallback_goal: &str,
        setup_authority: NodeDependencySetupAuthority,
        changed_paths: &[String],
    ) -> VerificationReport {
        if !self.enabled {
            return VerificationReport::pass();
        }
        let mut report = self
            .contract
            .as_ref()
            .map(|contract| {
                contract
                    .verify_with_goal_observed_with_setup_authority(
                        &config.workspace_root,
                        fallback_goal,
                        setup_authority,
                        config.offline,
                    )
                    .0
            })
            .unwrap_or_else(VerificationReport::pass);
        let violations = self
            .referenced_api
            .violations(&config.workspace_root, changed_paths);
        for violation in &violations {
            report.push_command_failure(
                format!("api-preservation-postcondition:{}", violation.owner_path),
                format!(
                    "referenced API '{}' was removed; existing callers: {}. Restore the smallest compatible implementation without rewriting unrelated APIs",
                    violation.symbol,
                    violation.caller_paths.join(",")
                ),
            );
        }
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "recovery_fix_safety_verification",
                "registered_verify_commands": self.verify_commands(),
                "referenced_api_surface_count": self.referenced_api.requirements.len(),
                "referenced_api_violations": violations,
                "changed_paths": changed_paths,
                "ok": report.is_pass(),
            }),
        );
        report
    }

    pub(crate) fn verify_commands(&self) -> Vec<String> {
        self.contract
            .as_ref()
            .map(|contract| contract.verify_commands.clone())
            .unwrap_or_default()
    }

    pub(crate) fn artifact_checkpoint(&self, root: &Path) -> ArtifactSnapshot {
        if self.enabled {
            ArtifactSnapshot::capture(root)
        } else {
            ArtifactSnapshot::default()
        }
    }

    pub(crate) fn observed_artifact_changes(
        &self,
        root: &Path,
        checkpoint: &ArtifactSnapshot,
        reported_paths: &[String],
        stage: &str,
        eval_events_path: Option<&Path>,
    ) -> Vec<String> {
        if !self.enabled {
            return reported_paths.to_vec();
        }
        let observed_paths = checkpoint.changed_paths(&ArtifactSnapshot::capture(root));
        let reported = reported_paths.iter().cloned().collect::<BTreeSet<_>>();
        let observed = observed_paths.iter().cloned().collect::<BTreeSet<_>>();
        let no_op_reported_paths = reported.difference(&observed).cloned().collect::<Vec<_>>();
        let unreported_mutation_paths = observed.difference(&reported).cloned().collect::<Vec<_>>();
        eval_events::emit(
            eval_events_path,
            json!({
                "event": "recovery_product_mutation_observed",
                "stage": stage,
                "reported_changed_paths": reported_paths,
                "observed_changed_paths": observed_paths,
                "no_op_reported_paths": no_op_reported_paths,
                "unreported_mutation_paths": unreported_mutation_paths,
                "mutation_observed": !observed.is_empty(),
            }),
        );
        observed.into_iter().collect()
    }

    pub(crate) fn observed_changes_from_start(
        &self,
        root: &Path,
        reported_paths: &[String],
        eval_events_path: Option<&Path>,
    ) -> Vec<String> {
        self.observed_artifact_changes(
            root,
            &self.initial_artifacts,
            reported_paths,
            "initial",
            eval_events_path,
        )
    }
}

impl ArtifactSnapshot {
    fn capture(root: &Path) -> Self {
        let files = workspace_artifact_paths(root)
            .into_iter()
            .filter_map(|(relative, path)| {
                let metadata = path.metadata().ok()?;
                let bytes = std::fs::read(path).ok()?;
                Some((
                    relative,
                    ArtifactFingerprint {
                        bytes: metadata.len(),
                        sha256: format!("{:x}", Sha256::digest(bytes)),
                    },
                ))
            })
            .collect();
        Self { files }
    }

    fn changed_paths(&self, after: &Self) -> Vec<String> {
        let paths = self
            .files
            .keys()
            .chain(after.files.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        paths
            .into_iter()
            .filter(|path| self.files.get(path) != after.files.get(path))
            .collect()
    }
}

impl ReferencedApiSnapshot {
    fn capture(root: &Path) -> Self {
        let sources = python_sources(root);
        let mut requirements = Vec::new();
        for (owner_path, owner_source) in &sources {
            for symbol in top_level_functions(owner_source) {
                let caller_paths = sources
                    .iter()
                    .filter(|(caller_path, caller_source)| {
                        caller_path != owner_path
                            && caller_references(caller_source, owner_path, &symbol)
                    })
                    .map(|(path, _)| path.clone())
                    .collect::<Vec<_>>();
                if !caller_paths.is_empty() {
                    requirements.push(ReferencedApi {
                        owner_path: owner_path.clone(),
                        symbol,
                        caller_paths,
                    });
                }
            }
        }
        requirements.sort_by(|left, right| {
            (&left.owner_path, &left.symbol).cmp(&(&right.owner_path, &right.symbol))
        });
        Self { requirements }
    }

    fn violations(&self, root: &Path, changed_paths: &[String]) -> Vec<ReferencedApi> {
        let changed = changed_paths
            .iter()
            .map(|path| path.replace('\\', "/"))
            .collect::<BTreeSet<_>>();
        self.requirements
            .iter()
            .filter(|requirement| changed.contains(&requirement.owner_path))
            .filter(|requirement| {
                let source = std::fs::read_to_string(root.join(&requirement.owner_path)).ok();
                !source.is_some_and(|source| {
                    top_level_functions(&source)
                        .iter()
                        .any(|symbol| symbol == &requirement.symbol)
                })
            })
            .cloned()
            .collect()
    }
}

fn python_sources(root: &Path) -> Vec<(String, String)> {
    let mut sources = workspace_artifact_paths(root)
        .into_iter()
        .filter(|(_, path)| path.extension().and_then(|extension| extension.to_str()) == Some("py"))
        .filter_map(|(relative, path)| {
            std::fs::read_to_string(path)
                .ok()
                .map(|source| (relative, source))
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

fn workspace_artifact_paths(root: &Path) -> Vec<(String, std::path::PathBuf)> {
    let mut paths = Vec::new();
    let filter_root = root.to_path_buf();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .parents(true)
        .filter_entry(move |entry| !private_runtime_path(&filter_root, entry.path()))
        .build();
    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file()
            || entry
                .metadata()
                .is_ok_and(|metadata| metadata.len() > MAX_SOURCE_BYTES)
        {
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        paths.push((
            relative.to_string_lossy().replace('\\', "/"),
            path.to_path_buf(),
        ));
    }
    paths.sort_by(|left, right| left.0.cmp(&right.0));
    paths
}

fn private_runtime_path(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return true;
    };
    relative.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(
                ".commandagent"
                    | ".anvil"
                    | ".cache"
                    | ".git"
                    | ".mypy_cache"
                    | ".next"
                    | ".pytest_cache"
                    | ".ruff_cache"
                    | "__pycache__"
                    | "build"
                    | "coverage"
                    | "dist"
                    | "evidence"
                    | "node_modules"
                    | "out"
                    | "target"
            )
        )
    })
}

fn top_level_functions(source: &str) -> Vec<String> {
    source
        .lines()
        .filter(|line| line.trim_start() == *line)
        .filter_map(|line| {
            line.strip_prefix("def ")
                .or_else(|| line.strip_prefix("async def "))
        })
        .filter_map(|rest| rest.split_once('(').map(|(name, _)| name.trim()))
        .filter(|name| {
            !name.starts_with('_')
                && !name.is_empty()
                && name
                    .chars()
                    .all(|character| character == '_' || character.is_ascii_alphanumeric())
        })
        .map(str::to_string)
        .collect()
}

fn caller_references(source: &str, owner_path: &str, symbol: &str) -> bool {
    let module = owner_path
        .strip_suffix(".py")
        .unwrap_or(owner_path)
        .replace('/', ".");
    let symbol_call = format!(".{symbol}(");
    let path_literal = [format!("\"{owner_path}\""), format!("'{owner_path}'")]
        .iter()
        .any(|literal| source.contains(literal));
    let direct_import = source.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(&format!("from {module} import "))
            && trimmed
                .split_once(" import ")
                .is_some_and(|(_, names)| names.split(',').any(|name| name.trim() == symbol))
    });
    direct_import || (source.contains(&symbol_call) && (path_literal || source.contains(&module)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/issue399-phase6-ab-uat/fixtures/recovery-api-preservation"
    );

    #[test]
    fn detects_removed_api_referenced_by_dynamic_loader_caller() {
        let root = Path::new(FIXTURE);
        let snapshot = ReferencedApiSnapshot::capture(root);
        assert!(snapshot.requirements.iter().any(|requirement| {
            requirement.owner_path == "pipeline/main.py"
                && requirement.symbol == "write_outputs"
                && requirement.caller_paths == ["scripts/repro.py"]
        }));

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("pipeline")).unwrap();
        std::fs::write(
            dir.path().join("pipeline/main.py"),
            std::fs::read_to_string(root.join("pipeline/broken-main.py")).unwrap(),
        )
        .unwrap();
        let violations = snapshot.violations(dir.path(), &["pipeline/main.py".to_string()]);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].symbol, "write_outputs");
    }

    #[test]
    fn ignores_unreferenced_public_functions_and_unchanged_owners() {
        let root = Path::new(FIXTURE);
        let snapshot = ReferencedApiSnapshot::capture(root);
        assert!(!snapshot.requirements.iter().any(|requirement| {
            requirement.owner_path == "pipeline/main.py" && requirement.symbol == "summarize"
        }));
        assert!(snapshot.violations(root, &[]).is_empty());
    }

    #[test]
    fn artifact_snapshot_uses_bytes_not_tool_names_for_mutation() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.py"), "print('before')\n").unwrap();
        let before = ArtifactSnapshot::capture(dir.path());

        std::fs::write(dir.path().join("app.py"), "print('before')\n").unwrap();
        assert!(
            before
                .changed_paths(&ArtifactSnapshot::capture(dir.path()))
                .is_empty()
        );

        std::fs::write(dir.path().join("app.py"), "print('after')\n").unwrap();
        assert_eq!(
            before.changed_paths(&ArtifactSnapshot::capture(dir.path())),
            ["app.py"]
        );
    }

    #[test]
    fn artifact_snapshot_counts_unreported_product_changes_but_ignores_runtime_evidence() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.py"), "print('before')\n").unwrap();
        let before = ArtifactSnapshot::capture(dir.path());
        std::fs::write(dir.path().join("app.py"), "print('changed by shell')\n").unwrap();
        std::fs::create_dir_all(dir.path().join("evidence")).unwrap();
        std::fs::write(dir.path().join("evidence/probe.json"), "{}\n").unwrap();
        std::fs::create_dir_all(dir.path().join(".next/cache")).unwrap();
        std::fs::write(dir.path().join(".next/cache/manifest.json"), "{}\n").unwrap();

        assert_eq!(
            before.changed_paths(&ArtifactSnapshot::capture(dir.path())),
            ["app.py"]
        );
    }
}
