//! `--operator-check` (Issue #1013): the run's own admission of the operator root's declarations,
//! with no provider call, leaving nothing behind.
//!
//! The configuration is resolved as a non-interactive `--prompt` run resolves it, and judged as
//! one. Every check calls the function the run calls. Where that function writes, its owner module
//! splits the judging part out and both the run and this check call it:
//!
//! - the OS sandbox is diagnosed, not installed (`os_sandbox::diagnose`): no
//!   `<state_dir>/sandbox/run-*` is made, and the process-wide state is not set;
//! - the run budget's declaration is read and decoded as the run reads it, and its wallet is never
//!   opened: no directory, no lock, no ledger written (`entry::check_without_opening`). A ledger
//!   already there is read without the lock and held to the declaration by the run's own
//!   continuation; the report always says the read was not locked;
//! - the executor's route is admitted by the basis of the declaration (`admission::admit_route_on`),
//!   the same choice the client factory makes, and no client is built;
//! - a stage set is judged with the sandbox as diagnosed (`StageRunFacts::of_diagnosis`), and no
//!   stage ledger is opened;
//! - the request-pattern setting is resolved as the run resolves it and judged by the same refusal
//!   (`request_pattern::refuse_unmetered`), and the secret scope is built as the run builds it;
//! - the isolation probe a proposal tool's admission runs is the same probe, under watch. It makes
//!   a temporary area under the system temporary directory, holds a loopback socket, and starts a
//!   sandboxed child; the area is removed here and looked at again, and one left behind fails the
//!   check. An interrupt during the probe can leave that empty area, as the run's own probe can.
//!
//! The whole report goes through the run's secret scrub once, just before it is printed.
//!
//! A pass means: the declarations, the configuration, and the probes are accepted now. It promises
//! nothing about a later run; see [`NOT_CHECKED_BY_DESIGN`].

use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::Cli;
use crate::config::{Config, ProviderCliOptions, ProviderRole};
use crate::planner::structured_stages::contract::StageRunFacts;
use crate::structured_budget::entry::{self, UnlockedLedger, WalletCheck};
use crate::structured_budget::ledger::{CommandWrites, LedgerEntry};
use crate::structured_budget::responses::AccountingBasis;
use crate::tools::os_sandbox::{self, SandboxDiagnosis};
use crate::tools::profile_tools::admission::AdmittedProfileTools;
use crate::tools::profile_tools::descriptor::{self, PinKind, ShownPin};
use crate::tools::profile_tools::isolation::probe_area::{self, ProbeCleanup};
use crate::tools::profile_tools::pre_run;

pub(crate) const SCHEMA_VERSION: &str = "commandagent.operator-check/v1";

/// What no check without a run can see. Shown in every report and in the docs.
pub(crate) const NOT_CHECKED_BY_DESIGN: &[&str] = &[
    "provider connectivity and authentication, including whether the provider key is set",
    "the model, service tier, and usage the provider actually returns",
    "the real request body, its token bound, and the reservation",
    "late responses and the latches they set",
    "creating the sandbox temporary, the wallet directory and lock, and saving the ledger at the real start",
    "whether another process holds the wallet's lock or the workspace lock now",
    "the existing state of the stage ledger",
    "a session to resume",
    "the schema-tool preflight against the built client's own wire; the check uses the configured executor's wire",
    "pins changed after this check; a run compares them again before each process",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
    Pass,
    Fail,
    /// Read, but nothing can be vouched for: a ledger that changed while it was read.
    Unverified,
    NotDeclared,
    /// Not reached, or not part of the judged run.
    NotChecked,
}

impl Status {
    const fn blocks(self) -> bool {
        matches!(self, Self::Fail | Self::Unverified)
    }
}

/// Every check, by its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckId {
    Controls,
    Pack,
    ProfileTools,
    ProbeCleanup,
    OsSandbox,
    ArtifactContract,
    StructuredStages,
    Declaration,
    Ledger,
    RouteExecutor,
    RoutePlanner,
    RequestPattern,
    SecretScope,
}

impl CheckId {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Controls => "run_budget.controls",
            Self::Pack => "pack",
            Self::ProfileTools => "profile_tools",
            Self::ProbeCleanup => "isolation.probe_cleanup",
            Self::OsSandbox => "os_sandbox",
            Self::ArtifactContract => "artifact_contract",
            Self::StructuredStages => "structured_stages",
            Self::Declaration => "run_budget.declaration",
            Self::Ledger => "run_budget.ledger",
            Self::RouteExecutor => "run_budget.route.executor",
            Self::RoutePlanner => "run_budget.route.planner",
            Self::RequestPattern => "request_pattern",
            Self::SecretScope => "secret_scope",
        }
    }
}

/// The check's own words for an outcome no refusal type names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OwnCode {
    Absent,
    Continues,
    ChangedWhileRead,
    ProbeAreaLeft,
    Applied,
    NotApplied,
    PackRefused,
    ArtifactContractRefused,
    SecretScopeRefused,
}

impl OwnCode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Continues => "continues",
            Self::ChangedWhileRead => "changed_while_read",
            Self::ProbeAreaLeft => "probe_area_left",
            Self::Applied => "applied",
            Self::NotApplied => "not_applied",
            Self::PackRefused => "pack_refused",
            Self::ArtifactContractRefused => "artifact_contract_refused",
            Self::SecretScopeRefused => "secret_scope_refused",
        }
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

fn check(id: CheckId, status: Status, code: Option<&str>, detail: impl Into<String>) -> Check {
    Check {
        id: id.as_str(),
        status,
        code: code.map(str::to_string),
        detail: detail.into(),
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
}

/// The report as it is printed: the run's scrub over every key and value, once.
fn scrubbed(report: &Report, workspace: &std::path::Path) -> anyhow::Result<Value> {
    let mut value = serde_json::to_value(report)?;
    if let Some(context) = crate::sensitive_data::active_for(Some(workspace)) {
        context.scrub_value_lenient(&mut value);
    }
    scrub_strings(&mut value);
    Ok(value)
}

/// The staged configuration catalog as well, over every string the lenient scrub left.
fn scrub_strings(value: &mut Value) {
    match value {
        Value::String(text) => *text = crate::sensitive_data::scrub_everything(text),
        Value::Array(items) => items.iter_mut().for_each(scrub_strings),
        Value::Object(map) => map.values_mut().for_each(scrub_strings),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}

/// The human form, from the scrubbed report.
fn render_human(report: &Value) -> String {
    let mut lines = vec![format!(
        "CommandAgent operator check: {} (judged as a non-interactive --prompt run; no provider call, nothing left behind)",
        text(&report["status"])
    )];
    for check in report["checks"].as_array().into_iter().flatten() {
        let code = check["code"]
            .as_str()
            .map(|code| format!(" [{code}]"))
            .unwrap_or_default();
        lines.push(format!(
            "{:<12} {}{code}: {}",
            text(&check["status"]),
            text(&check["id"]),
            text(&check["detail"])
        ));
    }
    let pins = report["pins"].as_array().cloned().unwrap_or_default();
    if !pins.is_empty() {
        lines.push("Pins (shown only; nothing is written):".to_string());
        for pin in &pins {
            let path = pin["path"]
                .as_str()
                .map(|path| format!(" {path}"))
                .unwrap_or_default();
            let declared = pin["declared"].as_str().unwrap_or("-");
            let calculated = pin["calculated"]
                .as_str()
                .map(str::to_string)
                .or_else(|| {
                    pin["unavailable"]
                        .as_str()
                        .map(|why| format!("unavailable: {why}"))
                })
                .unwrap_or_default();
            lines.push(format!(
                "  {} {}{path}: declared {declared}, calculated {calculated}",
                text(&pin["tool_id"]),
                text(&pin["pin"])
            ));
        }
    }
    lines.push("Not checked by design:".to_string());
    lines.extend(
        report["not_checked_by_design"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|item| format!("  - {}", text(item))),
    );
    lines.join("\n")
}

/// The CLI entry. A configuration the run would refuse to resolve is the run's exit-2 refusal.
pub(crate) fn run_cli(cli: Cli, provider_options: ProviderCliOptions) -> anyhow::Result<()> {
    let json = cli.json;
    let (report, workspace) = diagnose(cli, provider_options)?;
    let value = scrubbed(&report, &workspace)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{}", render_human(&value));
    }
    if report.failed() {
        anyhow::bail!("operator check found failed checks");
    }
    Ok(())
}

/// The configuration a non-interactive `--prompt` run resolves: the action is set before the
/// resolution, so every default that follows the action (the stream among them) is that run's.
/// The goal is empty, so nothing is inferred from its text.
pub(crate) fn resolve(
    mut cli: Cli,
    provider_options: ProviderCliOptions,
) -> anyhow::Result<Config> {
    cli.prompt = Some(String::new());
    Config::from_cli_with_provider_options(cli, provider_options)
}

fn diagnose(
    cli: Cli,
    provider_options: ProviderCliOptions,
) -> anyhow::Result<(Report, std::path::PathBuf)> {
    let sandbox_mode = cli.sandbox;
    let request_pattern = cli.request_pattern;
    let pack_cli = cli.clone();
    let config = resolve(cli, provider_options).map_err(crate::cli_argument_error)?;
    let pattern =
        crate::minimal_loop::request_pattern::settings(request_pattern, &config.workspace_root)
            .map_err(crate::cli_argument_error)?;
    let secret_scope = crate::config::install_run_secret_scope(&config);
    let mut checks = vec![controls(sandbox_mode, &config)];

    let resolved_pack = crate::cli_pack::resolve(&pack_cli, &config);
    checks.push(match &resolved_pack {
        Ok(None) => check(CheckId::Pack, Status::NotDeclared, None, "no pack selected"),
        Ok(Some(pack)) => check(
            CheckId::Pack,
            Status::Pass,
            None,
            format!("pack `{}` {} resolved", pack.id, pack.version),
        ),
        Err(error) => check(
            CheckId::Pack,
            Status::Fail,
            Some(OwnCode::PackRefused.as_str()),
            format!("{error:#}"),
        ),
    });

    let (admitted, cleanup) =
        probe_area::watching(|| pre_run::admitted_before_cli_run(&config, &resolved_pack));
    checks.push(match &admitted {
        Ok(None) => check(
            CheckId::ProfileTools,
            Status::NotDeclared,
            None,
            "the profile and pack declare no tools",
        ),
        Ok(Some(set)) => check(
            CheckId::ProfileTools,
            Status::Pass,
            None,
            format!(
                "{} tool(s) admitted, schema and proposal preflight passed",
                set.bindings().len()
            ),
        ),
        Err(refusal) => check(
            CheckId::ProfileTools,
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
    checks.push(
        match crate::minimal_loop::request_pattern::refuse_unmetered(pattern) {
            Ok(()) => check(
                CheckId::RequestPattern,
                Status::Pass,
                None,
                "the request-pattern setting is one this run may use",
            ),
            Err(refusal) => check(
                CheckId::RequestPattern,
                Status::Fail,
                Some(&format!("{refusal:?}")),
                refusal.to_string(),
            ),
        },
    );
    checks.push(match &secret_scope {
        Ok(_) => check(
            CheckId::SecretScope,
            Status::Pass,
            None,
            "the run's secret scope can be built",
        ),
        Err(error) => check(
            CheckId::SecretScope,
            Status::Fail,
            Some(OwnCode::SecretScopeRefused.as_str()),
            format!("{error:#}"),
        ),
    });

    let pins = pins(&config, &resolved_pack);
    Ok((Report::of(checks, pins), config.workspace_root.clone()))
}

/// The refusals a run makes before and beside its configuration, whatever is declared.
fn controls(sandbox_mode: os_sandbox::SandboxMode, config: &Config) -> Check {
    let refused = entry::refuse_sandbox_off(sandbox_mode)
        .and_then(|()| entry::refuse_a_control_named_as_key(config))
        .and_then(|()| entry::refuse_a_control_the_start_takes());
    match refused {
        Ok(()) => check(
            CheckId::Controls,
            Status::Pass,
            None,
            "the sandbox mode and the key's variable leave the run budget's controls alone",
        ),
        Err(refusal) => check(
            CheckId::Controls,
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
            CheckId::ProbeCleanup,
            Status::NotChecked,
            None,
            "no isolation probe made a temporary area",
        )
    } else if cleanup.clean() {
        check(
            CheckId::ProbeCleanup,
            Status::Pass,
            None,
            format!(
                "{} probe area(s) in the system temporary directory, each removed",
                cleanup.made
            ),
        )
    } else {
        check(
            CheckId::ProbeCleanup,
            Status::Fail,
            Some(OwnCode::ProbeAreaLeft.as_str()),
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
        CheckId::OsSandbox,
        Status::Pass,
        Some(if status.applied {
            OwnCode::Applied.as_str()
        } else {
            OwnCode::NotApplied.as_str()
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
            CheckId::ArtifactContract,
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
            CheckId::ArtifactContract,
            Status::Pass,
            None,
            "no artifact contract adapter is declared, or every declared one is admitted",
        ),
        Err(reason) => check(
            CheckId::ArtifactContract,
            Status::Fail,
            Some(OwnCode::ArtifactContractRefused.as_str()),
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
            CheckId::StructuredStages,
            Status::NotChecked,
            None,
            "the profile tools were not admitted",
        );
    };
    let Some(admitted) = admitted else {
        return check(
            CheckId::StructuredStages,
            Status::NotDeclared,
            None,
            "no stage set is declared",
        );
    };
    match admitted.judge_stage_run(config, StageRunFacts::of_diagnosis(config, sandbox)) {
        Ok(None) => check(
            CheckId::StructuredStages,
            Status::NotDeclared,
            None,
            "no stage set is declared",
        ),
        Ok(Some(stages)) => check(
            CheckId::StructuredStages,
            Status::Pass,
            None,
            format!("{stages} stage(s) admitted for this run; no stage ledger was opened"),
        ),
        Err(refusal) => check(
            CheckId::StructuredStages,
            Status::Fail,
            Some(refusal.kind.as_str()),
            refusal.reason,
        ),
    }
}

/// What the ledger check found, including that it was not reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LedgerFound<'a> {
    Found(&'a WalletCheck),
    Refused(&'a str),
    NotReached,
}

fn run_budget_checks(config: &Config, sandbox: &SandboxDiagnosis) -> Vec<Check> {
    if !entry::selected() {
        let mut checks = vec![
            check(
                CheckId::Declaration,
                Status::NotDeclared,
                None,
                "no run budget is selected",
            ),
            ledger_check(LedgerFound::NotReached),
        ];
        checks.extend(route_checks(config, None, Status::NotDeclared));
        return checks;
    }
    let found = entry::check_without_opening(true, config, CommandWrites::diagnosed(sandbox));
    let (declaration, ledger, basis, otherwise) = match &found {
        Ok(None) => (
            check(
                CheckId::Declaration,
                Status::NotDeclared,
                None,
                "this entry opens no wallet",
            ),
            ledger_check(LedgerFound::NotReached),
            None,
            Status::NotDeclared,
        ),
        Ok(Some(wallet)) => (
            check(
                CheckId::Declaration,
                Status::Pass,
                None,
                format!(
                    "`{}` decoded, accounting basis {}",
                    wallet.budget_id,
                    basis_word(wallet.basis)
                ),
            ),
            ledger_check(LedgerFound::Found(wallet)),
            Some(wallet.basis),
            Status::Pass,
        ),
        Err(refusal @ entry::EntryRefusal::Wallet(_)) => (
            check(
                CheckId::Declaration,
                Status::Pass,
                None,
                "decoded; the wallet's place or record was refused",
            ),
            ledger_check(LedgerFound::Refused(&format!("{refusal:?}"))),
            None,
            Status::NotChecked,
        ),
        Err(refusal) => (
            check(
                CheckId::Declaration,
                Status::Fail,
                Some(&format!("{refusal:?}")),
                refusal.to_string(),
            ),
            ledger_check(LedgerFound::NotReached),
            None,
            Status::NotChecked,
        ),
    };
    let mut checks = vec![declaration, ledger];
    checks.extend(route_checks(config, basis, otherwise));
    checks
}

const fn basis_word(basis: AccountingBasis) -> &'static str {
    match basis {
        AccountingBasis::ProviderBilled => "provider_billed",
        AccountingBasis::UsageTariff => "usage_tariff",
    }
}

/// The one builder of the ledger check: whatever it found, or that it was not reached, it says
/// that a read of the ledger takes no lock.
pub(crate) fn ledger_check(found: LedgerFound<'_>) -> Check {
    const UNLOCKED: &str = "read without the wallet's lock";
    let (mut found, entry) = match found {
        LedgerFound::NotReached => (
            check(
                CheckId::Ledger,
                Status::NotChecked,
                None,
                format!("not reached; a ledger is always {UNLOCKED}"),
            ),
            None,
        ),
        LedgerFound::Refused(code) => (
            check(
                CheckId::Ledger,
                Status::Fail,
                Some(code),
                format!("{UNLOCKED}; the run's open refuses this wallet"),
            ),
            None,
        ),
        LedgerFound::Found(wallet) => {
            let found = match wallet.ledger {
                UnlockedLedger::Absent => check(
                    CheckId::Ledger,
                    Status::Pass,
                    Some(OwnCode::Absent.as_str()),
                    format!(
                        "{UNLOCKED}; no ledger is there yet, and a fresh run's open would make it"
                    ),
                ),
                UnlockedLedger::Continues => check(
                    CheckId::Ledger,
                    Status::Pass,
                    Some(OwnCode::Continues.as_str()),
                    format!(
                        "{UNLOCKED}; the ledger there continues under this declaration, workspace, and profile"
                    ),
                ),
                UnlockedLedger::ChangedWhileRead => check(
                    CheckId::Ledger,
                    Status::Unverified,
                    Some(OwnCode::ChangedWhileRead.as_str()),
                    format!(
                        "{UNLOCKED}; the file changed between two reads, so it is not vouched for"
                    ),
                ),
            };
            (found, Some(wallet.entry))
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

/// A `--prompt` run builds the executor's client only, so only its route counts. The planner's is
/// shown as not part of the judged run.
fn route_checks(config: &Config, basis: Option<AccountingBasis>, otherwise: Status) -> Vec<Check> {
    let executor = if basis.is_none() {
        let detail = match otherwise {
            Status::NotDeclared => "no run budget is selected",
            _ => "the declaration was not admitted",
        };
        check(CheckId::RouteExecutor, otherwise, None, detail)
    } else {
        match crate::structured_budget::admission::admit_route_on(
            config,
            ProviderRole::Executor,
            basis,
        ) {
            Ok(()) => check(
                CheckId::RouteExecutor,
                Status::Pass,
                None,
                "the wallet can count this route",
            ),
            Err(refusal) => check(
                CheckId::RouteExecutor,
                Status::Fail,
                Some(&format!("{refusal:?}")),
                refusal.to_string(),
            ),
        }
    };
    let planner = check(
        CheckId::RoutePlanner,
        Status::NotChecked,
        None,
        "not judged: a --prompt run builds no planner client",
    );
    vec![executor, planner]
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
                    pin: PinKind::DescriptorSha256,
                    path: None,
                    declared: None,
                    calculated: None,
                    unavailable: Some(refusal.to_string()),
                }],
            },
        )
        .collect()
}

#[cfg(test)]
#[path = "operator_check_tests.rs"]
mod tests;
