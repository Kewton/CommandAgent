use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::Parser as _;
use serde_json::Value;

use super::{
    Check, CheckId, LedgerFound, NOT_CHECKED_BY_DESIGN, OwnCode, ProbeCleanup, Report,
    SCHEMA_VERSION, Status, check, ledger_check, probe_cleanup_check,
};
use crate::structured_budget::admission::AdmissionRefusal;
use crate::structured_budget::entry::{EntryRefusal, UnlockedLedger, Unmetered, WalletCheck};
use crate::structured_budget::ledger::{LedgerEntry, LedgerRefusal};
use crate::structured_budget::policy::PolicyRefusal;
use crate::structured_budget::policy_v2::PolicyV2Refusal;
use crate::structured_budget::responses::AccountingBasis;
use crate::tools::profile_tools::RefusalKind;
use crate::tools::profile_tools::descriptor::{PinKind, ShownPin};

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
        let report = Report::of(vec![check(CheckId::Pack, status, None, "")], Vec::new());
        assert_eq!(report.failed(), failed, "{status:?}");
    }
}

fn wallet(ledger: UnlockedLedger, entry: LedgerEntry) -> WalletCheck {
    WalletCheck {
        budget_id: "x".to_string(),
        basis: AccountingBasis::UsageTariff,
        entry,
        ledger,
    }
}

/// PM condition 3 (with the ledger's own seam test, which drives two reads that differ): a
/// ledger that changed while it was read is `unverified`, and the report fails. Every form of
/// the ledger check, the one not reached included, says that its read takes no lock.
#[test]
fn a_ledger_that_changed_while_read_fails_and_every_ledger_check_says_unlocked() {
    let changed = wallet(UnlockedLedger::ChangedWhileRead, LedgerEntry::Fresh);
    let found = ledger_check(LedgerFound::Found(&changed));
    assert_eq!(found.status, Status::Unverified);
    assert_eq!(found.code.as_deref(), Some("changed_while_read"));
    assert!(Report::of(vec![found.clone()], Vec::new()).failed());
    let continues = wallet(UnlockedLedger::Continues, LedgerEntry::Resume);
    let absent = wallet(UnlockedLedger::Absent, LedgerEntry::Fresh);
    for found in [
        found,
        ledger_check(LedgerFound::Found(&continues)),
        ledger_check(LedgerFound::Found(&absent)),
        ledger_check(LedgerFound::Refused("Wallet(Corrupt)")),
        ledger_check(LedgerFound::NotReached),
    ] {
        assert_eq!(found.data["read_without_lock"], true, "{found:?}");
        assert!(
            found.detail.contains("without the wallet's lock"),
            "{found:?}"
        );
    }
    assert_eq!(
        ledger_check(LedgerFound::NotReached).status,
        Status::NotChecked
    );
}

fn config_of(arguments: &[&str]) -> crate::config::Config {
    let cli = crate::cli::Cli::try_parse_from(arguments).unwrap();
    crate::config::Config::from_cli_with_provider_options(cli, Default::default()).unwrap()
}

fn checked_config_of(arguments: &[&str]) -> crate::config::Config {
    let cli = crate::cli::Cli::try_parse_from(arguments).unwrap();
    super::resolve(cli, Default::default()).unwrap()
}

/// codex-3 S1: the check resolves its configuration as a `--prompt` run does, before the
/// resolution, so the defaults that follow the action are that run's; flags and a preset keep
/// what they say.
#[test]
fn the_configuration_is_resolved_as_a_prompt_run() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = std::fs::canonicalize(temp.path()).unwrap();
    std::fs::create_dir_all(workspace.join(".commandagent")).unwrap();
    std::fs::write(
        workspace.join(".commandagent/config.toml"),
        "[preset.quiet]\nstream = \"on\"\n",
    )
    .unwrap();
    let cwd = workspace.to_str().unwrap();
    let base = ["commandagent", "--provider", "lm-studio", "--cwd", cwd];
    for extra in [&[][..], &["--stream", "on"][..], &["--preset", "quiet"][..]] {
        let checked = checked_config_of(&[&base[..], extra, &["--operator-check"][..]].concat());
        let run = config_of(&[&base[..], extra, &["--prompt", "x"][..]].concat());
        assert!(
            matches!(checked.action, crate::config::Action::Prompt(ref goal) if goal.is_empty()),
            "{extra:?}"
        );
        assert_eq!(checked.stream, run.stream, "{extra:?}");
        assert_eq!(
            checked.field_sources.stream, run.field_sources.stream,
            "{extra:?}"
        );
    }
    let default = checked_config_of(&[&base[..], &["--operator-check"][..]].concat());
    assert!(!default.stream);
    assert_eq!(default.field_sources.stream, "default:non_interactive");
    let flag =
        checked_config_of(&[&base[..], &["--stream", "on", "--operator-check"][..]].concat());
    assert!(flag.stream);
}

// --- M2: every key and fixed word the report can carry is reserved -----------------------------

fn statuses() -> Vec<Status> {
    use Status::*;
    let all = vec![Pass, Fail, Unverified, NotDeclared, NotChecked];
    for status in &all {
        match status {
            Pass | Fail | Unverified | NotDeclared | NotChecked => {}
        }
    }
    all
}

fn check_ids() -> Vec<CheckId> {
    use CheckId::*;
    let all = vec![
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
    ];
    for id in &all {
        match id {
            Controls | Pack | ProfileTools | ProbeCleanup | OsSandbox | ArtifactContract
            | StructuredStages | Declaration | Ledger | RouteExecutor | RoutePlanner
            | RequestPattern | SecretScope => {}
        }
    }
    all
}

fn own_codes() -> Vec<OwnCode> {
    use OwnCode::*;
    let all = vec![
        Absent,
        Continues,
        ChangedWhileRead,
        ProbeAreaLeft,
        Applied,
        NotApplied,
        PackRefused,
        ArtifactContractRefused,
        SecretScopeRefused,
    ];
    for code in &all {
        match code {
            Absent
            | Continues
            | ChangedWhileRead
            | ProbeAreaLeft
            | Applied
            | NotApplied
            | PackRefused
            | ArtifactContractRefused
            | SecretScopeRefused => {}
        }
    }
    all
}

fn refusal_kinds() -> Vec<RefusalKind> {
    use RefusalKind::*;
    let all = vec![
        UnknownToolId,
        NameCollision,
        ConflictingDeclaration,
        InvalidPin,
        WorkspaceDescriptor,
        InvalidDescriptor,
        RegistryUnavailable,
        Unavailable,
        SelectedPackInvalid,
        SchemaUnsupported,
        SchemaLimitExceeded,
        SchemaScrubbed,
        SchemaMismatch,
        InputRevisionUnavailable,
        InputRevisionMismatch,
        SchemaViolation,
        EffectUnavailable,
        EffectInvalidPin,
        EffectInvalidDescriptor,
        EffectConflictingDeclaration,
        EffectWorkspaceDescriptor,
        EffectUnknownToolId,
        EffectRegistryUnavailable,
        EffectNameCollision,
        EffectReadOnly,
        EffectLimitExceeded,
        EffectOutputRefused,
        EffectPathRefused,
        EffectEnvRefused,
        ProcessFailed,
        ResultRejected,
        CommitFailed,
        StageContractRefused,
        StageLedgerRefused,
        StageRunEnded,
        StageIncomplete,
    ];
    for kind in &all {
        match kind {
            UnknownToolId
            | NameCollision
            | ConflictingDeclaration
            | InvalidPin
            | WorkspaceDescriptor
            | InvalidDescriptor
            | RegistryUnavailable
            | Unavailable
            | SelectedPackInvalid
            | SchemaUnsupported
            | SchemaLimitExceeded
            | SchemaScrubbed
            | SchemaMismatch
            | InputRevisionUnavailable
            | InputRevisionMismatch
            | SchemaViolation
            | EffectUnavailable
            | EffectInvalidPin
            | EffectInvalidDescriptor
            | EffectConflictingDeclaration
            | EffectWorkspaceDescriptor
            | EffectUnknownToolId
            | EffectRegistryUnavailable
            | EffectNameCollision
            | EffectReadOnly
            | EffectLimitExceeded
            | EffectOutputRefused
            | EffectPathRefused
            | EffectEnvRefused
            | ProcessFailed
            | ResultRejected
            | CommitFailed
            | StageContractRefused
            | StageLedgerRefused
            | StageRunEnded
            | StageIncomplete => {}
        }
    }
    all
}

fn policy_refusals() -> Vec<PolicyRefusal> {
    use PolicyRefusal::*;
    let all = vec![
        TooLarge,
        Malformed,
        WrongProtocol,
        InvalidBudgetId,
        BudgetIdDiffers,
        InvalidAmount,
        FallbacksAllowed,
        ReasoningNotInOutput,
        UnsupportedApi,
        InvalidRoute,
        InvalidTariff,
        ZeroBound,
        InvalidLedgerDir,
        LimitTooLarge,
    ];
    for refusal in &all {
        match refusal {
            TooLarge | Malformed | WrongProtocol | InvalidBudgetId | BudgetIdDiffers
            | InvalidAmount | FallbacksAllowed | ReasoningNotInOutput | UnsupportedApi
            | InvalidRoute | InvalidTariff | ZeroBound | InvalidLedgerDir | LimitTooLarge => {}
        }
    }
    all
}

fn policy_v2_refusals() -> Vec<PolicyV2Refusal> {
    use PolicyV2Refusal::*;
    let all = vec![
        TooLarge,
        Malformed,
        WrongProtocol,
        WrongAccountingBasis,
        InvalidBudgetId,
        BudgetIdDiffers,
        InvalidAmount,
        UnsupportedApi,
        InvalidEndpoint,
        InvalidRoute,
        InvalidResponseModels,
        UnsupportedServiceTier,
        InvalidPriceConditions,
        InvalidTariff,
        InvalidCacheWriteMode,
        CacheWritePricesDiffer,
        InvalidTiers,
        InvalidInputBound,
        ZeroBound,
        InvalidLedgerDir,
        LimitTooLarge,
    ];
    for refusal in &all {
        match refusal {
            TooLarge
            | Malformed
            | WrongProtocol
            | WrongAccountingBasis
            | InvalidBudgetId
            | BudgetIdDiffers
            | InvalidAmount
            | UnsupportedApi
            | InvalidEndpoint
            | InvalidRoute
            | InvalidResponseModels
            | UnsupportedServiceTier
            | InvalidPriceConditions
            | InvalidTariff
            | InvalidCacheWriteMode
            | CacheWritePricesDiffer
            | InvalidTiers
            | InvalidInputBound
            | ZeroBound
            | InvalidLedgerDir
            | LimitTooLarge => {}
        }
    }
    all
}

fn ledger_refusals() -> Vec<LedgerRefusal> {
    use LedgerRefusal::*;
    let all = vec![
        Missing,
        Corrupt,
        Held,
        PolicyChanged,
        WorkspaceChanged,
        ProfileChanged,
        ProfileBlank,
        DirectoryInsideWorkspace,
        DirectoryWritableByCommands,
        SandboxPolicyUnknown,
        AccountingBasisChanged,
        Vanished,
        Diverged,
        Unavailable,
    ];
    for refusal in &all {
        match refusal {
            Missing
            | Corrupt
            | Held
            | PolicyChanged
            | WorkspaceChanged
            | ProfileChanged
            | ProfileBlank
            | DirectoryInsideWorkspace
            | DirectoryWritableByCommands
            | SandboxPolicyUnknown
            | AccountingBasisChanged
            | Vanished
            | Diverged
            | Unavailable => {}
        }
    }
    all
}

fn unmetered() -> Vec<Unmetered> {
    use Unmetered::*;
    let all = vec![
        Workflow,
        ModelProbe,
        RequestPattern,
        GuiSend,
        ModelCompletion,
        ProviderCheck,
        StartupProbe,
    ];
    for way in &all {
        match way {
            Workflow | ModelProbe | RequestPattern | GuiSend | ModelCompletion | ProviderCheck
            | StartupProbe => {}
        }
    }
    all
}

/// Every word a run budget refusal puts in `code`: its `Debug` text, nested values included.
fn budget_codes() -> Vec<String> {
    let mut all: Vec<EntryRefusal> = vec![
        EntryRefusal::SandboxOff,
        EntryRefusal::KeyReachesCommands,
        EntryRefusal::KeyIsControl,
        EntryRefusal::InvalidBudgetId,
        EntryRefusal::NoOperatorRoot,
        EntryRefusal::PolicyUnreadable,
        EntryRefusal::PolicyInCommandReach,
        EntryRefusal::NoSandboxPolicy,
    ];
    all.extend(unmetered().into_iter().map(EntryRefusal::Unmetered));
    all.extend(policy_refusals().into_iter().map(EntryRefusal::Policy));
    all.extend(policy_v2_refusals().into_iter().map(EntryRefusal::PolicyV2));
    all.extend(ledger_refusals().into_iter().map(EntryRefusal::Wallet));
    for refusal in &all {
        match refusal {
            EntryRefusal::Unmetered(_)
            | EntryRefusal::SandboxOff
            | EntryRefusal::KeyReachesCommands
            | EntryRefusal::KeyIsControl
            | EntryRefusal::InvalidBudgetId
            | EntryRefusal::NoOperatorRoot
            | EntryRefusal::PolicyUnreadable
            | EntryRefusal::PolicyInCommandReach
            | EntryRefusal::Policy(_)
            | EntryRefusal::PolicyV2(_)
            | EntryRefusal::NoSandboxPolicy
            | EntryRefusal::Wallet(_) => {}
        }
    }
    let admission = [
        AdmissionRefusal::SelectedWithNoWallet,
        AdmissionRefusal::RouteNotCountable,
    ];
    for refusal in &admission {
        match refusal {
            AdmissionRefusal::SelectedWithNoWallet | AdmissionRefusal::RouteNotCountable => {}
        }
    }
    all.iter()
        .map(|refusal| format!("{refusal:?}"))
        .chain(admission.iter().map(|refusal| format!("{refusal:?}")))
        .collect()
}

/// Every report the check can print, as its shape: every check id, status, and code, every
/// `data` form, and every pin kind, with each operator-chosen or free value a placeholder shorter
/// than any secret.
fn every_emitted_shape() -> Vec<Value> {
    const SHORT: &str = "x";
    let mut codes: Vec<String> = own_codes()
        .into_iter()
        .map(|code| code.as_str().to_string())
        .collect();
    codes.extend(
        refusal_kinds()
            .into_iter()
            .map(|kind| kind.as_str().to_string()),
    );
    codes.extend(budget_codes());
    let mut checks: Vec<Check> = Vec::new();
    for (index, id) in check_ids().into_iter().enumerate() {
        for status in statuses() {
            checks.push(check(id, status, Some(&codes[index % codes.len()]), SHORT));
        }
    }
    for code in &codes {
        checks.push(check(CheckId::Pack, Status::Fail, Some(code), SHORT));
    }
    let left = ProbeCleanup {
        made: 1,
        left: vec![PathBuf::from(SHORT)],
    };
    checks.push(probe_cleanup_check(&left));
    for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
        for ledger in [
            UnlockedLedger::Absent,
            UnlockedLedger::Continues,
            UnlockedLedger::ChangedWhileRead,
        ] {
            checks.push(ledger_check(LedgerFound::Found(&wallet(ledger, entry))));
        }
    }
    let mut sandbox = check(CheckId::OsSandbox, Status::Pass, None, SHORT);
    sandbox.data = serde_json::json!({ "applied": true, "created": false });
    checks.push(sandbox);
    let pins = PinKind::all()
        .into_iter()
        .map(|pin| ShownPin {
            tool_id: SHORT.to_string(),
            pin,
            path: Some(PathBuf::from(SHORT)),
            declared: Some(SHORT.to_string()),
            calculated: Some(SHORT.to_string()),
            unavailable: Some(SHORT.to_string()),
        })
        .collect();
    // A detail is free text, and the parent of the temporary area is the host's: neither is a word.
    for check in &mut checks {
        check.detail = SHORT.to_string();
        if check.data.get("temporary_parent").is_some() {
            check.data["temporary_parent"] = Value::String(SHORT.to_string());
        }
    }
    let mut report = Report::of(checks, pins);
    report.status = Status::Fail;
    let mut value = serde_json::to_value(&report).unwrap();
    // The sentences are free text, not words: a scrub of one leaves the report readable.
    value["not_checked_by_design"] = Value::Array(Vec::new());
    vec![value]
}

fn words(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::String(word) => {
            out.insert(word.clone());
        }
        Value::Array(items) => items.iter().for_each(|item| words(item, out)),
        Value::Object(object) => {
            for (key, item) in object {
                out.insert(key.clone());
                words(item, out);
            }
        }
        _ => {}
    }
}

/// M2: every key and fixed word the report can carry, long enough to be registered as a secret,
/// is one no secret can be, so the scrub over the whole report can never rewrite it.
#[test]
fn every_emitted_word_long_enough_to_be_a_secret_is_reserved() {
    let mut found = BTreeSet::new();
    for shape in every_emitted_shape() {
        words(&shape, &mut found);
    }
    found.insert(SCHEMA_VERSION.to_string());
    let missing = found
        .iter()
        .filter(|word| word.chars().count() >= crate::sensitive_data::MIN_GENERIC_SECRET_LEN)
        .filter(|word| !crate::sensitive_data::collides_with_fixed_identifier(word))
        // A top-level key the scrub already keeps by name (`FIXED_SCHEMA_KEYS`).
        .filter(|word| !crate::sensitive_data::is_fixed_schema_key(word))
        .cloned()
        .collect::<Vec<_>>();
    assert!(missing.is_empty(), "not reserved: {missing:?}");
    // The set is the real one: a sample of the words the reproduction found unreserved.
    for word in [
        "calculated",
        "judged_as",
        "read_without_lock",
        "temporary_parent",
        "not_checked",
        "run_budget.ledger",
        "changed_while_read",
        "dependency_directory",
        "RouteNotCountable",
        "Wallet(PolicyChanged)",
        "SandboxOff",
    ] {
        assert!(found.contains(word), "{word}");
    }
    assert!(!NOT_CHECKED_BY_DESIGN.is_empty());
}

/// M1 and M2 together: a credential registered in the run's scope is scrubbed from every value
/// of the report, and the report's keys, ids, statuses, and codes stay as they are.
#[test]
fn the_scrub_of_the_whole_report_keeps_its_structure() {
    let credential = "dummy-credential-1013-value";
    let mut catalog = crate::sensitive_data::SecretCatalog::new();
    catalog
        .register_credential("COMPAT_API_KEY", credential)
        .unwrap();
    let mut checks = vec![probe_cleanup_check(&ProbeCleanup {
        made: 1,
        left: vec![PathBuf::from(format!("/tmp/{credential}"))],
    })];
    checks.push(ledger_check(LedgerFound::Refused("Wallet(PolicyChanged)")));
    let pins = vec![ShownPin {
        tool_id: "tool".to_string(),
        pin: PinKind::DependencyDirectory,
        path: Some(PathBuf::from(format!("/tools/{credential}"))),
        declared: Some(credential.to_string()),
        calculated: Some("calculated-value".to_string()),
        unavailable: None,
    }];
    let report = Report::of(checks, pins);
    let mut value = serde_json::to_value(&report).unwrap();
    let before = value.clone();
    catalog.scrub_value_lenient(&mut value);
    assert!(!value.to_string().contains(credential), "{value}");
    let mut keys_before = BTreeSet::new();
    let mut keys_after = BTreeSet::new();
    words(&before, &mut keys_before);
    words(&value, &mut keys_after);
    for word in keys_before.iter().filter(|word| !word.contains(credential)) {
        assert!(keys_after.contains(word), "{word} was rewritten");
    }
    assert_eq!(value["checks"][1]["data"]["read_without_lock"], true);
    assert_eq!(value["checks"][1]["code"], "Wallet(PolicyChanged)");
    assert_eq!(value["pins"][0]["pin"], "dependency_directory");
}
