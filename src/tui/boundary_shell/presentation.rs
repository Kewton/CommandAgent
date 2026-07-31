use std::path::Path;

use anyhow::{Context, bail};

use super::acceptance::{NextAction, TerminalPresentation};
use super::confirmation::{ConfirmationIdentity, PackSelection};
use super::pack_catalog;

pub fn render_gate_one(
    identity: &ConfirmationIdentity,
    repository_root: &Path,
) -> anyhow::Result<String> {
    validate_complete_identity(identity)?;
    let card_hash = identity.card_hash()?;
    let mut lines = vec![
        "# Gate 1 — Request confirmation".to_string(),
        String::new(),
        format!("- Card hash: {card_hash}"),
        format!("- Request: {}", identity.request),
        format!("- Workspace: {}", identity.workspace),
        format!(
            "- Route: {} × {} × {}",
            identity.profile, identity.intent, identity.task_family
        ),
        format!("- Route basis: {}", identity.route_bases.join("; ")),
        format!("- Contract: {}", identity.contract_ref),
        format!("- Checks: {}", identity.contract_checks.join(", ")),
        format!(
            "- Value tag: {} ({}/{}, {})",
            identity.band_rate, identity.band_full, identity.band_denominator, identity.band_arm
        ),
        format!("- Measurement: {}", identity.band_measurement),
        format!("- Band source: {}", identity.band_source),
        format!("- Full meaning: {}", identity.full_meaning),
        format!(
            "- Planner: {} / {}",
            identity.pins.planner_provider, identity.pins.planner_model
        ),
        format!(
            "- Executor: {} / {}",
            identity.pins.executor_provider, identity.pins.executor_model
        ),
        format!("- Preset: {}", identity.pins.preset),
    ];
    render_pack(identity, repository_root, &mut lines)?;
    let candidates = pack_catalog::compatible(&identity.profile, &identity.intent);
    lines.push(if candidates.is_empty() {
        "- Compatible admitted packs: none".to_string()
    } else {
        format!(
            "- Compatible admitted packs: {}",
            candidates
                .iter()
                .map(|pack| format!("{}@{} / {}", pack.id, pack.version, pack.hash))
                .collect::<Vec<_>>()
                .join("; ")
        )
    });
    lines.extend([
        String::new(),
        "This card is a proposal, not an earned result.".to_string(),
        format!("Confirm with `/confirm {card_hash}` before dispatch."),
    ]);
    Ok(lines.join("\n"))
}

pub fn render_gate_three(
    identity: &ConfirmationIdentity,
    terminal: &TerminalPresentation,
) -> anyhow::Result<String> {
    validate_terminal_identity(identity, terminal)?;
    if !terminal.full {
        bail!("non-full terminals must use Gate 4");
    }
    Ok(format!(
        "# Gate 3 — Acceptance\n\nConfirmed Full meaning: {}\n\n{}",
        identity.full_meaning, terminal.acceptance_sheet
    ))
}

pub fn render_gate_four(
    identity: &ConfirmationIdentity,
    terminal: &TerminalPresentation,
    actions: &[(NextAction, bool, &str)],
) -> anyhow::Result<String> {
    validate_terminal_identity(identity, terminal)?;
    if terminal.full {
        bail!("full terminals must use Gate 3");
    }
    let section5 = terminal
        .section5
        .as_deref()
        .context("Gate 4 requires section 5")?;
    let action_lines = actions
        .iter()
        .map(|(action, enabled, reason)| {
            format!(
                "- {}: {} — {}",
                action.as_str(),
                if *enabled { "available" } else { "unavailable" },
                reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!(
        "# Gate 4 — Failure and next action\n\nConfirmed Full meaning: {}\n\n{}\n\n## Section 5\n\n{}\n\n## Typed next actions\n\n{}",
        identity.full_meaning, terminal.acceptance_sheet, section5, action_lines
    ))
}

fn render_pack(
    identity: &ConfirmationIdentity,
    repository_root: &Path,
    lines: &mut Vec<String>,
) -> anyhow::Result<()> {
    match &identity.pack {
        PackSelection::None => {
            lines.push("- Pack: no pack".to_string());
            lines.push("- Pack pin: no pack".to_string());
        }
        PackSelection::Pinned {
            id,
            version,
            hash,
            point,
        } => {
            lines.push(format!("- Pack: {id}@{version}"));
            lines.push(format!("- Pack pin: {hash}"));
            lines.push(format!("- Pack point: {point}"));
            let observed = pack_catalog::observed_pin(repository_root, &identity.pack)?;
            if observed.as_deref() != Some(hash) {
                lines.push(format!(
                    "- WARNING: stale pack pin (confirmed {hash}, observed {})",
                    observed.as_deref().unwrap_or("missing")
                ));
            } else {
                lines.push("- Pack pin status: exact-byte match".to_string());
            }
        }
    }
    Ok(())
}

fn validate_complete_identity(identity: &ConfirmationIdentity) -> anyhow::Result<()> {
    let required = [
        ("request", identity.request.as_str()),
        ("workspace", identity.workspace.as_str()),
        ("profile", identity.profile.as_str()),
        ("intent", identity.intent.as_str()),
        ("task_family", identity.task_family.as_str()),
        ("contract_ref", identity.contract_ref.as_str()),
        ("band_rate", identity.band_rate.as_str()),
        ("band_arm", identity.band_arm.as_str()),
        ("band_measurement", identity.band_measurement.as_str()),
        ("band_source", identity.band_source.as_str()),
        ("full_meaning", identity.full_meaning.as_str()),
        ("planner_provider", identity.pins.planner_provider.as_str()),
        ("planner_model", identity.pins.planner_model.as_str()),
        (
            "executor_provider",
            identity.pins.executor_provider.as_str(),
        ),
        ("executor_model", identity.pins.executor_model.as_str()),
        ("preset", identity.pins.preset.as_str()),
    ];
    let missing = required
        .into_iter()
        .filter_map(|(name, value)| value.trim().is_empty().then_some(name))
        .collect::<Vec<_>>();
    if !missing.is_empty()
        || identity.route_bases.is_empty()
        || identity.contract_checks.is_empty()
        || identity.band_denominator == 0
    {
        bail!(
            "Gate 1 refuses conversational omission: missing={}, bases={}, checks={}, denominator={}",
            missing.join(","),
            identity.route_bases.len(),
            identity.contract_checks.len(),
            identity.band_denominator
        );
    }
    pack_catalog::validate_selection(&identity.profile, &identity.intent, &identity.pack)
}

fn validate_terminal_identity(
    identity: &ConfirmationIdentity,
    terminal: &TerminalPresentation,
) -> anyhow::Result<()> {
    validate_complete_identity(identity)?;
    if identity.card_hash()? != terminal.card_hash {
        bail!("terminal sheet identity differs from the confirmed Gate 1 card");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::planner::adjudication::contract::IntentId;
    use crate::planner::profile::ProfileId;
    use crate::tui::boundary_shell::band_catalog::value_for;
    use crate::tui::boundary_shell::family_catalog::TaskFamilyId;
    use crate::tui::boundary_shell::route::{RouteBasis, RouteCandidate};

    use super::*;
    use crate::tui::boundary_shell::confirmation::ExecutionPins;

    fn identity(pack: PackSelection) -> ConfirmationIdentity {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let route = RouteCandidate {
            profile: ProfileId::PythonCli,
            intent: IntentId::Create,
            family: TaskFamilyId::Stats,
            bases: vec![RouteBasis {
                rule: "fixture",
                observation: "stats".to_string(),
            }],
            contract_ref: "docs/cli-profile-contract.md",
        };
        ConfirmationIdentity::new(
            "create a CLI".to_string(),
            root,
            &route,
            value_for("python-cli", IntentId::Create, TaskFamilyId::Stats).unwrap(),
            ExecutionPins {
                planner_provider: "ollama".to_string(),
                planner_model: "planner".to_string(),
                executor_provider: "ollama".to_string(),
                executor_model: "executor".to_string(),
                preset: "profile".to_string(),
            },
            pack,
        )
        .unwrap()
    }

    #[test]
    fn no_pack_card_cannot_omit_value_full_meaning_or_explicit_pin_state() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let identity = identity(PackSelection::None);
        let rendered = render_gate_one(&identity, root).unwrap();
        for required in [
            "Value tag: 0% (0/3, formal Window B)",
            "Full meaning: C1-C4 pass",
            "Pack: no pack",
            "Pack pin: no pack",
            "Checks: C1, C2, C3, C4",
        ] {
            assert!(rendered.contains(required), "{rendered}");
        }
    }

    #[test]
    fn missing_value_or_pack_pin_is_a_fixture_failure_not_a_shorter_card() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut missing_value = identity(PackSelection::None);
        missing_value.band_denominator = 0;
        assert!(render_gate_one(&missing_value, root).is_err());

        let mut missing_pin = identity(PackSelection::None);
        missing_pin.pack = PackSelection::Pinned {
            id: "cli-assist".to_string(),
            version: "1.1.0".to_string(),
            hash: String::new(),
            point: "cli-validation".to_string(),
        };
        assert!(render_gate_one(&missing_pin, root).is_err());
    }

    #[test]
    fn stale_pack_pin_is_warned_without_silently_rebinding_the_card() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let identity = identity(PackSelection::Pinned {
            id: "cli-assist".to_string(),
            version: "1.1.0".to_string(),
            hash: pack_catalog::ADMITTED_PACKS[1].hash.to_string(),
            point: "cli-validation".to_string(),
        });
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("packs/cli-assist/1.1.0");
        std::fs::create_dir_all(&destination).unwrap();
        let mut bytes =
            std::fs::read(repository_root.join("packs/cli-assist/1.1.0/assist.yaml")).unwrap();
        bytes.extend_from_slice(b"\n# stale bytes\n");
        std::fs::write(destination.join("assist.yaml"), bytes).unwrap();

        let rendered = render_gate_one(&identity, temp.path()).unwrap();
        assert!(rendered.contains("WARNING: stale pack pin"), "{rendered}");
        assert!(
            rendered.contains(pack_catalog::ADMITTED_PACKS[1].hash),
            "{rendered}"
        );
    }

    #[test]
    fn full_sheet_is_included_verbatim_and_cannot_cross_card_identity() {
        let identity = identity(PackSelection::None);
        let sheet = "# Acceptance sheet\n\nverdict: full".to_string();
        let terminal =
            TerminalPresentation::new(identity.card_hash().unwrap(), sheet.clone(), true, None)
                .unwrap();
        let rendered = render_gate_three(&identity, &terminal).unwrap();
        assert!(rendered.ends_with(&sheet));

        let other =
            TerminalPresentation::new("sha256:other".to_string(), sheet, true, None).unwrap();
        assert!(render_gate_three(&identity, &other).is_err());
    }
}
