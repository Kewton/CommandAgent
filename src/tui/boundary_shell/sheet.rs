use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde_json::Value;

use super::confirmation::ConfirmationIdentity;
use super::directive::DirectiveContinuation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedSheet {
    pub markdown: String,
    pub full: bool,
    pub section5: Option<String>,
}

pub fn generate(
    identity: &ConfirmationIdentity,
    events_path: Option<&Path>,
    command_succeeded: bool,
) -> anyhow::Result<GeneratedSheet> {
    let event = crate::eval_events::latest_tui_command_stop_event(events_path)
        .context("tui_command_stop is required to generate the acceptance sheet")?;
    let event_profile = string(&event, "effective_profile")
        .or_else(|| string(&event, "profile"))
        .unwrap_or("unknown");
    if canonical_profile(event_profile) != canonical_profile(&identity.profile) {
        bail!(
            "executed profile `{event_profile}` differs from confirmed profile `{}`",
            identity.profile
        );
    }
    let status = string(&event, "status").unwrap_or("unknown");
    let assurance = string(&event, "assurance_level").unwrap_or("unknown");
    let assurance_reason = string(&event, "assurance_reason").unwrap_or("");
    let runtime = string(&event, "runtime_acceptance_status").unwrap_or("not_recorded");
    let final_acceptance = string(&event, "final_acceptance_status").unwrap_or("not_recorded");
    let release_gate = string(&event, "release_gate_status").unwrap_or("not_recorded");
    let stop_reason = string(&event, "stop_reason")
        .or_else(|| string(&event, "primary_reason"))
        .unwrap_or(if command_succeeded {
            "completed"
        } else {
            "unknown failure"
        });
    let full = command_succeeded
        && assurance == "full"
        && matches!(final_acceptance, "full" | "full_success" | "completed")
        && status == "completed";
    let events_display = events_path
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "not_recorded".to_string());
    let summary_display = events_path
        .and_then(Path::parent)
        .map(|parent| parent.join("summary.md").display().to_string())
        .unwrap_or_else(|| "not_recorded".to_string());
    let section5 = (!full).then(|| stop_reason.to_string());
    let mut markdown = format!(
        "# D-3c acceptance sheet\n\n\
         ## 1. Confirmed identity\n\n\
         - Card hash: {}\n\
         - Request: {}\n\
         - Workspace: {}\n\
         - Route: {} × {} × {}\n\
         - Contract: {}\n\
         - Full meaning: {}\n\
         - Value tag at confirmation: {} ({}/{}, {})\n\n\
         ## 2. Terminal projection\n\n\
         - Command succeeded: {}\n\
         - Status: {}\n\
         - Assurance: {}{}\n\
         - Runtime acceptance: {}\n\
         - Final acceptance: {}\n\
         - Release gate: {}\n\n\
         ## 3. Definition of done\n\n\
         - Contract checks: {}\n\
         {}\n\n\
         ## 4. Machine evidence\n\n\
         - Event stream: {}\n\
         - Product summary: {}\n\n\
         ## 5. Stop reason\n\n\
         {}\n",
        identity.card_hash()?,
        identity.request,
        identity.workspace,
        identity.profile,
        identity.intent,
        identity.task_family,
        identity.contract_ref,
        identity.full_meaning,
        identity.band_rate,
        identity.band_full,
        identity.band_denominator,
        identity.band_arm,
        command_succeeded,
        status,
        assurance,
        if assurance_reason.is_empty() {
            String::new()
        } else {
            format!(" ({assurance_reason})")
        },
        runtime,
        final_acceptance,
        release_gate,
        identity.contract_checks.join(", "),
        pack_lines(identity),
        events_display,
        summary_display,
        stop_reason,
    );
    if let Some(manifest) = identity.draft_manifest.as_ref() {
        let route_line = format!(
            "- Route: {} × {} × {}\n",
            identity.profile, identity.intent, identity.task_family
        );
        let mut manifest_lines = format!(
            "- Manifest: {}\n- Manifest source: {}\n- Manifest hash: {}\n- Assurance ceiling: {}\n",
            manifest.path, manifest.source, manifest.hash, manifest.assurance_ceiling
        );
        if let Some(base) = manifest.base_profile.as_ref() {
            manifest_lines.push_str(&format!("- Overlay base: {base} (admitted)\n"));
        }
        markdown = markdown.replacen(&route_line, &format!("{route_line}{manifest_lines}"), 1);
    }
    Ok(GeneratedSheet {
        markdown,
        full,
        section5,
    })
}

pub fn persist(
    state_root: &Path,
    identity: &ConfirmationIdentity,
    sheet: &GeneratedSheet,
) -> anyhow::Result<PathBuf> {
    let directory = state_root.join("boundary-sheets");
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("create sheet directory {}", directory.display()))?;
    let path = directory.join(format!(
        "{}.md",
        identity.card_hash()?.trim_start_matches("sha256:")
    ));
    std::fs::write(&path, sheet.markdown.as_bytes())
        .with_context(|| format!("write acceptance sheet {}", path.display()))?;
    Ok(path)
}

pub fn with_directive_metadata(
    mut sheet: GeneratedSheet,
    continuation: &DirectiveContinuation,
) -> GeneratedSheet {
    sheet.markdown.push_str(&format!(
        "\n## Directive continuation metadata\n\n\
- Directive round: {}\n\
- Directive hash: {}\n\
- Target run ID: {}\n\
- Continuation plan: {}\n",
        continuation.directive_round,
        continuation.directive_hash,
        continuation.target_run_id,
        continuation.plan_workspace_path,
    ));
    sheet
}

pub fn persist_directive_round(
    state_root: &Path,
    identity: &ConfirmationIdentity,
    sheet: &GeneratedSheet,
    round: u32,
) -> anyhow::Result<PathBuf> {
    let directory = state_root.join("boundary-sheets");
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("create sheet directory {}", directory.display()))?;
    let path = directory.join(format!(
        "{}-directive-round-{round}.md",
        identity.card_hash()?.trim_start_matches("sha256:")
    ));
    std::fs::write(&path, sheet.markdown.as_bytes())
        .with_context(|| format!("write directive acceptance sheet {}", path.display()))?;
    Ok(path)
}

fn string<'a>(event: &'a Value, key: &str) -> Option<&'a str> {
    event.get(key).and_then(Value::as_str)
}

fn canonical_profile(profile: &str) -> &str {
    match profile {
        "cli" => "python-cli",
        other => other,
    }
}

fn pack_label(identity: &ConfirmationIdentity) -> String {
    match &identity.pack {
        super::confirmation::PackSelection::None => "no pack".to_string(),
        super::confirmation::PackSelection::Pinned {
            id, version, hash, ..
        } => format!("{id}@{version} / {hash}"),
    }
}

fn pack_lines(identity: &ConfirmationIdentity) -> String {
    let label = pack_label(identity);
    match &identity.pack {
        super::confirmation::PackSelection::None => format!("- Pack: {label}"),
        super::confirmation::PackSelection::Pinned { source, .. } => format!(
            "- Pack: {label}\n- Pack source: {}",
            source.japanese_label()
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::planner::adjudication::contract::IntentId;
    use crate::planner::pack::catalog::{ADMITTED_PACKS, PackSource};
    use crate::planner::profile::ProfileId;
    use crate::tui::boundary_shell::band_catalog::value_for;
    use crate::tui::boundary_shell::confirmation::{ExecutionPins, PackSelection};
    use crate::tui::boundary_shell::family_catalog::TaskFamilyId;
    use crate::tui::boundary_shell::route::{RouteBasis, RouteCandidate};

    use super::*;

    fn identity(root: &Path) -> ConfirmationIdentity {
        let route = RouteCandidate {
            profile: ProfileId::Ingest,
            intent: IntentId::Create,
            family: TaskFamilyId::List,
            bases: vec![RouteBasis {
                rule: "fixture",
                observation: "list".to_string(),
            }],
            contract_ref: "docs/ingest-profile-contract.md",
        };
        ConfirmationIdentity::new(
            "create ingest".to_string(),
            root,
            &route,
            value_for("ingest", IntentId::Create, TaskFamilyId::List).unwrap(),
            ExecutionPins {
                planner_provider: "ollama".to_string(),
                planner_model: "planner".to_string(),
                executor_provider: "ollama".to_string(),
                executor_model: "executor".to_string(),
                preset: "profile".to_string(),
            },
            PackSelection::None,
        )
        .unwrap()
    }

    #[test]
    fn sheet_uses_terminal_event_without_upgrading_a_failed_run() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        crate::eval_events::emit(
            Some(&events),
            serde_json::json!({
                "event": "tui_command_stop",
                "effective_profile": "ingest",
                "status": "failed",
                "assurance_level": "failed",
                "assurance_reason": "source_binding_violation",
                "runtime_acceptance_status": "failed",
                "final_acceptance_status": "failed",
                "release_gate_status": "failed",
                "stop_reason": "N2 rejected an invented event",
            }),
        );
        let sheet = generate(&identity(dir.path()), Some(&events), false).unwrap();
        assert!(!sheet.full);
        assert_eq!(
            sheet.section5.as_deref(),
            Some("N2 rejected an invented event")
        );
        assert!(sheet.markdown.contains("Assurance: failed"));
        assert!(
            sheet
                .markdown
                .contains("Contract checks: N1, N2, N3, N4, N5")
        );
    }

    #[test]
    fn pack_lines_add_a_supply_source_only_for_a_pinned_pack() {
        let dir = tempfile::tempdir().unwrap();
        let mut identity = identity(dir.path());
        assert_eq!(pack_lines(&identity), "- Pack: no pack");

        identity.pack = PackSelection::Pinned {
            id: "cli-assist".to_string(),
            version: "1.1.0".to_string(),
            hash: ADMITTED_PACKS[1].hash.to_string(),
            point: "cli-validation".to_string(),
            source: PackSource::Admitted,
        };
        assert_eq!(
            pack_lines(&identity),
            format!(
                "- Pack: cli-assist@1.1.0 / {}\n- Pack source: 承認済み",
                ADMITTED_PACKS[1].hash
            )
        );
    }
}
