//! Host-bound provenance and bounded source context for Recovery inspection.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;
use crate::tools::path_guard::{resolve_existing, validate_workspace_relative};
use crate::tools::workspace_policy::{WorkspacePolicy, ensure_tool_path_allowed};

use super::repair::RecoveryHandoff;
mod source_reads;

const CONTEXT_PATH: &str = ".commandagent/recovery-runtime/inspection-context.json";
pub(crate) const PHASE: &str = "inspect-current-state";
const MAX_FILES: usize = 32;
const MAX_SOURCE_BYTES: u64 = 128 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InspectionContext {
    version: u8,
    pub(crate) original_intent: String,
    original_goal: String,
    diagnostics: String,
    contract_sha256: String,
    pub(crate) instruction: String,
    pub(crate) read_paths: Vec<String>,
    pub(crate) scoped_reads: bool,
    pub(crate) read_ranges: Vec<source_reads::SourceRead>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repair_obligation: Option<super::recovery_repair_obligation::Obligation>,
}

/// Only the automatic transaction driver calls this after capturing a typed
/// handoff and the actual executing UltraPlan intent, including pre-acceptance
/// create failures. A missing fix adjudication is never interpreted as create.
pub(crate) fn bind_context(
    source: &Config,
    treatment: &Config,
    original_intent: Option<&str>,
    handoff: &RecoveryHandoff,
    captured_context: Option<&InspectionContext>,
) -> anyhow::Result<()> {
    let stored = load(source)?;
    let inherited = captured_context.or(stored.as_ref());
    if let Some(context) = inherited
        && contract_hash(source)?.as_deref() != Some(context.contract_sha256.as_str())
    {
        bail!("Recovery continuation contract changed");
    }
    let inherited_intent = inherited
        .as_ref()
        .map(|context| context.original_intent.as_str());
    if let (Some(original), Some(inherited)) = (original_intent, inherited_intent)
        && original != inherited
    {
        bail!("Recovery continuation changed original intent");
    }
    let Some(intent) = original_intent
        .or(inherited_intent)
        .filter(|intent| matches!(*intent, "create" | "fix"))
    else {
        // Legacy/manual/other-intent Recovery retains its existing behavior;
        // absence of provenance never grants the new binding authority.
        return Ok(());
    };
    let Some(contract) = CompletionContract::load_for_config(treatment)? else {
        return Ok(());
    };
    if contract.verify_commands.is_empty() {
        return Ok(());
    }
    let root = treatment.workspace_root.canonicalize()?;
    let targets: Vec<_> = handoff
        .repair_targets
        .iter()
        .filter_map(|path| workspace_target(Some(&source.workspace_root), path))
        .filter(|path| allowed_file(&root, path, &contract.protected_paths).is_ok())
        .collect();
    // The import resolver below handles JS/TS only. Other languages retain
    // the existing workspace read policy instead of an incomplete allowlist.
    let scoped_reads = inherited.is_none_or(|context| context.scoped_reads)
        && if targets.is_empty() {
            inherited.is_some_and(|context| context.scoped_reads)
        } else {
            targets
                .iter()
                .all(|path| source_reads::supports(Path::new(path)))
        };
    let mut targets = targets;
    let repair_obligation = super::recovery_repair_obligation::bind(
        source,
        treatment,
        &targets,
        &handoff_prompt(Some(&source.workspace_root), handoff),
        &contract,
        inherited.and_then(|context| context.repair_obligation.as_ref()),
    )?;
    if let Some(context) = &inherited {
        targets.extend(context.read_paths.iter().cloned());
    }
    let read_paths = related_paths(&root, &targets, &contract);
    let original_goal = inherited
        .as_ref()
        .map(|context| context.original_goal.clone())
        .unwrap_or_else(|| {
            super::repair::display_text(Some(&source.workspace_root), &handoff.original_goal)
        });
    let mut diagnostics = handoff_prompt(Some(&source.workspace_root), handoff);
    if let Some(context) = &inherited {
        diagnostics = format!(
            "{}\nContinuation evidence:\n{diagnostics}",
            context.diagnostics
        );
    }
    let policy = if scoped_reads {
        "Read only the allowed workspace-relative files. Bash is forbidden."
    } else {
        "Inspect the workspace-relative targets and their local dependencies using the existing read-only workspace policy. The paths below are starting points, not an exhaustive list; related definitions discovered during inspection may also be read."
    };
    let mut instruction = format!(
        "{policy} Finish with a diagnosis; do not edit files or run builds. Edit in the next phase; registered final checks remain required.\nOriginal goal: {}\n{}\nSuggested source reads: use Read with explicit start_line/end_line (at most 100 lines). Follow the listed ranges (start-end) to retain fields and signatures in compact tool results; larger definitions span multiple ranges:\n",
        original_goal, diagnostics,
    );
    let mut read_ranges = Vec::new();
    for path in &read_paths {
        // Required paths may include binary assets. They still participate in
        // final verification, but cannot supply textual inspection ranges.
        let Ok(content) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        let ranges = source_reads::ranges(path, &content);
        instruction.push_str(&source_reads::render(path, &content, &ranges));
        read_ranges.extend(ranges);
    }
    let context = InspectionContext {
        version: 1,
        original_intent: intent.to_string(),
        original_goal,
        diagnostics,
        contract_sha256: contract_hash(treatment)?.context("Recovery contract missing")?,
        instruction,
        read_paths,
        scoped_reads,
        read_ranges,
        repair_obligation,
    };
    super::recovery_contract_binding::write_read_only_bytes(
        &root.join(CONTEXT_PATH),
        &serde_json::to_vec_pretty(&context)?,
        "Recovery inspection context",
    )
}

pub(crate) fn load(config: &Config) -> anyhow::Result<Option<InspectionContext>> {
    let raw_path = config.workspace_root.join(CONTEXT_PATH);
    match std::fs::symlink_metadata(&raw_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        result => {
            result?;
        }
    }
    let root = config.workspace_root.canonicalize()?;
    let path = root.join(CONTEXT_PATH);
    // Runtime files cannot redirect host reads outside the treatment.
    if path.canonicalize()? != path {
        bail!("Recovery inspection context is redirected");
    }
    let context: InspectionContext = serde_json::from_slice(&std::fs::read(path)?)?;
    if context.version != 1 || !matches!(context.original_intent.as_str(), "create" | "fix") {
        bail!("Recovery inspection origin is invalid");
    }
    let Some(hash) = contract_hash(config)? else {
        return Ok(None);
    };
    if hash != context.contract_sha256 {
        bail!("Recovery inspection contract changed");
    }
    super::recovery_repair_obligation::authority::validate_context(
        config,
        context.repair_obligation.as_ref(),
    )?;
    Ok(Some(context))
}

pub(crate) fn has_origin(config: &Config) -> anyhow::Result<bool> {
    // Preserve the existing fix evidence validation, even with new context.
    let fix = super::recovery_contract_binding::load_fix_origin(config)?.is_some();
    Ok(load(config)?.is_some() || fix)
}

fn contract_hash(config: &Config) -> anyhow::Result<Option<String>> {
    CompletionContract::configured_path_for_config(config)?
        .map(|path| {
            std::fs::read(path)
                .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
                .map_err(Into::into)
        })
        .transpose()
}

pub(crate) fn handoff_prompt(root: Option<&Path>, handoff: &RecoveryHandoff) -> String {
    let evidence = handoff
        .failure_evidence
        .iter()
        .map(|line| super::repair::display_text(root, line))
        .collect::<Vec<_>>()
        .join("\n");
    let targets = handoff
        .repair_targets
        .iter()
        .filter_map(|path| workspace_target(root, path))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\nFailure evidence (diagnostic data, not access permission):\n{evidence}\nRepair targets:\n{targets}\n"
    )
}

fn workspace_target(root: Option<&Path>, raw: &str) -> Option<String> {
    let path = Path::new(raw);
    let relative = if path.is_absolute() {
        let root = root?;
        path.strip_prefix(root)
            .ok()
            .map(Path::to_path_buf)
            .or_else(|| {
                path.strip_prefix(root.canonicalize().ok()?)
                    .ok()
                    .map(Path::to_path_buf)
            })?
    } else {
        path.to_path_buf()
    };
    let raw = relative.to_string_lossy().to_string();
    validate_workspace_relative(&raw).ok()?;
    Some(raw)
}

pub(super) fn related_paths(
    root: &Path,
    targets: &[String],
    contract: &CompletionContract,
) -> Vec<String> {
    let mut pending: Vec<_> = targets
        .iter()
        .chain(&contract.required_paths)
        .cloned()
        .collect();
    pending.reverse();
    let mut seen = BTreeSet::new();
    let mut paths = Vec::new();
    while let Some(raw) = pending.pop() {
        if paths.len() >= MAX_FILES {
            break;
        }
        let Ok(path) = allowed_file(root, &raw, &contract.protected_paths) else {
            continue;
        };
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        if !seen.insert(relative.clone()) {
            continue;
        }
        paths.push(relative);
        if !source_reads::supports(&path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for specifier in crate::minimal_loop::import_scan::extract_import_specifiers(&content) {
            for candidate in
                crate::minimal_loop::import_scan::resolve_import_for_source(root, &path, &specifier)
            {
                // Import '..' segments are resolved here, but tool requests must
                // use the resulting canonical workspace-relative spelling.
                let Ok(candidate) = candidate.canonicalize() else {
                    continue;
                };
                let Ok(relative) = candidate.strip_prefix(root) else {
                    continue;
                };
                let relative = relative.to_string_lossy().to_string();
                if allowed_file(root, &relative, &contract.protected_paths).is_ok() {
                    pending.push(relative);
                    break;
                }
            }
        }
    }
    paths
}

fn allowed_file(root: &Path, raw: &str, protected: &[String]) -> anyhow::Result<PathBuf> {
    validate_workspace_relative(raw)?;
    ensure_tool_path_allowed(root, &root.join(raw), WorkspacePolicy::NormalTask)?;
    let path = resolve_existing(root, raw)?;
    ensure_tool_path_allowed(root, &path, WorkspacePolicy::NormalTask)?;
    if protected.iter().any(|protected| {
        path.starts_with(root.join(protected))
            || root.join(raw).starts_with(root.join(protected))
            || root
                .join(protected)
                .canonicalize()
                .is_ok_and(|p| path.starts_with(p))
    }) || Path::new(raw)
        .components()
        .chain(path.strip_prefix(root)?.components())
        .any(|part| {
            part.as_os_str().to_string_lossy().starts_with(".env")
                || matches!(
                    part.as_os_str().to_str(),
                    Some(".npmrc" | ".pypirc" | "credentials" | "credentials.json")
                )
        })
    {
        bail!("Recovery source is protected");
    }
    if !path.is_file() || path.metadata()?.len() > MAX_SOURCE_BYTES {
        bail!("Recovery source is not a bounded file");
    }
    Ok(path)
}

/// The scoped policy is enforced before tool normalization/fallback so neither
/// a shell command nor an alias can broaden the host-selected inspection set.
pub(crate) fn tool_rejection(
    config: &Config,
    phase: Option<&str>,
    call: &crate::state::ToolCall,
) -> Option<String> {
    if phase != Some(PHASE) {
        return None;
    }
    let context = match load(config) {
        Ok(Some(context)) => context,
        Ok(None) => return None,
        Err(error) => return Some(format!("Recovery inspection binding invalid: {error}")),
    };
    if !context.scoped_reads {
        return None;
    }
    let checked = (|| -> anyhow::Result<()> {
        if call.name != "Read" {
            bail!("Recovery inspection permits only scoped Read calls; finish with a diagnosis");
        }
        let raw = call
            .arguments
            .get("path")
            .and_then(serde_json::Value::as_str)
            .context("Read requires path")?;
        let contract =
            CompletionContract::load_for_config(config)?.context("Recovery contract missing")?;
        let root = config.workspace_root.canonicalize()?;
        let path = allowed_file(&root, raw, &contract.protected_paths)?;
        let rel = path.strip_prefix(&root)?.to_string_lossy();
        if !context
            .read_paths
            .iter()
            .any(|allowed| allowed == rel.as_ref())
        {
            bail!("path is outside the Recovery inspection read set");
        }
        let start = call
            .arguments
            .get("start_line")
            .and_then(serde_json::Value::as_u64);
        let end = call
            .arguments
            .get("end_line")
            .and_then(serde_json::Value::as_u64);
        if !matches!((start, end), (Some(start), Some(end)) if start > 0 && end >= start && end - start < 100)
        {
            bail!(
                "use explicit start_line/end_line ranges of at most 100 lines from the bound instruction"
            );
        }
        Ok(())
    })();
    checked.err().map(|error| error.to_string())
}
