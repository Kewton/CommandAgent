//! Stage obligations bound to host receipts and input revisions (Issue #975, G4).
//!
//! A profile or pack can declare stages over its propose-artifacts tools. A stage is complete
//! only when, for its current inputs, the host adopted the bytes and every check the operator
//! requires passed. A new upstream version takes the state from everything below it.
//!
//! - [`contract`] admits the declaration before any provider call.
//! - [`state`] is the state machine: typed inputs, no file, no process, clock as an argument.
//! - [`invalidation`] computes what an upstream change takes away.
//! - [`receipt`] binds a target's state to G3's commit receipt without changing it.
//! - [`resume`] lowers a saved record. A saved record never raises a state.
//! - [`ledger`] keeps attempts and generations across sessions and for a later process.
//!
//! [`StageRun`] joins them to a set of admitted tools. A tool in no declared stage never reaches
//! any of this, so a run with no stage declaration reads and writes no record here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use contract::{
    AdmittedTarget, ContractRefusal, StageContract, StageContractDraft, StageRunFacts, TargetKey,
};
use ledger::{HeldLedger, LedgerEntry, LedgerRefusal, LedgerRegistry, LedgerRequest};
use receipt::{InputDigest, sha256_hex};
use resume::AdmittedPins;
use state::{AttemptRequest, BytesSource, ObservedBytes};

use crate::config::Config;
use crate::tools::profile_tools::admission::declared_stage_identity;
use crate::tools::profile_tools::commit::CommitReceipt;
use crate::tools::profile_tools::schema::binding::AdoptedInputs;
use crate::tools::profile_tools::schema::invocation::SchemaGuard;
use crate::tools::profile_tools::selection::{self, PackSelection};
use crate::tools::profile_tools::{ProfileToolRefusal, RefusalKind};

pub(crate) fn contract_refused(refusal: ContractRefusal) -> ProfileToolRefusal {
    ProfileToolRefusal::new(
        RefusalKind::StageContractRefused,
        format!(
            "the declared stages cannot be admitted: {:?} {}",
            refusal.kind, refusal.subject
        ),
    )
}

/// A fixed sentence by kind, with the path and the input's name in `data`: a later step reads a
/// message by its words, and a word in a path could make this end look like something to retry.
pub(crate) fn ledger_refused(refusal: LedgerRefusal) -> ProfileToolRefusal {
    let mut data = serde_json::json!({
        "code": refusal.code(),
        "recovery": refusal.recovery(),
    });
    if let LedgerRefusal::FixedInputChanged { path } = &refusal {
        data["input"] = serde_json::Value::String(path.clone());
    }
    ProfileToolRefusal {
        kind: RefusalKind::StageLedgerRefused,
        reason: format!(
            "the record of this run's stages cannot be used: {}. {}",
            refusal.code(),
            refusal.recovery()
        ),
        data: Some(data),
    }
}

/// One tool of a stage set: its pins as admitted, and the guard holding its adopted inputs.
#[derive(Debug)]
pub(crate) struct StageMember {
    pub(crate) pins: AdmittedPins,
    pub(crate) guard: Arc<SchemaGuard>,
}

#[derive(Debug, Clone)]
struct StageSession {
    contract: Arc<StageContract>,
    state_dir: PathBuf,
    entry: LedgerEntry,
}

/// What one tool call brings that the host cannot read elsewhere.
#[derive(Debug)]
pub(crate) struct StageCallFacts<'a> {
    pub(crate) turn: u64,
    pub(crate) now_ms: u64,
    pub(crate) expected_revision: &'a str,
    pub(crate) arguments_sha256: String,
    pub(crate) request_id: String,
    pub(crate) current_output: &'a ObservedBytes,
    pub(crate) adopted: &'a AdoptedInputs,
}

/// The stage set of one admission. Opening its ledger waits for the first call: an entry that
/// only admits, as a preflight does, holds no lock and touches no file.
#[derive(Debug)]
pub(crate) struct StageRun {
    draft: StageContractDraft,
    members: BTreeMap<String, StageMember>,
    workspace: PathBuf,
    profile: String,
    ledgers: Arc<LedgerRegistry>,
    session: Mutex<Option<StageSession>>,
    /// The targets this session's calls named, whatever came of them.
    touched: Mutex<BTreeSet<TargetKey>>,
    notes: Mutex<Vec<StageNote>>,
    /// Calls in a row that started no process. Nothing else limits them: they spend no attempt.
    calls_without_process: AtomicUsize,
    record_notice_shown: AtomicBool,
}

/// A call's effect on whether a stage moved, for the loop's stagnation count. This is the only
/// thing the loop learns of a stage call: it never reads an answer's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageNote {
    pub(crate) target: TargetKey,
    pub(crate) progress: bool,
}

/// Which obligations completion asks about.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum StageScope {
    /// Every obligation of the set: a run with no final acceptance after it, or that acceptance.
    #[default]
    All,
    /// A plan step answers for the stages it called. The rest is final acceptance's to judge.
    Touched,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnmetObligation {
    pub(crate) key: TargetKey,
    pub(crate) phase: state::Phase,
    pub(crate) revision: Option<String>,
}

/// Why completion does not stand. Only a fixed code and opaque IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageShortfall {
    pub(crate) code: &'static str,
    pub(crate) unmet: Vec<UnmetObligation>,
    /// What this very judgment took a state from, because the bytes had moved.
    pub(crate) invalidated: Vec<TargetKey>,
    /// Where the record is and what to do about it, when the record is what refused. Never in a
    /// message: a word in a path could make this end look like something to retry.
    pub(crate) record: Option<serde_json::Value>,
}

impl StageShortfall {
    const fn of(code: &'static str) -> Self {
        Self {
            code,
            unmet: Vec::new(),
            invalidated: Vec::new(),
            record: None,
        }
    }

    /// Whether another submission could change the answer. A record that cannot be vouched for
    /// and a run a secret ended cannot be put right by a model.
    pub(crate) fn can_be_resubmitted(&self) -> bool {
        !self.unmet.is_empty()
    }

    /// IDs only, which admission limits to a few plain characters. A repair prompt shows this.
    pub(crate) fn to_refusal(&self) -> ProfileToolRefusal {
        let names = self
            .unmet
            .iter()
            .map(|obligation| format!("{}/{}", obligation.key.stage, obligation.key.target))
            .collect::<Vec<_>>()
            .join(", ");
        let reason = if names.is_empty() {
            format!("{}: {} unmet", self.code, self.unmet.len())
        } else {
            format!("{}: {} unmet: {names}", self.code, self.unmet.len())
        };
        ProfileToolRefusal::new(RefusalKind::StageIncomplete, reason)
    }

    pub(crate) fn event(&self) -> serde_json::Value {
        let unmet = self
            .unmet
            .iter()
            .map(|obligation| {
                serde_json::json!({
                    "stage": obligation.key.stage,
                    "target": obligation.key.target,
                    "phase": obligation.phase,
                })
            })
            .collect::<Vec<_>>();
        let mut event = serde_json::json!({
            "event": "structured_stage_incomplete",
            "code": self.code,
            "unmet": unmet,
            "invalidated": self.invalidated,
        });
        if let Some(record) = &self.record {
            event["record"] = record.clone();
        }
        event
    }
}

/// A plan run by `/resume` is a resume to everything below it that asks.
pub(crate) fn config_resuming(config: &Config, resumed_from: &str) -> Config {
    let mut resuming = config.clone();
    resuming.resume = Some(resumed_from.to_string());
    resuming
}

/// Stage calls in a row that may start no process. `stale` and `not_ready` spend no attempt, so
/// without this only the session's iteration budget would stop a model repeating them (S8).
const MAX_CALLS_WITHOUT_PROCESS: usize = 8;

/// What a session's admission left for the one judgment made outside a session.
#[derive(Debug, Clone)]
enum BoundAdmission {
    Stages(Arc<StageRun>),
    /// A selection only an admission can read, read by a session, with no stage in it.
    NoStages,
}

const NO_SESSION_BOUND: &str = "stage_incomplete: no_session_bound_the_declared_stages";

/// What a stage set's seals were made under: its contract, and each tool's descriptor bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageIdentity {
    pub(crate) contract_sha256: String,
    /// By tool ID.
    pub(crate) descriptors: BTreeMap<String, String>,
}

/// Why the stages declared now could not be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeclaredStagesUnread {
    SelectedPack,
    Stages,
}

/// Another pack selection is another set of stages: one's seals say nothing about the other's. So
/// is another declaration of the profile, which a later extension root can make of the same name.
type BoundKey = (PathBuf, String, Option<&'static str>, Option<PackSelection>);

/// What each workspace, profile, declaration, and pack selection last admitted in this process.
/// Without a stage declaration and without a selection only an admission can read, nothing comes
/// here.
static BOUND_RUNS: Mutex<BTreeMap<BoundKey, BoundAdmission>> = Mutex::new(BTreeMap::new());

/// The one place a key is made, so a record and the gate that looks for it name a run alike. The
/// declaration is the registered manifest's hash, from memory: a builtin profile has none, and no
/// root can declare its name. `None` is a workspace whose path cannot be resolved.
fn bound_key(workspace: &Path, profile: &str, pack: Option<PackSelection>) -> Option<BoundKey> {
    let workspace = std::fs::canonicalize(workspace).ok()?;
    let declaration =
        crate::planner::extension_profiles::find(profile).map(|found| found.manifest_hash);
    Some((workspace, profile.to_string(), declaration, pack))
}

fn remember(workspace: &Path, profile: &str, pack: Option<PackSelection>, bound: BoundAdmission) {
    if let Some(key) = bound_key(workspace, profile, pack) {
        BOUND_RUNS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, bound);
    }
}

/// A session admitted no stage. A later final gate need not read the selection to know that.
pub(crate) fn remember_admission_without_stages(config: &Config) {
    if let Some(pack) = selection::selection_left_to_admission() {
        remember(
            &config.workspace_root,
            &config.profile,
            Some(pack),
            BoundAdmission::NoStages,
        );
    }
}

/// Final acceptance's question: every obligation of the set, by the bytes there now. It can only
/// lower a report. A pass here says nothing wider than that the obligations were met (D8). A run
/// memory says has no stage, with no declaration and no pack or a decoded pack with none, returns
/// at the first line, having read no table and touched no file. A pack only an admission can read
/// goes on to resolve the workspace's path and look in the table. A record of stages is then held
/// to what is declared now, which reads the declaration; a record of no stage reads nothing more.
pub(crate) fn final_gate_failure(config: &Config) -> Option<String> {
    // G5 before G4, with or without a stage (Issue #976).
    if let Some(refused) = crate::structured_budget::outcome::refusal_text(config) {
        return Some(refused);
    }
    let pack = selection::selection_that_may_hold_stages(&config.profile)?;
    let bound = bound_key(&config.workspace_root, &config.profile, pack).and_then(|key| {
        BOUND_RUNS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .cloned()
    });
    match bound {
        Some(BoundAdmission::Stages(run)) => {
            if let Some(failure) = run.seals_are_of_another_contract(&config.profile) {
                return Some(failure.to_string());
            }
            let shortfall = run.judge(StageScope::All)?;
            run.report_shortfall(&shortfall, config.eval_events_path.as_deref());
            Some(shortfall.to_refusal().to_string())
        }
        Some(BoundAdmission::NoStages) => None,
        None => no_session_failure(&config.profile),
    }
}

/// The table has nothing for this key: no session of this process admitted it, or the workspace's
/// path cannot be resolved to look. Only when memory says nothing of a declaration does the gate
/// read the pack here. What it finds is kept nowhere, so a later gate reads it again.
fn no_session_failure(profile: &str) -> Option<String> {
    let declared = selection::stages_declared_without_a_read(profile)
        || match selection::selected_stages_by_reading(profile) {
            Ok(declared) => declared,
            Err(_) => {
                return Some("stage_incomplete: the_selected_pack_cannot_be_read".to_string());
            }
        };
    declared.then(|| NO_SESSION_BOUND.to_string())
}

/// What one verdict says for one scope.
fn shortfall_of(
    verdict: &state::StageVerdict,
    scope: StageScope,
    touched: &BTreeSet<TargetKey>,
    revision_of: impl Fn(&TargetKey) -> Option<String>,
) -> Option<StageShortfall> {
    if !verdict.ledger_intact() {
        return Some(StageShortfall::of("ledger_not_intact"));
    }
    let unmet = verdict
        .obligations()
        .iter()
        .filter(|status| status.phase != state::Phase::Sealed)
        .filter(|status| scope == StageScope::All || touched.contains(&status.key))
        .map(|status| UnmetObligation {
            key: status.key.clone(),
            phase: status.phase,
            revision: revision_of(&status.key),
        })
        .collect::<Vec<_>>();
    let stands = match scope {
        StageScope::All => verdict.complete(),
        // Not a claim that the stages are complete: only that this step left none it
        // called unsealed. The set as a whole is judged by `complete` alone.
        StageScope::Touched => unmet.is_empty(),
    };
    (!stands).then(|| StageShortfall {
        code: "obligations_unmet",
        unmet,
        invalidated: verdict.invalidated().to_vec(),
        record: None,
    })
}

/// A plan with no final acceptance after it: a step answered only for what it called, so its end
/// answers for the rest.
pub(crate) fn refuse_unmet_plan(config: &Config) -> anyhow::Result<()> {
    crate::structured_budget::outcome::gate(config)?;
    final_gate_failure(config).map_or(Ok(()), |unmet| Err(anyhow::anyhow!("{unmet}")))
}

/// The last word on a session: however it ended, a success with obligations unmet is no success.
pub(crate) fn refuse_unmet_session<T>(
    result: anyhow::Result<T>,
    run: Option<&StageRun>,
    scope: StageScope,
    events: Option<&Path>,
) -> anyhow::Result<T> {
    let outcome = result?;
    let Some(run) = run else {
        return Ok(outcome);
    };
    let Some(shortfall) = run.judge(scope) else {
        return Ok(outcome);
    };
    run.report_shortfall(&shortfall, events);
    Err(shortfall.to_refusal().into())
}

impl StageRun {
    pub(crate) fn new(
        draft: StageContractDraft,
        members: BTreeMap<String, StageMember>,
        workspace: PathBuf,
        profile: String,
        ledgers: Arc<LedgerRegistry>,
    ) -> Self {
        Self {
            draft,
            members,
            workspace,
            profile,
            ledgers,
            session: Mutex::new(None),
            touched: Mutex::new(BTreeSet::new()),
            notes: Mutex::new(Vec::new()),
            calls_without_process: AtomicUsize::new(0),
            record_notice_shown: AtomicBool::new(false),
        }
    }

    /// Why this run's seals do not answer for the stages declared now, before its record is opened.
    /// With no session since, nothing else compares them: a seal is judged by its bytes alone.
    fn seals_are_of_another_contract(&self, profile: &str) -> Option<&'static str> {
        let admitted = StageIdentity {
            contract_sha256: self.draft.contract_sha256(),
            descriptors: self
                .members
                .iter()
                .map(|(tool, member)| {
                    let descriptor = member.pins.pins.descriptor_sha256.clone();
                    (tool.clone(), descriptor)
                })
                .collect(),
        };
        match declared_stage_identity(&self.workspace, profile) {
            Ok(Some(declared)) if declared == admitted => None,
            Ok(_) => Some(NO_SESSION_BOUND),
            Err(DeclaredStagesUnread::SelectedPack) => {
                Some("stage_incomplete: the_selected_pack_cannot_be_read")
            }
            Err(DeclaredStagesUnread::Stages) => {
                Some("stage_incomplete: the_declared_stages_cannot_be_read")
            }
        }
    }

    /// Whether completion stands for this scope, by the bytes there now. `None` means it does.
    /// The one place a verdict is read for completion, once per question: it writes what moved.
    /// A step that called no stage asks nothing, and opens nothing.
    pub(crate) fn judge(&self, scope: StageScope) -> Option<StageShortfall> {
        let touched = self
            .touched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if scope == StageScope::Touched && touched.is_empty() {
            return None;
        }
        let bytes = ledger::WorkspaceBytes::new(&self.workspace);
        let held = match self.open_ledger(crate::tools::profile_tools::effects::now_ms(), &bytes) {
            Ok((held, _)) => held,
            Err(refusal) => {
                return Some(StageShortfall {
                    record: self.with_ledger_path(refusal).data,
                    ..StageShortfall::of("ledger_refused")
                });
            }
        };
        let Ok(mut ledger) = held.lock() else {
            return Some(StageShortfall::of("ledger_refused"));
        };
        let verdict = ledger.verdict(&bytes);
        shortfall_of(&verdict, scope, &touched, |key| ledger.revision_of(key))
    }

    /// The event for a run's end, and what the operator needs when the record is the reason. A
    /// repair cycle asks again and again; the notice shows once.
    fn report_shortfall(&self, shortfall: &StageShortfall, events: Option<&Path>) {
        crate::eval_events::emit(events, shortfall.event());
        if let Some(record) = &shortfall.record
            && !self.record_notice_shown.swap(true, Ordering::SeqCst)
        {
            crate::tools::profile_tools::effects::show_stage_refusal(record);
        }
    }

    /// Records a call's effect. A call that started no process counts against a limit of its own
    /// and says whether that limit is reached. The limit is never below the set's size: one reply
    /// listing a whole chain gets `not_ready` from all but its top, and repeats nothing.
    pub(crate) fn note_call(&self, target: &TargetKey, started_process: bool) -> bool {
        self.touched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(target.clone());
        if started_process {
            self.calls_without_process.store(0, Ordering::SeqCst);
            return false;
        }
        self.note_no_progress(target);
        let limit = self
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map_or(0, |session| session.contract.targets().count())
            .max(MAX_CALLS_WITHOUT_PROCESS);
        self.calls_without_process.fetch_add(1, Ordering::SeqCst) + 1 >= limit
    }

    pub(crate) fn note_no_progress(&self, target: &TargetKey) {
        self.push_note(target, false);
    }

    pub(crate) fn note_progress(&self, target: &TargetKey) {
        self.push_note(target, true);
    }

    fn push_note(&self, target: &TargetKey, progress: bool) {
        self.notes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(StageNote {
                target: target.clone(),
                progress,
            });
    }

    pub(crate) fn take_notes(&self) -> Vec<StageNote> {
        std::mem::take(&mut *self.notes.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Whether this run may hold stages at all, with the values in force now. An entry can ask
    /// this before it spends a provider call. It keeps nothing.
    pub(crate) fn judge_run(
        &self,
        run: StageRunFacts,
        state_dir: &Path,
    ) -> Result<StageContract, ProfileToolRefusal> {
        let contract = self.draft.for_run(run).map_err(contract_refused)?;
        ledger::refuse_state_dir_inside_workspace(state_dir, &self.workspace)
            .map_err(ledger_refused)?;
        Ok(contract)
    }

    /// Where the operator finds the record, on a refusal about it.
    pub(crate) fn with_ledger_path(&self, mut refusal: ProfileToolRefusal) -> ProfileToolRefusal {
        let session = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        if let (Some(data), Some(session)) = (refusal.data.as_mut(), session.as_ref())
            && let Some(file) =
                ledger::ledger_file(&session.state_dir, &self.workspace, &self.profile)
        {
            data["ledger_path"] = serde_json::Value::String(file.display().to_string());
        }
        refusal
    }

    /// What only a session can answer, asked anew for each one.
    pub(crate) fn bind_session(
        &self,
        run: StageRunFacts,
        state_dir: &Path,
        entry: LedgerEntry,
    ) -> Result<(), ProfileToolRefusal> {
        let contract = self.judge_run(run, state_dir)?;
        *self.session.lock().unwrap_or_else(PoisonError::into_inner) = Some(StageSession {
            contract: Arc::new(contract),
            state_dir: state_dir.to_path_buf(),
            entry,
        });
        self.touched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        self.calls_without_process.store(0, Ordering::SeqCst);
        Ok(())
    }

    /// For the judgment final acceptance makes once sessions are over.
    pub(crate) fn remember_bound(self: &Arc<Self>) {
        remember(
            &self.workspace,
            &self.profile,
            selection::pack_selection_from_environment(),
            BoundAdmission::Stages(self.clone()),
        );
    }

    /// The ledger as of now, and the contract it is held to. Never called during an admission.
    pub(crate) fn open_ledger(
        &self,
        now_ms: u64,
        bytes: &dyn BytesSource,
    ) -> Result<(HeldLedger, Arc<StageContract>), ProfileToolRefusal> {
        let session = self
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| {
                ProfileToolRefusal::new(
                    RefusalKind::StageContractRefused,
                    "a stage tool was called outside a session that admitted its stages",
                )
            })?;
        let pins = self
            .members
            .iter()
            .map(|(tool, member)| (tool.clone(), member.pins.clone()))
            .collect();
        let held = self
            .ledgers
            .open(&LedgerRequest {
                state_dir: &session.state_dir,
                workspace: &self.workspace,
                profile: &self.profile,
                entry: session.entry,
                contract: session.contract.clone(),
                now_ms,
                bytes,
                pins: &pins,
            })
            .map_err(ledger_refused)?;
        Ok((held, session.contract))
    }

    /// The one place a call's values become what the state machine judges. Paths come from the
    /// admitted target and digests from what G2 adopted, so no spelling and no digest here is
    /// the caller's.
    pub(crate) fn attempt_request<'a>(
        &self,
        contract: &StageContract,
        target: &AdmittedTarget,
        call: StageCallFacts<'a>,
    ) -> Option<AttemptRequest<'a>> {
        let member = self.members.get(&target.tool_id)?;
        let digest_of = |adopted: &AdoptedInputs, path: &str| InputDigest {
            path: path.to_string(),
            sha256: adopted
                .inputs
                .iter()
                .find(|input| input.path == path)
                .map(|input| sha256_hex(&input.bytes))
                .unwrap_or_default(),
        };
        let mut adopted_fixed = Vec::new();
        for other in contract.targets() {
            let adopted = self.members.get(&other.tool_id)?.guard.adopted();
            let fixed = other
                .inputs
                .iter()
                .filter(|input| input.upstream.is_none())
                .map(|input| digest_of(&adopted, &input.path))
                .collect();
            adopted_fixed.push((other.key.clone(), fixed));
        }
        Some(AttemptRequest {
            turn: call.turn,
            now_ms: call.now_ms,
            expected_revision: call.expected_revision,
            pins: member.pins.pins.clone(),
            schema_sha256: member.pins.schema_sha256.clone(),
            input_revision: call.adopted.revision.clone(),
            inputs: target
                .inputs
                .iter()
                .map(|input| digest_of(call.adopted, &input.path))
                .collect(),
            arguments_sha256: call.arguments_sha256,
            request_id: call.request_id,
            current_output: call.current_output,
            adopted_fixed,
        })
    }

    /// A seal is what every reader of this output takes its new version from (S6).
    pub(crate) fn readopt_readers(
        &self,
        contract: &StageContract,
        writer: &AdmittedTarget,
        receipt: &CommitReceipt,
    ) -> Result<(), ProfileToolRefusal> {
        for reader in contract.targets() {
            let reads_it = reader
                .inputs
                .iter()
                .any(|input| input.upstream.as_ref() == Some(&writer.key));
            if reads_it && let Some(member) = self.members.get(&reader.tool_id) {
                member.guard.readopt(receipt)?;
            }
        }
        Ok(())
    }
}

pub(crate) mod contract;
pub(crate) mod invalidation;
pub(crate) mod ledger;
pub(crate) mod receipt;
pub(crate) mod resume;
pub(crate) mod state;

#[cfg(test)]
mod no_progress_limit_tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    use super::contract::{
        HostToolFacts, NarrowedStageLimits, StageContractDraft, StageDeclaration, StageLimits,
        StageRunFacts, StageSetDeclaration, TargetDeclaration, TargetKey,
    };
    use super::ledger::{LedgerEntry, LedgerRegistry};
    use super::{MAX_CALLS_WITHOUT_PROCESS, StageRun, StageScope, shortfall_of, state};

    /// `count` stages in a chain, none of them submitted, with no process behind any.
    fn chain(temp: &tempfile::TempDir, count: usize) -> StageRun {
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let output = |index: usize| format!("out/s{index}.json");
        let tools = (0..count)
            .map(|index| HostToolFacts {
                tool_id: format!("t{index}"),
                output_id: "artifact".to_string(),
                output_path: output(index),
                schema_inputs: index.checked_sub(1).map(output).into_iter().collect(),
                has_revision_argument: false,
                has_compiler: false,
                required_checks: vec!["shape".to_string()],
                limits: StageLimits {
                    attempts_per_target: 2,
                    attempts_per_run: 32,
                    deadline_seconds: 60,
                },
            })
            .collect::<Vec<_>>();
        let stages = (0..count)
            .map(|index| StageDeclaration {
                id: format!("s{index}"),
                targets: vec![TargetDeclaration {
                    id: "main".to_string(),
                    tool_id: format!("t{index}"),
                }],
                depends_on: Vec::new(),
                required_checks: Vec::new(),
            })
            .collect();
        let draft = StageContractDraft::new(
            &StageSetDeclaration {
                stages,
                limits: NarrowedStageLimits::default(),
            },
            &tools,
        )
        .unwrap();
        let run = StageRun::new(
            draft,
            BTreeMap::new(),
            workspace,
            "a-profile".to_string(),
            Arc::new(LedgerRegistry::default()),
        );
        run.bind_session(
            StageRunFacts::for_test(true, false),
            &root.join("state"),
            LedgerEntry::Fresh,
        )
        .unwrap();
        run
    }

    fn key(index: usize) -> TargetKey {
        TargetKey {
            stage: format!("s{index}"),
            target: "main".to_string(),
        }
    }

    /// N3: a reply that lists a whole chain runs the top and gets `not_ready` from each one below.
    /// That is one honest reply, and not a model repeating itself.
    #[test]
    fn a_chain_longer_than_the_limit_can_be_listed_in_one_reply() {
        let temp = tempfile::tempdir().unwrap();
        let run = chain(&temp, 9);
        for index in 1..9 {
            assert!(!run.note_call(&key(index), false), "call {index} of 8");
        }
        assert!(run.note_call(&key(1), false), "the ninth is past the chain");
    }

    #[test]
    fn a_set_smaller_than_the_limit_still_stops_at_the_limit() {
        let temp = tempfile::tempdir().unwrap();
        let run = chain(&temp, 2);
        for repeat in 1..MAX_CALLS_WITHOUT_PROCESS {
            assert!(!run.note_call(&key(0), false), "call {repeat}");
        }
        assert!(run.note_call(&key(0), false));
    }

    /// M11: the one way a step could call a record it cannot vouch for fine. `Touched` looks at
    /// the stages it called alone, so this is its last stand.
    #[test]
    fn a_verdict_with_no_record_behind_it_stands_for_no_scope() {
        let temp = tempfile::tempdir().unwrap();
        let contract = chain(&temp, 1)
            .judge_run(StageRunFacts::for_test(true, false), &temp.path().join("s"))
            .unwrap();
        let verdict = state::StageMachine::new(
            Arc::new(contract),
            "nonce".to_string(),
            "nonce".to_string(),
            1,
        )
        .verdict_without_ledger();
        assert!(!verdict.ledger_intact());
        for scope in [StageScope::All, StageScope::Touched] {
            let shortfall = shortfall_of(&verdict, scope, &BTreeSet::new(), |_| None).unwrap();
            assert_eq!(shortfall.code, "ledger_not_intact", "{scope:?}");
        }
    }
}

#[cfg(test)]
mod resume_entry_tests {
    use std::path::Path;

    use super::config_resuming;
    use super::ledger::LedgerEntry;

    #[test]
    fn a_plan_run_by_resume_says_so_and_a_fresh_session_still_wins() {
        let mut config =
            crate::tools::profile_tools::effects::config_for_test(Path::new("/nonexistent"));
        assert_eq!(LedgerEntry::of_config(&config), LedgerEntry::Fresh);
        let resuming = config_resuming(&config, "018f3333");
        assert_eq!(resuming.resume.as_deref(), Some("018f3333"));
        assert_eq!(LedgerEntry::of_config(&resuming), LedgerEntry::Resume);
        // Only this one field differs, and the original is left as it was.
        assert_eq!(config.resume, None);
        let mut same = resuming;
        same.resume = None;
        assert_eq!(format!("{same:?}"), format!("{config:?}"));

        config.fresh_session = true;
        assert_eq!(
            LedgerEntry::of_config(&config_resuming(&config, "018f3333")),
            LedgerEntry::FreshSession
        );
    }
}

#[cfg(test)]
mod test_support {
    //! Synthetic stage sets and a fake byte source. The IDs and paths mean nothing to the host.

    use std::collections::BTreeMap;
    use std::sync::Arc;

    use sha2::{Digest, Sha256};

    use super::contract::{
        ContractRefusal, HostToolFacts, NarrowedStageLimits, StageContract, StageDeclaration,
        StageLimits, StageRunFacts, StageSetDeclaration, TargetDeclaration, TargetKey, admit,
    };
    use super::receipt::{CheckResult, CheckStatus, InputDigest, ReceiptParts, ToolPins};
    use super::state::{
        AttemptRequest, BeginDecision, BytesSource, CommitDecision, CommitObservation,
        LocalDecision, ObservedBytes, StageMachine,
    };
    use crate::tools::profile_tools::commit::CommitReceipt;
    use crate::tools::profile_tools::protocol::RECEIPT_PROTOCOL;

    /// The admission, in a run an OS boundary holds and no workflow owns.
    pub(crate) fn admit_enforced(
        declaration: &StageSetDeclaration,
        tools: &[HostToolFacts],
    ) -> Result<StageContract, ContractRefusal> {
        admit(StageRunFacts::for_test(true, false), declaration, tools)
    }

    pub(crate) fn digest(text: &str) -> String {
        sha_of(text.as_bytes())
    }

    pub(crate) fn sha_of(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    pub(crate) fn key(stage: &str, target: &str) -> TargetKey {
        TargetKey {
            stage: stage.to_string(),
            target: target.to_string(),
        }
    }

    pub(crate) fn host_limits() -> StageLimits {
        StageLimits {
            attempts_per_target: 8,
            attempts_per_run: 64,
            deadline_seconds: 3600,
        }
    }

    pub(crate) fn facts(tool: &str, output: &str, inputs: &[&str]) -> HostToolFacts {
        HostToolFacts {
            tool_id: tool.to_string(),
            output_id: "artifact".to_string(),
            output_path: output.to_string(),
            schema_inputs: inputs.iter().map(|path| path.to_string()).collect(),
            has_revision_argument: false,
            has_compiler: false,
            required_checks: vec!["shape".to_string(), "content".to_string()],
            limits: host_limits(),
        }
    }

    /// A stage with one target, named after its tool.
    pub(crate) fn one_target(stage: &str, tool: &str) -> StageDeclaration {
        StageDeclaration {
            id: stage.to_string(),
            targets: vec![TargetDeclaration {
                id: tool.to_string(),
                tool_id: tool.to_string(),
            }],
            depends_on: Vec::new(),
            required_checks: Vec::new(),
        }
    }

    fn chained(
        first: (&str, &str, &str),
        second: (&str, &str, &str),
        fixed: &str,
    ) -> StageContract {
        let target = |id: &str, tool: &str| TargetDeclaration {
            id: id.to_string(),
            tool_id: tool.to_string(),
        };
        let stage = |id: &str, declared: TargetDeclaration| StageDeclaration {
            id: id.to_string(),
            targets: vec![declared],
            depends_on: Vec::new(),
            required_checks: Vec::new(),
        };
        let first_tool = format!("{}-tool", first.0);
        let second_tool = format!("{}-tool", second.0);
        // The downstream comes first in the file: the order comes from the paths.
        let declaration = StageSetDeclaration {
            stages: vec![
                stage(second.0, target(second.1, &second_tool)),
                stage(first.0, target(first.1, &first_tool)),
            ],
            limits: NarrowedStageLimits::default(),
        };
        let tools = [
            facts(&first_tool, first.2, &[fixed]),
            facts(&second_tool, second.2, &[first.2]),
        ];
        admit_enforced(&declaration, &tools).unwrap()
    }

    pub(crate) fn two_stage_contract() -> StageContract {
        chained(
            ("alpha", "main", "out/alpha.json"),
            ("beta", "main", "out/beta.json"),
            "fixed/source.json",
        )
    }

    pub(crate) fn setting_object_contract() -> StageContract {
        chained(
            ("settings", "object", "config/settings.json"),
            ("settings_index", "lookup", "config/settings-index.json"),
            "seed/settings-schema.json",
        )
    }

    pub(crate) fn document_index_contract() -> StageContract {
        chained(
            ("catalog", "documents", "docs/catalog.json"),
            ("summary", "digest", "docs/summary.json"),
            "seed/document-list.json",
        )
    }

    #[derive(Debug, Clone, Default)]
    pub(crate) struct FakeBytes {
        files: BTreeMap<String, ObservedBytes>,
    }

    impl FakeBytes {
        pub(crate) fn put(&mut self, path: &str, bytes: &[u8]) {
            self.files
                .insert(path.to_string(), ObservedBytes::Present(bytes.to_vec()));
        }

        pub(crate) fn with(&self, path: &str, bytes: &[u8]) -> Self {
            let mut copy = self.clone();
            copy.put(path, bytes);
            copy
        }

        pub(crate) fn without(&self, path: &str) -> Self {
            let mut copy = self.clone();
            copy.files.remove(path);
            copy
        }

        pub(crate) fn unreadable(&self, path: &str) -> Self {
            let mut copy = self.clone();
            copy.files
                .insert(path.to_string(), ObservedBytes::Unreadable);
            copy
        }

        pub(crate) fn observe_path(&self, path: &str) -> ObservedBytes {
            self.observe(path)
        }
    }

    impl BytesSource for FakeBytes {
        fn observe(&self, path: &str) -> ObservedBytes {
            self.files
                .get(path)
                .cloned()
                .unwrap_or(ObservedBytes::Absent)
        }
    }

    pub(crate) fn pins_for(tool: &str) -> ToolPins {
        ToolPins {
            tool_id: tool.to_string(),
            tool_version: "1.0.0".to_string(),
            descriptor_sha256: digest(&format!("descriptor {tool}")),
            runtime_sha256: digest(&format!("runtime {tool}")),
        }
    }

    pub(crate) fn schema_for(tool: &str) -> String {
        digest(&format!("schema {tool}"))
    }

    pub(crate) fn current_inputs(
        contract: &StageContract,
        target: &TargetKey,
        bytes: &FakeBytes,
    ) -> Vec<InputDigest> {
        contract
            .target(target)
            .map(|admitted| {
                admitted
                    .inputs
                    .iter()
                    .map(|input| InputDigest {
                        path: input.path.clone(),
                        sha256: bytes.observe(&input.path).sha256().unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// One call, its values taken from the operator's pins and the bytes there now.
    #[allow(
        clippy::too_many_arguments,
        reason = "a test helper that spells out each value a call varies"
    )]
    pub(crate) fn request_for<'a>(
        contract: &Arc<StageContract>,
        target: &TargetKey,
        expected_revision: &'a str,
        bytes: &FakeBytes,
        turn: u64,
        now_ms: u64,
        arguments: &str,
        current_output: &'a ObservedBytes,
    ) -> AttemptRequest<'a> {
        let tool = contract
            .target(target)
            .map_or("", |admitted| admitted.tool_id.as_str());
        let inputs = current_inputs(contract, target, bytes);
        let adopted_fixed = contract
            .targets()
            .map(|admitted| {
                let fixed = admitted
                    .inputs
                    .iter()
                    .filter(|input| input.upstream.is_none())
                    .map(|input| InputDigest {
                        path: input.path.clone(),
                        sha256: bytes.observe(&input.path).sha256().unwrap_or_default(),
                    })
                    .collect();
                (admitted.key.clone(), fixed)
            })
            .collect();
        AttemptRequest {
            turn,
            now_ms,
            expected_revision,
            pins: pins_for(tool),
            schema_sha256: schema_for(tool),
            input_revision: digest(&serde_json::to_string(&inputs).unwrap()),
            inputs,
            arguments_sha256: digest(arguments),
            request_id: "request-1".to_string(),
            current_output,
            adopted_fixed,
        }
    }

    pub(crate) fn passed_checks(contract: &StageContract, target: &TargetKey) -> Vec<CheckResult> {
        contract
            .target(target)
            .map(|admitted| {
                admitted
                    .required_checks
                    .iter()
                    .map(|id| CheckResult {
                        id: id.clone(),
                        status: CheckStatus::Passed,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn commit_for(
        contract: &StageContract,
        target: &TargetKey,
        pins: &ToolPins,
        schema: &str,
        input_revision: &str,
        candidate: &str,
    ) -> CommitReceipt {
        CommitReceipt {
            protocol: RECEIPT_PROTOCOL.to_string(),
            request_id: "request-1".to_string(),
            tool_id: pins.tool_id.clone(),
            output_id: "artifact".to_string(),
            path: contract
                .target(target)
                .map(|admitted| admitted.output_path.clone())
                .unwrap_or_default(),
            committed: true,
            durable: true,
            cancelled_after_commit: false,
            runtime_sha256: pins.runtime_sha256.clone(),
            input_revision: input_revision.to_string(),
            schema_sha256: schema.to_string(),
            result_sha256: digest("result"),
            committed_sha256: candidate.to_string(),
            committed_bytes: 1,
            diagnostics: Vec::new(),
        }
    }

    /// The whole path of one good submission on the machine, the file written where a rename
    /// would be. Panics on anything but a seal.
    #[allow(
        clippy::too_many_arguments,
        reason = "a test helper that spells out each value a call varies"
    )]
    pub(crate) fn seal(
        machine: &mut StageMachine,
        target: &TargetKey,
        content: &[u8],
        bytes: &mut FakeBytes,
        turn: u64,
        now_ms: u64,
        arguments: &str,
    ) -> CommitDecision {
        let contract = machine.contract().clone();
        let admitted = contract.target(target).unwrap().clone();
        let revision = machine.revision_of(target).unwrap();
        let output = bytes.observe(&admitted.output_path);
        let request = request_for(
            &contract, target, &revision, bytes, turn, now_ms, arguments, &output,
        );
        let (pins, schema, input_revision) = (
            request.pins.clone(),
            request.schema_sha256.clone(),
            request.input_revision.clone(),
        );
        let BeginDecision::Proceed(ticket) = machine.begin(target, request) else {
            panic!("the attempt on {target:?} did not start");
        };
        let candidate = sha_of(content);
        assert_eq!(
            machine.record_local_result(
                &ticket,
                &candidate,
                passed_checks(&contract, target),
                &output,
            ),
            LocalDecision::ReadyToCommit
        );
        machine.record_intent(&ticket, output.sha256()).unwrap();
        bytes.put(&admitted.output_path, content);
        let commit = commit_for(
            &contract,
            target,
            &pins,
            &schema,
            &input_revision,
            &candidate,
        );
        let decision = machine
            .record_commit(&ticket, CommitObservation::Stood(Box::new(commit)))
            .unwrap();
        assert!(decision.is_progress(), "{decision:?}");
        decision
    }

    pub(crate) fn commit_receipt(candidate: &str) -> CommitReceipt {
        CommitReceipt {
            protocol: RECEIPT_PROTOCOL.to_string(),
            request_id: "request-1".to_string(),
            tool_id: "tool-1".to_string(),
            output_id: "artifact".to_string(),
            path: "out/a.json".to_string(),
            committed: true,
            durable: true,
            cancelled_after_commit: false,
            runtime_sha256: digest("runtime"),
            input_revision: digest("inputs"),
            schema_sha256: digest("schema"),
            result_sha256: digest("result"),
            committed_sha256: candidate.to_string(),
            committed_bytes: 9,
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn parts(candidate: &str) -> ReceiptParts {
        ReceiptParts {
            run_nonce: "run-a".to_string(),
            process_nonce: "process-a".to_string(),
            stage: "alpha".to_string(),
            target: "main".to_string(),
            output_path: "out/a.json".to_string(),
            attempt: 1,
            generation: 1,
            pins: ToolPins {
                tool_id: "tool-1".to_string(),
                tool_version: "1.0.0".to_string(),
                descriptor_sha256: digest("descriptor"),
                runtime_sha256: digest("runtime"),
            },
            schema_sha256: digest("schema"),
            input_revision: digest("inputs"),
            inputs: Vec::new(),
            dependencies: Vec::new(),
            arguments_sha256: digest("arguments"),
            request_id: "request-1".to_string(),
            candidate_sha256: candidate.to_string(),
            checks: vec![CheckResult {
                id: "shape".to_string(),
                status: CheckStatus::Passed,
            }],
        }
    }
}
