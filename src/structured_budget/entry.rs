//! Where a run comes in under a wallet, and every way out of one that nothing could count.
//!
//! With `COMMANDAGENT_RUN_BUDGET` absent, every function here returns before it reads anything
//! else: no operator root, no policy, no clock, no ledger. That is off, and off is as it was.
//!
//! With it present, a run that uses the write sandbox opens its wallet here, once, after the sandbox
//! is installed and applied: the ID from the variable, the operator's declaration from
//! `<operator root>/run-budgets/<id>.json` read with a bound and no link followed, the system clock,
//! the entry the run asked for, and the policy of the sandbox applied on it. The handle goes on the
//! run's configuration, and every clone of it carries the one wallet.
//!
//! An entry whose sends no wallet can count is refused before it sends: a workflow, a model probe,
//! request-pattern routing, the GUI's delegated runs and its model discovery, the CLI's model
//! completion, the doctor's provider checks, the startup probe, and a run with the OS sandbox off.
//! So is a key whose variable a command of the run would inherit. Bash's own network is not the
//! wallet's to count: the limit is on the provider calls this program makes.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::admission::RUN_BUDGET_ENV;
use super::clock::SystemBudgetClock;
use super::ledger::{CommandWrites, LedgerEntry, LedgerOpen, LedgerRefusal};
use super::policy::{PolicyRefusal, RunBudgetPolicy, valid_budget_id};
use super::transport::RunBudgetHandle;
use crate::config::{Action, Config};
use crate::tools::os_sandbox::SandboxMode;

/// Where the operator's declarations are, under the operator root.
const POLICY_DIRECTORY: &str = "run-budgets";
/// One byte past the largest declaration, so a larger one is seen and refused as too large.
const MOST_POLICY_BYTES_READ: u64 = (1 << 20) + 1;

/// A way a run sends that no wallet counts. Fixed words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unmetered {
    Workflow,
    ModelProbe,
    RequestPattern,
    GuiSend,
    ModelCompletion,
    ProviderCheck,
    StartupProbe,
}

/// Why an entry did not go on. Fixed words only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryRefusal {
    Unmetered(Unmetered),
    /// The OS sandbox is off, so nothing keeps the run's commands from the wallet.
    SandboxOff,
    /// The variable of the provider key is one a command of the run inherits.
    KeyReachesCommands,
    /// The variable of the provider key is one the run budget is controlled by. On macOS the
    /// start hands a key's variable over and takes it out of the environment, so the control
    /// would be gone where it is read.
    KeyIsControl,
    InvalidBudgetId,
    /// No operator root is set, or it is not an absolute path.
    NoOperatorRoot,
    PolicyUnreadable,
    /// The declaration is somewhere a command of the run can write.
    PolicyInCommandReach,
    Policy(PolicyRefusal),
    /// No sandbox is applied on the run, so no place can be judged out of its commands' reach.
    NoSandboxPolicy,
    Wallet(LedgerRefusal),
}

impl std::fmt::Display for EntryRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "the run budget refused the entry: {self:?}")
    }
}

impl std::error::Error for EntryRefusal {}

/// Whether a wallet is selected. Its value, whatever it is, is the ID. The start of the command
/// line on macOS takes a provider key's variable out of the environment and holds its value, so
/// what it holds counts as much as the environment does: a selection is never lost there. Only
/// whether it is present is read here, never the value.
pub(crate) fn selected() -> bool {
    crate::startup_credentials::var_os(RUN_BUDGET_ENV).is_some()
}

/// Every variable the run budget is controlled by, as `structured_budget` reads it: the selection
/// (`admission.rs`, read by [`selected`] and [`open_for_run`]) and the operator root the
/// declaration is read under (`artifact_contract_checks::CONFIG_ROOT_ENV`, read by
/// [`read_policy`]). `HOME`, which the sandbox's caches are found under, is refused as a key by
/// [`commands_inherit`] already.
const CONTROL_NAMES: [&str; 2] = [
    RUN_BUDGET_ENV,
    crate::planner::artifact_contract_checks::CONFIG_ROOT_ENV,
];

/// The provider key's variable may not be a control of the run budget, or the legacy spelling of
/// one, whether a wallet is selected or not: on macOS a selection under that name is taken out of
/// the environment before anything here could see it. Only names are compared; nothing is read.
/// Off, a run is refused only when its key is named so.
pub(crate) fn refuse_a_control_named_as_key(config: &Config) -> Result<(), EntryRefusal> {
    let key = config
        .openai_compatible
        .as_ref()
        .and_then(|compatible| compatible.api_key_env.as_deref());
    match key {
        Some(name)
            if CONTROL_NAMES
                .iter()
                .any(|control| crate::env_compat::matches_current_or_legacy(name, control)) =>
        {
            Err(EntryRefusal::KeyIsControl)
        }
        _ => Ok(()),
    }
}

/// Refuses a way out of the wallet when one is selected. With none selected it reads nothing more.
pub(crate) fn refuse(way: Unmetered) -> Result<(), EntryRefusal> {
    if selected() {
        return Err(EntryRefusal::Unmetered(way));
    }
    Ok(())
}

/// The GUI's delegated runs and its model discovery: the delegated CLI gets an environment of its
/// own, so the selection would not reach it, and a model list is a send no wallet counts.
pub fn refuse_unmetered_gui_send() -> anyhow::Result<()> {
    refuse(Unmetered::GuiSend).map_err(anyhow::Error::new)
}

/// A run with the OS sandbox off holds its commands nowhere.
pub(crate) fn refuse_sandbox_off(mode: SandboxMode) -> Result<(), EntryRefusal> {
    if selected() && mode == SandboxMode::Off {
        return Err(EntryRefusal::SandboxOff);
    }
    Ok(())
}

/// Opens the run's wallet, once, after the sandbox is installed. `uses_write_sandbox` is whether
/// the action runs commands under it: one that does not has no sandbox to judge a wallet by, and
/// opens none. A workflow opens none either: it is refused where it starts.
pub(crate) fn open_for_run(
    uses_write_sandbox: bool,
    config: &mut Config,
) -> Result<(), EntryRefusal> {
    open_noting(uses_write_sandbox, config, &mut |_| {})
}

/// [`open_for_run`], telling `touch` each thing it reads, in order, for a test that counts them.
fn open_noting(
    uses_write_sandbox: bool,
    config: &mut Config,
    touch: &mut dyn FnMut(&'static str),
) -> Result<(), EntryRefusal> {
    if !selected() {
        return Ok(());
    }
    if !uses_write_sandbox
        || matches!(config.action, Action::Workflow(..))
        || config.field_sources.run_budget.is_some()
    {
        return Ok(());
    }
    let id = std::env::var(RUN_BUDGET_ENV)
        .ok()
        .filter(|id| valid_budget_id(id))
        .ok_or(EntryRefusal::InvalidBudgetId)?;
    refuse_a_key_commands_inherit(config)?;
    let writes = CommandWrites::applied().ok_or(EntryRefusal::NoSandboxPolicy)?;
    let policy = read_policy(&id, &writes, touch)?;
    touch("clock");
    let clock = Arc::new(SystemBudgetClock::new());
    let entry = if config.resume.is_some() && !config.fresh_session {
        LedgerEntry::Resume
    } else {
        LedgerEntry::Fresh
    };
    touch("ledger");
    let handle = RunBudgetHandle::open(LedgerOpen {
        policy: Arc::new(policy),
        workspace: &config.workspace_root,
        explicit_profile: config.profile_explicit.then_some(config.profile.as_str()),
        entry,
        clock,
        command_writes: Some(writes),
    })
    .map_err(EntryRefusal::Wallet)?;
    config.field_sources.run_budget = Some(Arc::new(handle));
    Ok(())
}

/// The operator's declaration of `id`, read with a bound through a walk that follows no link below
/// the root, from a place no command of the run can write.
fn read_policy(
    id: &str,
    writes: &CommandWrites,
    touch: &mut dyn FnMut(&'static str),
) -> Result<RunBudgetPolicy, EntryRefusal> {
    touch("root");
    let root = crate::planner::artifact_contract_checks::config_root_from_environment()
        .filter(|root| root.is_absolute())
        .ok_or(EntryRefusal::NoOperatorRoot)?;
    let root = std::fs::canonicalize(root).map_err(|_| EntryRefusal::NoOperatorRoot)?;
    let relative = Path::new(POLICY_DIRECTORY).join(format!("{id}.json"));
    if within_command_reach(&root.join(&relative), writes) {
        return Err(EntryRefusal::PolicyInCommandReach);
    }
    touch("policy");
    let file = crate::tools::dir_fd::open_read_file(&root, &relative)
        .map_err(|_| EntryRefusal::PolicyUnreadable)?;
    let mut bytes = Vec::new();
    file.take(MOST_POLICY_BYTES_READ)
        .read_to_end(&mut bytes)
        .map_err(|_| EntryRefusal::PolicyUnreadable)?;
    RunBudgetPolicy::decode(&bytes, id).map_err(EntryRefusal::Policy)
}

/// Whether `path` is under the workspace, the run's temporary, or a cache the sandbox lets its
/// commands write, each as it really leads.
fn within_command_reach(path: &Path, writes: &CommandWrites) -> bool {
    let policy = writes.policy();
    let caches = &policy.caches;
    let roots: Vec<&PathBuf> = [&policy.workspace_root, &policy.run_tmp_dir]
        .into_iter()
        .chain(
            [
                &caches.cargo_registry,
                &caches.cargo_git,
                &caches.cargo_package_cache,
                &caches.npm,
                &caches.pip,
                &caches.playwright,
            ]
            .into_iter()
            .flatten(),
        )
        .collect();
    roots.into_iter().any(|root| {
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        path.starts_with(root)
    })
}

/// The provider key's variable may not be one a Bash command of the run inherits from this
/// process, nor one this program hands a command itself.
fn refuse_a_key_commands_inherit(config: &Config) -> Result<(), EntryRefusal> {
    let key = config
        .openai_compatible
        .as_ref()
        .and_then(|compatible| compatible.api_key_env.as_deref());
    match key {
        Some(name) if commands_inherit(name) || commands_are_handed(name) => {
            Err(EntryRefusal::KeyReachesCommands)
        }
        _ => Ok(()),
    }
}

/// What `bounded_process` passes from this process's environment to every child it spawns, as it
/// is written there. This is a copy, since the list there is private and `bounded_process` is not
/// this module's to change. A test holds the two together: it reads every name and prefix written
/// in `child_env_allowed`, with the near misses beside each (another case, the legacy spelling,
/// one character more, a prefix's own edge), runs a real child with each, and requires every name
/// a child gets to be one refused here. What it cannot see is a rule that is not written as a
/// string in that function.
pub(crate) fn commands_inherit(name: &str) -> bool {
    matches!(
        name,
        "PATH" | "HOME" | "LANG" | "TERM" | "RUSTUP_HOME" | "CARGO_HOME"
    ) || name.starts_with("LC_")
        || name.starts_with("npm_config_")
}

/// What `bounded_process` lets a caller set on a child, beside what it inherits. A key under one of
/// these names could be handed on by whoever sets it.
fn commands_are_handed(name: &str) -> bool {
    matches!(
        name,
        "NODE_ENV"
            | "NODE_OPTIONS"
            | "NODE_PATH"
            | "PLAYWRIGHT_BROWSERS_PATH"
            | "NEXT_TELEMETRY_DISABLED"
            | "PORT"
            | "TMPDIR"
    ) || [
        "COMMANDAGENT_FOREIGN_TOOLCHAIN_ROOT",
        "COMMANDAGENT_FOREIGN_TOOLCHAIN_EVENTS",
    ]
    .iter()
    .any(|current| crate::env_compat::matches_current_or_legacy(name, current))
}

#[cfg(test)]
#[path = "entry_tests.rs"]
mod tests;
