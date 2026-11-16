//! `--operator-check` (Issue #1013): the run's own admission of the operator root's declarations,
//! with no provider call and nothing written.
//!
//! The configuration is resolved as a run resolves it and judged as a non-interactive `--prompt`
//! run. Every check calls the function the run calls. Where that function writes, its owner module
//! splits the judging part out and both the run and this check call it:
//!
//! - the OS sandbox is diagnosed, not installed (`os_sandbox::diagnose`): no
//!   `<state_dir>/sandbox/run-*` is made, and the process-wide state is not set;
//! - the run budget's declaration is read and decoded as the run reads it, and its wallet is never
//!   opened: no directory, no lock, no ledger written (`entry::check_without_opening`). A ledger
//!   already there is read without the lock and held to the declaration by the run's own
//!   continuation; the report always says the read was not locked;
//! - the route is admitted by the basis of the declaration (`admission::admit_route_on`), the
//!   same choice the client factory makes, and no client is built;
//! - a stage set is judged with the sandbox as diagnosed (`StageRunFacts::of_diagnosis`), and no
//!   stage ledger is opened;
//! - the isolation probe a proposal tool's admission runs is the same probe, under watch: its
//!   temporary area is removed here and looked at again, and one left behind fails the check.
//!
//! A pass means: the declarations, the configuration, and the probes are accepted now. It promises
//! nothing about a later run; see [`NOT_CHECKED_BY_DESIGN`].

use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::Cli;
use crate::config::{Action, Config, ProviderCliOptions, ProviderRole};
use crate::planner::structured_stages::contract::StageRunFacts;
use crate::structured_budget::entry::{self, UnlockedLedger, WalletCheck};
use crate::structured_budget::ledger::{CommandWrites, LedgerEntry};
use crate::structured_budget::responses::AccountingBasis;
use crate::tools::os_sandbox::{self, SandboxDiagnosis};
use crate::tools::profile_tools::admission::AdmittedProfileTools;
use crate::tools::profile_tools::descriptor::{self, ShownPin};
use crate::tools::profile_tools::isolation::probe_area::{self, ProbeCleanup};
use crate::tools::profile_tools::pre_run;

pub(crate) const SCHEMA_VERSION: &str = "commandagent.operator-check/v1";

/// What no check without a run can see. Shown in every report and in the docs.
pub(crate) const NOT_CHECKED_BY_DESIGN: &[&str] = &[
    "provider connectivity and authentication",
    "the model, service tier, and usage the provider actually returns",
    "the real request body, its token bound, and the reservation",
    "late responses and the latches they set",
    "creating the sandbox temporary, the wallet directory and lock, and saving the ledger at the real start",
    "whether another process holds the wallet's lock now",
    "pins changed after this check; a run compares them again before each process",
    "request-pattern routing settings",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
    Pass,
    Fail,
    /// Read, but nothing can be vouched for: a ledger that changed while it was read.
    Unverified,
    NotDeclared,
    /// Not reached: a step it depends on did not pass.
    NotChecked,
}

impl Status {
    const fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Unverified => "unverified",
            Self::NotDeclared => "not_declared",
            Self::NotChecked => "not_checked",
        }
    }

    const fn blocks(self) -> bool {
        matches!(self, Self::Fail | Self::Unverified)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Check {
    pub(crate) id: &'static str,
    pub(crate) status: Status,
    /// The run's own word for the outcome: a refusal kind or variant, or a fixed word.
    pub(crate) code: Option<String>,
    pub(crate) detail: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub(crate) data: Value,
}

fn check(id: &'static str, status: Status, code: Option<&str>, detail: impl Into<String>) -> Check {
    Check {
        id,
        status,
        code: code.map(str::to_string),
        detail: crate::sensitive_data::scrub_everything(&detail.into()),
        data: Value::Null,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Report {
    pub(crate) schema_version: &'static str,
    /// `pass` or `fail`.
    pub(crate) status: Status,
    /// The run whose admission is judged.
    pub(crate) judged_as: &'static str,
    pub(crate) checks: Vec<Check>,
    pub(crate) pins: Vec<ShownPin>,
    pub(crate) not_checked_by_design: &'static [&'static str],
}

impl Report {
    fn of(checks: Vec<Check>, pins: Vec<ShownPin>) -> Self {
        let status = if checks.iter().any(|check| check.status.blocks()) {
            Status::Fail
        } else {
            Status::Pass
        };
        Self {
            schema_version: SCHEMA_VERSION,
            status,
            judged_as: "prompt",
            checks,
            pins,
            not_checked_by_design: NOT_CHECKED_BY_DESIGN,
        }
    }

    pub(crate) fn failed(&self) -> bool {
        self.status != Status::Pass
    }

    fn render_human(&self) -> String {
        let mut lines = vec![format!(
            "CommandAgent operator check: {} (judged as a non-interactive --prompt run; no provider call, nothing written)",
            self.status.label()
        )];
        for check in &self.checks {
            let code = check
                .code
                .as_deref()
                .map(|code| format!(" [{code}]"))
                .unwrap_or_default();
            lines.push(format!(
                "{:<12} {}{code}: {}",
                check.status.label(),
                check.id,
                check.detail
            ));
        }
        if !self.pins.is_empty() {
            lines.push("Pins (shown only; nothing is written):".to_string());
            for pin in &self.pins {
                let path = pin
                    .path
                    .as_ref()
                    .map(|path| format!(" {}", path.display()))
                    .unwrap_or_default();
                let declared = pin.declared.as_deref().unwrap_or("-");
                let calculated = pin
                    .calculated
                    .clone()
                    .or_else(|| {
                        pin.unavailable
                            .as_ref()
                            .map(|why| format!("unavailable: {why}"))
                    })
                    .unwrap_or_default();
                lines.push(format!(
                    "  {} {}{path}: declared {declared}, calculated {calculated}",
                    pin.tool_id, pin.pin
                ));
            }
        }
        lines.push("Not checked by design:".to_string());
        lines.extend(
            NOT_CHECKED_BY_DESIGN
                .iter()
                .map(|item| format!("  - {item}")),
        );
        lines.join("\n")
    }
}

/// The CLI entry. A configuration the run would refuse to resolve is the run's exit-2 refusal.
pub(crate) fn run_cli(cli: Cli, provider_options: ProviderCliOptions) -> anyhow::Result<()> {
    let json = cli.json;
    let report = diagnose(cli, provider_options)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", report.render_human());
    }
    if report.failed() {
        anyhow::bail!("operator check found failed checks");
    }
    Ok(())
}

fn diagnose(cli: Cli, provider_options: ProviderCliOptions) -> anyhow::Result<Report> {
    let sandbox_mode = cli.sandbox;
    let pack_cli = cli.clone();
    let mut config = Config::from_cli_with_provider_options(cli, provider_options)
        .map_err(crate::cli_argument_error)?;
    // Without an action flag the configuration is a REPL's. The check judges the entry of a
    // non-interactive run, which uses the write sandbox and opens its wallet.
    config.action = Action::Prompt(String::new());
    let mut checks = vec![controls(sandbox_mode, &config)];

    let resolved_pack = crate::cli_pack::resolve(&pack_cli, &config);
    checks.push(match &resolved_pack {
        Ok(None) => check("pack", Status::NotDeclared, None, "no pack selected"),
        Ok(Some(pack)) => check(
            "pack",
            Status::Pass,
            None,
            format!("pack `{}` {} resolved", pack.id, pack.version),
        ),
        Err(error) => check(
            "pack",
            Status::Fail,
            Some("pack_refused"),
            format!("{error:#}"),
        ),
    });

    let (admitted, cleanup) =
        probe_area::watching(|| pre_run::admitted_before_cli_run(&config, &resolved_pack));
    checks.push(match &admitted {
        Ok(None) => check(
            "profile_tools",
            Status::NotDeclared,
            None,
            "the profile and pack declare no tools",
        ),
        Ok(Some(set)) => check(
            "profile_tools",
            Status::Pass,
            None,
            format!(
                "{} tool(s) admitted, schema and proposal preflight passed",
                set.bindings().len()
            ),
        ),
        Err(refusal) => check(
            "profile_tools",
            Status::Fail,
            Some(refusal.kind.as_str()),
            refusal.reason.clone(),
        ),
    });
    checks.push(probe_cleanup_check(&cleanup));

    let sandbox = os_sandbox::diagnose(
        sandbox_mode,
        &config.workspace_root,
        &config.state_dir,
        config.offline,
        os_sandbox::dev_server_ports(&config.profile, crate::config::action_goal(&config.action)),
    );
    checks.push(sandbox_check(&sandbox));
    checks.push(artifact_contract_check(&config, &resolved_pack));
    checks.push(stages_check(&config, admitted.as_ref().ok(), &sandbox));
    checks.extend(run_budget_checks(&config, &sandbox));

    let pins = pins(&config, &resolved_pack);
    Ok(Report::of(checks, pins))
}

/// The refusals a run makes before and beside its configuration, whatever is declared.
fn controls(sandbox_mode: os_sandbox::SandboxMode, config: &Config) -> Check {
    let refused = entry::refuse_sandbox_off(sandbox_mode)
        .and_then(|()| entry::refuse_a_control_named_as_key(config))
        .and_then(|()| entry::refuse_a_control_the_start_takes());
    match refused {
        Ok(()) => check(
            "run_budget.controls",
            Status::Pass,
            None,
            "the sandbox mode and the key's variable leave the run budget's controls alone",
        ),
        Err(refusal) => check(
            "run_budget.controls",
            Status::Fail,
            Some(&format!("{refusal:?}")),
            refusal.to_string(),
        ),
    }
}

/// A probe that left its temporary area behind is a failure here, whatever its verdict.
pub(crate) fn probe_cleanup_check(cleanup: &ProbeCleanup) -> Check {
    let area = std::env::temp_dir();
    let mut found = if cleanup.made == 0 {
        check(
            "isolation.probe_cleanup",
            Status::NotChecked,
            None,
            "no isolation probe made a temporary area",
        )
    } else if cleanup.clean() {
        check(
            "isolation.probe_cleanup",
            Status::Pass,
            None,
            format!(
                "{} probe area(s) in the system temporary directory, each removed",
                cleanup.made
            ),
        )
    } else {
        check(
            "isolation.probe_cleanup",
            Status::Fail,
            Some("probe_area_left"),
            format!(
                "{} of {} probe area(s) could not be removed",
                cleanup.left.len(),
                cleanup.made
            ),
        )
    };
    found.data = json!({
        "temporary_parent": area,
        "made": cleanup.made,
        "left": cleanup.left,
    });
    found
}

fn sandbox_check(sandbox: &SandboxDiagnosis) -> Check {
    let status = &sandbox.status;
    let mut found = check(
        "os_sandbox",
        Status::Pass,
        Some(if status.applied {
            "applied"
        } else {
            "not_applied"
        }),
        format!(
            "mode {}, backend {}{}",
            status.mode.as_str(),
            status.backend,
            status
                .reason
                .as_deref()
                .map(|reason| format!(": {reason}"))
                .unwrap_or_default()
        ),
    );
    found.data = json!({ "applied": status.applied, "created": false });
    found
}

fn artifact_contract_check(
    config: &Config,
    resolved_pack: &anyhow::Result<Option<crate::cli_pack::ResolvedPack>>,
) -> Check {
    let Ok(pack) = resolved_pack else {
        return check(
            "artifact_contract",
            Status::NotChecked,
            None,
            "the pack was not resolved",
        );
    };
    let guard = crate::cli_pack::RuntimeEnvironmentGuard::install(pack.as_ref());
    let judged = guard
        .map_err(|error| format!("{error:#}"))
        .and_then(|_guard| {
            crate::planner::artifact_contract_checks::preflight(
                &config.workspace_root,
                &config.profile,
            )
        });
    match judged {
        Ok(()) => check(
            "artifact_contract",
            Status::Pass,
            None,
            "no artifact contract adapter is declared, or every declared one is admitted",
        ),
        Err(reason) => check(
            "artifact_contract",
            Status::Fail,
            Some("artifact_contract_refused"),
            reason,
        ),
    }
}

fn stages_check(
    config: &Config,
    admitted: Option<&Option<AdmittedProfileTools>>,
    sandbox: &SandboxDiagnosis,
) -> Check {
    let Some(admitted) = admitted else {
        return check(
            "structured_stages",
            Status::NotChecked,
            None,
            "the profile tools were not admitted",
        );
    };
    let Some(admitted) = admitted else {
        return check(
            "structured_stages",
            Status::NotDeclared,
            None,
            "no stage set is declared",
        );
    };
    match admitted.judge_stage_run(config, StageRunFacts::of_diagnosis(config, sandbox)) {
        Ok(None) => check(
            "structured_stages",
            Status::NotDeclared,
            None,
            "no stage set is declared",
        ),
        Ok(Some(stages)) => check(
            "structured_stages",
            Status::Pass,
            None,
            format!("{stages} stage(s) admitted for this run; no stage ledger was opened"),
        ),
        Err(refusal) => check(
            "structured_stages",
            Status::Fail,
            Some(refusal.kind.as_str()),
            refusal.reason,
        ),
    }
}

fn run_budget_checks(config: &Config, sandbox: &SandboxDiagnosis) -> Vec<Check> {
    if !entry::selected() {
        let mut checks = vec![check(
            "run_budget.declaration",
            Status::NotDeclared,
            None,
            "no run budget is selected",
        )];
        checks.extend(route_checks(config, None, Status::NotDeclared));
        return checks;
    }
    let found = entry::check_without_opening(true, config, CommandWrites::diagnosed(sandbox));
    let not_checked = || check("run_budget.ledger", Status::NotChecked, None, "");
    match found {
        Ok(None) => {
            let mut checks = vec![check(
                "run_budget.declaration",
                Status::NotDeclared,
                None,
                "this entry opens no wallet",
            )];
            checks.extend(route_checks(config, None, Status::NotDeclared));
            checks
        }
        Ok(Some(wallet)) => {
            let mut checks = vec![
                check(
                    "run_budget.declaration",
                    Status::Pass,
                    None,
                    format!(
                        "`{}` decoded, accounting basis {}",
                        wallet.budget_id,
                        basis_word(wallet.basis)
                    ),
                ),
                ledger_check(Ok(&wallet)),
            ];
            checks.extend(route_checks(config, Some(wallet.basis), Status::Pass));
            checks
        }
        Err(entry::EntryRefusal::Wallet(refusal)) => {
            let mut checks = vec![
                check(
                    "run_budget.declaration",
                    Status::Pass,
                    None,
                    "decoded; the wallet's place or record was refused",
                ),
                ledger_check(Err(&format!("{:?}", entry::EntryRefusal::Wallet(refusal)))),
            ];
            checks.extend(route_checks(config, None, Status::NotChecked));
            checks
        }
        Err(refusal) => {
            let mut checks = vec![
                check(
                    "run_budget.declaration",
                    Status::Fail,
                    Some(&format!("{refusal:?}")),
                    refusal.to_string(),
                ),
                not_checked(),
            ];
            checks.extend(route_checks(config, None, Status::NotChecked));
            checks
        }
    }
}

const fn basis_word(basis: AccountingBasis) -> &'static str {
    match basis {
        AccountingBasis::ProviderBilled => "provider_billed",
        AccountingBasis::UsageTariff => "usage_tariff",
    }
}

/// The ledger's check always says that its read took no lock.
fn ledger_check(found: Result<&WalletCheck, &str>) -> Check {
    const UNLOCKED: &str = "read without the wallet's lock";
    let (mut found, entry) = match found {
        Err(code) => (
            check(
                "run_budget.ledger",
                Status::Fail,
                Some(code),
                format!("{UNLOCKED}; the run's open refuses this wallet"),
            ),
            None,
        ),
        Ok(wallet) => {
            let entry = Some(wallet.entry);
            let found = match wallet.ledger {
                UnlockedLedger::Absent => check(
                    "run_budget.ledger",
                    Status::Pass,
                    Some("absent"),
                    format!(
                        "{UNLOCKED}; no ledger is there yet, and a fresh run's open would make it"
                    ),
                ),
                UnlockedLedger::Continues => check(
                    "run_budget.ledger",
                    Status::Pass,
                    Some("continues"),
                    format!(
                        "{UNLOCKED}; the ledger there continues under this declaration, workspace, and profile"
                    ),
                ),
                UnlockedLedger::ChangedWhileRead => check(
                    "run_budget.ledger",
                    Status::Unverified,
                    Some("changed_while_read"),
                    format!(
                        "{UNLOCKED}; the file changed between two reads, so it is not vouched for"
                    ),
                ),
            };
            (found, entry)
        }
    };
    found.data = json!({
        "read_without_lock": true,
        "entry": entry.map(|entry| match entry {
            LedgerEntry::Fresh => "fresh",
            LedgerEntry::Resume => "resume",
        }),
    });
    found
}

/// The roles a run builds clients for: the executor always, the planner for a planned run.
fn route_checks(config: &Config, basis: Option<AccountingBasis>, otherwise: Status) -> Vec<Check> {
    [
        ("run_budget.route.executor", ProviderRole::Executor),
        ("run_budget.route.planner", ProviderRole::Planner),
    ]
    .into_iter()
    .map(|(id, role)| {
        if basis.is_none() {
            let detail = match otherwise {
                Status::NotDeclared => "no run budget is selected",
                _ => "the declaration was not admitted",
            };
            return check(id, otherwise, None, detail);
        }
        match crate::structured_budget::admission::admit_route_on(config, role, basis) {
            Ok(()) => check(id, Status::Pass, None, "the wallet can count this route"),
            Err(refusal) => check(
                id,
                Status::Fail,
                Some(&format!("{refusal:?}")),
                refusal.to_string(),
            ),
        }
    })
    .collect()
}

fn pins(
    config: &Config,
    resolved_pack: &anyhow::Result<Option<crate::cli_pack::ResolvedPack>>,
) -> Vec<ShownPin> {
    let Ok(Some(tools)) = pre_run::selected_before_cli_run(config, resolved_pack) else {
        return Vec::new();
    };
    tools
        .iter()
        .flat_map(
            |tool| match descriptor::shown_pins(tool, &config.workspace_root) {
                Ok(pins) => pins,
                Err(refusal) => vec![ShownPin {
                    tool_id: tool.clone(),
                    pin: "descriptor_sha256",
                    path: None,
                    declared: None,
                    calculated: None,
                    unavailable: Some(crate::sensitive_data::scrub_everything(
                        &refusal.to_string(),
                    )),
                }],
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{ProbeCleanup, Report, Status, check, probe_cleanup_check};

    /// PM condition 4: a probe whose temporary area is still there after its removal fails the
    /// check, and so the whole report, whatever the probe itself answered. The isolation module's
    /// own test drives a real probe through a removal that fails; this is what the check makes of it.
    #[test]
    fn a_probe_area_left_behind_fails_the_check_and_the_report() {
        let left = ProbeCleanup {
            made: 2,
            left: vec![PathBuf::from("probe-area")],
        };
        let found = probe_cleanup_check(&left);
        assert_eq!(found.status, Status::Fail);
        assert_eq!(found.code.as_deref(), Some("probe_area_left"));
        assert_eq!(found.data["made"], 2);
        let report = Report::of(vec![found], Vec::new());
        assert!(report.failed());

        let clean = probe_cleanup_check(&ProbeCleanup {
            made: 1,
            left: Vec::new(),
        });
        assert_eq!(clean.status, Status::Pass);
        assert!(!Report::of(vec![clean], Vec::new()).failed());
        let none = probe_cleanup_check(&ProbeCleanup::default());
        assert_eq!(none.status, Status::NotChecked);
    }

    /// A stage set is judged by what the sandbox probe answered, never by a caller's word: the
    /// same facts a session reads, with the boundary from the diagnosis.
    #[test]
    fn the_stage_facts_take_the_boundary_from_the_diagnosis() {
        use crate::planner::structured_stages::contract::StageRunFacts;
        use crate::tools::os_sandbox::{
            SandboxAvailability, SandboxMode, SandboxPolicy, diagnosis_for_test,
        };
        let temp = tempfile::tempdir().unwrap();
        let config = crate::tools::profile_tools::effects::config_for_test(temp.path());
        let policy = || SandboxPolicy::new(temp.path(), &temp.path().join("tmp"));
        for (mode, availability, enforced) in [
            (SandboxMode::Auto, SandboxAvailability::Available, true),
            (SandboxMode::Required, SandboxAvailability::Available, true),
            (SandboxMode::Off, SandboxAvailability::Available, false),
            (
                SandboxMode::Auto,
                SandboxAvailability::Unavailable("nested".to_string()),
                false,
            ),
        ] {
            let diagnosis = diagnosis_for_test(mode, policy(), availability);
            assert_eq!(diagnosis.status.applied, enforced);
            assert_eq!(diagnosis.applied_policy().is_some(), enforced);
            assert_eq!(
                StageRunFacts::of_diagnosis(&config, &diagnosis),
                StageRunFacts::for_test(enforced, false),
                "{mode:?}"
            );
        }
    }

    /// An unverified ledger is not a pass.
    #[test]
    fn only_pass_and_the_undeclared_or_unreached_pass_the_report() {
        for (status, failed) in [
            (Status::Pass, false),
            (Status::NotDeclared, false),
            (Status::NotChecked, false),
            (Status::Fail, true),
            (Status::Unverified, true),
        ] {
            let report = Report::of(vec![check("x", status, None, "")], Vec::new());
            assert_eq!(report.failed(), failed, "{status:?}");
        }
    }
}
