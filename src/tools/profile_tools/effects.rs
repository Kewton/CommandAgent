//! The `propose-artifacts` effect (Issue #974, G3): a pinned process proposes bytes for one
//! declared output, and only the host, after checking them, adopts them.
//!
//! A proposal tool is not an extension: it does not pass through the Read branch of
//! `ExtensionTool`, and it records its own events instead of `access=read_only`. Its
//! authority is the Write target of the existing allow policy, judged with the values in
//! force before the start, right before the process, and right before the commit.
//!
//! What was admitted is one immutable value. Nothing a model sends or a process prints can
//! change an executable, an argument vector, or a destination.

use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::admission::{ToolExecutionReceipt, ToolUsage};
use super::commit::{self, CommitPlan, CommitReceipt};
use super::descriptor::{self, Descriptor, ProcessPins, RegistryBinding};
use super::isolation::{self, StrictLaunch};
use super::process::{self, RunLimits};
use super::protocol::{self, Binding, BoundedDiagnostics, Diagnostic, ProcessDeclaration};
use super::schema::SchemaInput;
use super::schema::binding::AdoptedInputs;
use super::schema::invocation::SchemaGuard;
use super::secret_output::SecretForms;
use super::selection::DeclaredTool;
use super::{ProfileToolRefusal, RefusalKind};
use crate::config::{Action, Config, OpenAiCompatibleConfig};
use crate::eval_events;
use crate::mode::ExecutionMode;
use crate::planner::structured_stages::contract::{AdmittedTarget, StageContract};
use crate::planner::structured_stages::ledger::{
    self as stage_ledger, HeldLedger, LedgerRefusal, StageLedger, WorkspaceBytes,
};
use crate::planner::structured_stages::receipt::{ToolPins, sha256_hex};
use crate::planner::structured_stages::resume::AdmittedPins;
use crate::planner::structured_stages::state::{
    AttemptFailure, AttemptTicket, BeginDecision, BytesSource, CheckFailureKind, CommitDecision,
    CommitObservation, ExhaustedLimit, FailureDecision, InvalidatedTarget, LocalDecision,
    ObservedBytes, TerminalFailure, judge_checks,
};
use crate::planner::structured_stages::{
    StageCallFacts, StageMember, StageNote, StageRun, ledger_refused,
};
use crate::tools::registry::{FunctionSpec, ToolContext, ToolSpec};
use crate::tools::workspace_policy::WorkspacePolicy;

pub const EFFECT_PROPOSE_ARTIFACTS: &str = "propose-artifacts";
pub const RUNTIME_PINNED_PROCESS: &str = "pinned_process";
const TRANSPORT: &str = "profile_tool";
const STAGE_PROCESS: &str = "before_process";
const STAGE_COMMIT: &str = "before_commit";
/// The model's idea of a stage tool's revision. The operator's schema declares it: required and
/// exactly `type: string`, so nothing the host publishes changes and the schema's own pin and
/// hash cover it. It is compared and nothing else. It is neither part of what a call asks for nor
/// shown to the process.
pub(crate) const EXPECTED_REVISION_ARGUMENT: &str = "expected_revision";

/// What is known at the start of a session about the session itself. `ToolContext.mode` is
/// always `Act` in a session, so an inspect step is told apart here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionAccess {
    Normal,
    InspectStep,
    ScopedRecoveryReads,
}

fn refusal(kind: RefusalKind, reason: impl Into<String>) -> ProfileToolRefusal {
    ProfileToolRefusal::new(kind, reason)
}

/// The existing rules for a workspace path that names a credential, a hidden or engine
/// directory, or a frozen verification input, with no other path reading of its own.
fn judge_path(
    root: &Path,
    relative: &str,
    protected: &[String],
    policy: WorkspacePolicy,
    tool: &str,
    role: &str,
) -> Result<(), ProfileToolRefusal> {
    let invalid = |reason: &str| {
        refusal(
            RefusalKind::EffectPathRefused,
            format!("tool `{tool}` {role} {reason}"),
        )
    };
    crate::tools::path_guard::validate_workspace_relative(relative)
        .map_err(|_| invalid("is not a workspace-relative path"))?;
    crate::tools::hidden_path::ensure_reference_allowed(root, relative, policy)
        .map_err(|_| invalid("is a hidden path"))?;
    let resolved = root.join(relative);
    crate::tools::sensitive_path::ensure_not_sensitive_path(root, &resolved)
        .map_err(|_| invalid("is a credential path"))?;
    crate::tools::workspace_policy::ensure_tool_path_allowed(root, &resolved, policy)
        .map_err(|_| invalid("is not allowed by the workspace policy"))?;
    if crate::minimal_loop::protected_paths::matching_path(root, relative, protected).is_some() {
        return Err(invalid("is a protected path"));
    }
    Ok(())
}

/// The Write target of the allow policy, with the auto-approval the caller holds. This is
/// the only authority this effect takes, and an explicit read-only policy stops it whatever
/// `yes` says.
fn authorize(root: &Path, auto_approve: bool, interactive: bool) -> Result<(), ProfileToolRefusal> {
    crate::tools::allow_policy::authorize_current(
        "Write",
        &Value::Null,
        root,
        auto_approve,
        interactive,
    )
    .map_err(|_| {
        refusal(
            RefusalKind::EffectReadOnly,
            "this run cannot write under its approval or its allow policy",
        )
    })
}

/// The checks that need nothing but the descriptor and the operating system, cheapest
/// first. An input is judged before it is read, because the G2 read is not a credential
/// check and is left as it is for a `read` tool. Returns the pins to compare later.
pub(super) fn admit_process<'a>(
    descriptor: &'a Descriptor,
    declared: &DeclaredTool,
    workspace: &Path,
) -> Result<(&'a ProcessDeclaration, ProcessPins), ProfileToolRefusal> {
    let tool = descriptor.tool_id.as_str();
    if declared.effect != descriptor.effect {
        return Err(refusal(
            RefusalKind::ConflictingDeclaration,
            format!(
                "tool `{tool}` is declared with effect `{}` but registered with `{}`",
                declared.effect, descriptor.effect
            ),
        ));
    }
    if let Some(feature) = descriptor.requires.iter().chain(&declared.requires).next() {
        return Err(refusal(
            RefusalKind::EffectUnavailable,
            format!("tool `{tool}` requires `{feature}`, which is not implemented"),
        ));
    }
    let declaration = descriptor.process.as_ref().ok_or_else(|| {
        refusal(
            RefusalKind::InvalidDescriptor,
            format!("tool `{tool}` runtime `{RUNTIME_PINNED_PROCESS}` needs a `process`"),
        )
    })?;
    declaration.validate(tool)?;
    let schema = descriptor.schema.as_ref().ok_or_else(|| {
        refusal(
            RefusalKind::InvalidDescriptor,
            format!("tool `{tool}` effect `{EFFECT_PROPOSE_ARTIFACTS}` needs a `schema`"),
        )
    })?;
    let policy = WorkspacePolicy::for_task_request();
    let output = declaration.single_output();
    judge_path(workspace, &output.path, &[], policy, tool, "output")?;
    let inputs = super::schema::binding::declared_inputs(schema, tool)?;
    for input in &inputs {
        judge_path(workspace, input, &[], policy, tool, "input")?;
        if *input == output.path {
            return Err(refusal(
                RefusalKind::EffectPathRefused,
                format!("tool `{tool}` output is one of its own inputs"),
            ));
        }
    }
    check_input_files(workspace, &inputs, tool)?;
    let pins = descriptor::verify_process_pins(declaration, tool, workspace)?;
    isolation::probe_strict()?;
    Ok((declaration, pins))
}

/// A refusal from a proposal tool's admission, with that origin kept.
pub(super) fn with_effect_origin(error: ProfileToolRefusal) -> ProfileToolRefusal {
    refusal(error.kind.with_effect_origin(), error.reason)
}

/// An input reaches the process's stdin, so one regular file with one name is required: a
/// hard link would carry a credential's bytes under an allowed name. A file that cannot be
/// opened is left to the G2 read, which refuses it. A `read` tool's inputs are not judged here.
#[cfg(unix)]
fn check_input_files(
    workspace: &Path,
    inputs: &[String],
    tool: &str,
) -> Result<(), ProfileToolRefusal> {
    use std::os::unix::fs::MetadataExt as _;
    for input in inputs {
        let Ok(file) = crate::tools::dir_fd::open_read_file(workspace, Path::new(input)) else {
            continue;
        };
        let holds = file
            .metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.nlink() == 1);
        if !holds {
            return Err(refusal(
                RefusalKind::EffectPathRefused,
                format!("tool `{tool}` input is not one regular file with one name"),
            ));
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_input_files(
    _workspace: &Path,
    _inputs: &[String],
    tool: &str,
) -> Result<(), ProfileToolRefusal> {
    Err(refusal(
        RefusalKind::EffectUnavailable,
        format!("tool `{tool}` inputs cannot be inspected on this platform"),
    ))
}

/// One admitted proposal tool.
pub(crate) struct ProposalTool {
    spec: ToolSpec,
    tool_id: String,
    tool_version: String,
    descriptor_sha256: String,
    runtime_sha256: String,
    declaration: ProcessDeclaration,
    registry: RegistryBinding,
    pins: ProcessPins,
    workspace: PathBuf,
    /// Held since admission, so a root replaced later redirects nothing.
    root: File,
    guard: Arc<SchemaGuard>,
    max_input_bytes: usize,
    max_result_bytes: usize,
    receipts: Arc<Mutex<Vec<ToolExecutionReceipt>>>,
    commits: Arc<Mutex<Vec<CommitReceipt>>>,
    pending_paths: Mutex<Vec<String>>,
    /// The run's compatible provider, once an entry has judged this tool. Its key is a secret
    /// of this run under whatever name the configuration chose.
    compatible: OnceLock<OpenAiCompatibleConfig>,
    /// Set only for a tool a declared stage names. Everything else keeps its one submission.
    stage: OnceLock<Arc<StageRun>>,
}

impl fmt::Debug for ProposalTool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProposalTool")
            .field("tool_id", &self.tool_id)
            .field("runtime_sha256", &self.runtime_sha256)
            .finish_non_exhaustive()
    }
}

/// What admission hands over for one tool.
pub(super) struct ProposalParts {
    pub(super) name: String,
    pub(super) description: String,
    pub(super) tool_id: String,
    pub(super) tool_version: String,
    pub(super) descriptor_sha256: String,
    pub(super) declaration: ProcessDeclaration,
    pub(super) registry: RegistryBinding,
    pub(super) pins: ProcessPins,
    pub(super) workspace: PathBuf,
    pub(super) guard: Arc<SchemaGuard>,
    pub(super) max_input_bytes: usize,
    pub(super) max_output_bytes: usize,
    pub(super) receipts: Arc<Mutex<Vec<ToolExecutionReceipt>>>,
    pub(super) commits: Arc<Mutex<Vec<CommitReceipt>>>,
}

impl ProposalTool {
    pub(super) fn new(parts: ProposalParts) -> Result<Self, ProfileToolRefusal> {
        let root = File::open(&parts.workspace).map_err(|_| {
            refusal(
                RefusalKind::RegistryUnavailable,
                "the workspace cannot be opened",
            )
        })?;
        crate::tools::extension::validate_tool_name(&parts.name)
            .map_err(|error| refusal(RefusalKind::InvalidDescriptor, error.to_string()))?;
        if parts.description.trim().is_empty() {
            return Err(refusal(
                RefusalKind::InvalidDescriptor,
                format!("tool `{}` requires a description", parts.name),
            ));
        }
        let max_result_bytes = parts
            .max_output_bytes
            .min(parts.declaration.limits.max_result_bytes);
        Ok(Self {
            spec: ToolSpec {
                kind: "function".to_string(),
                function: FunctionSpec {
                    name: parts.name,
                    description: parts.description,
                    parameters: parts.guard.published_schema().clone(),
                },
            },
            tool_id: parts.tool_id,
            tool_version: parts.tool_version,
            descriptor_sha256: parts.descriptor_sha256,
            runtime_sha256: parts.declaration.runtime_sha256(),
            declaration: parts.declaration,
            registry: parts.registry,
            pins: parts.pins,
            workspace: parts.workspace,
            root,
            guard: parts.guard,
            max_input_bytes: parts.max_input_bytes,
            max_result_bytes,
            receipts: parts.receipts,
            commits: parts.commits,
            pending_paths: Mutex::new(Vec::new()),
            compatible: OnceLock::new(),
            stage: OnceLock::new(),
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.spec.function.name
    }

    pub(super) fn tool_id(&self) -> &str {
        &self.tool_id
    }

    pub(crate) fn has_stage(&self) -> bool {
        self.stage.get().is_some()
    }

    pub(crate) fn take_stage_notes(&self) -> Vec<StageNote> {
        self.stage
            .get()
            .map_or_else(Vec::new, |stage| stage.take_notes())
    }

    /// The contract and this tool come from two reads of the descriptor. A file replaced between
    /// them must not leave a target judged on paths its tool does not use.
    pub(super) fn matches_target(&self, target: &AdmittedTarget) -> Result<(), ProfileToolRefusal> {
        let output = self.declaration.single_output();
        let inputs = target.inputs.iter().map(|input| &input.path);
        if output.path != target.output_path
            || output.id != target.output_id
            || !inputs.eq(self.guard.declared_inputs().iter())
        {
            return Err(refusal(
                RefusalKind::StageContractRefused,
                format!(
                    "tool `{}` changed while its stages were being admitted",
                    self.tool_id
                ),
            ));
        }
        Ok(())
    }

    /// What a stage set holds of this tool. A tool whose schema does not take the revision
    /// argument cannot be one: a model would have no way to answer a `stale`.
    pub(super) fn stage_member(&self) -> Result<StageMember, ProfileToolRefusal> {
        if !declares_expected_revision(self.guard.published_schema()) {
            return Err(refusal(
                RefusalKind::StageContractRefused,
                format!(
                    "tool `{}` must declare `{EXPECTED_REVISION_ARGUMENT}`: required, and exactly `type: string`",
                    self.tool_id
                ),
            ));
        }
        Ok(StageMember {
            pins: AdmittedPins {
                pins: ToolPins {
                    tool_id: self.tool_id.clone(),
                    tool_version: self.tool_version.clone(),
                    descriptor_sha256: self.descriptor_sha256.clone(),
                    runtime_sha256: self.runtime_sha256.clone(),
                },
                schema_sha256: self.guard.adopted_schema_sha256().to_string(),
            },
            guard: self.guard.clone(),
        })
    }

    pub(super) fn attach_stage(&self, stage: Arc<StageRun>) {
        let _ = self.stage.set(stage);
    }

    pub(crate) fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    pub(super) fn runtime_sha256(&self) -> &str {
        &self.runtime_sha256
    }

    /// Paths committed since the last call, from the receipts this tool made.
    pub(crate) fn take_committed_paths(&self) -> Vec<String> {
        std::mem::take(
            &mut self
                .pending_paths
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    fn input_paths(&self) -> Vec<String> {
        self.guard.declared_inputs().to_vec()
    }

    /// The context of one entry, before its first provider call. Off for every set
    /// without a proposal tool, because this is only reached through one.
    pub(super) fn preflight(&self, config: &Config) -> Result<(), ProfileToolRefusal> {
        let tool = self.name();
        if let Some(compatible) = &config.openai_compatible {
            let _ = self.compatible.set(compatible.clone());
        }
        if matches!(config.action, Action::PlanSteps(_) | Action::UltraPlan(_)) {
            return Err(refusal(
                RefusalKind::EffectReadOnly,
                format!("tool `{tool}` cannot run under an action that only plans"),
            ));
        }
        authorize(&config.workspace_root, config.yes, false)?;
        let protected =
            crate::minimal_loop::completion::CompletionContract::load_for_config(config)
                .map_err(|_| {
                    refusal(
                        RefusalKind::EffectPathRefused,
                        format!("tool `{tool}` cannot read the protected paths of this run"),
                    )
                })?
                .map(|contract| contract.protected_paths)
                .unwrap_or_default();
        self.judge_all_paths(&protected, WorkspacePolicy::for_task_request())?;
        let mut forbidden = self.input_paths();
        forbidden.extend(protected);
        commit::preflight_destination(
            &self.root,
            &self.workspace,
            &self.declaration.single_output().path,
            &forbidden,
        )
        .map_err(|error| refusal(error.kind, format!("tool `{tool}`: {}", error.reason)))?;
        self.preflight_secrets(config)
    }

    /// A value the secret scrub would change is refused with the value not repeated: the
    /// process would receive a secret, and the host adds no looser rule of its own.
    fn preflight_secrets(&self, config: &Config) -> Result<(), ProfileToolRefusal> {
        let tool = self.name();
        let scope = crate::config::install_run_secret_scope(config).map_err(|_| {
            refusal(
                RefusalKind::EffectEnvRefused,
                format!("tool `{tool}`: the secret scope cannot be installed"),
            )
        })?;
        let changes = |text: &str| scope.scrub_text(text) != text;
        let env = self
            .declaration
            .env
            .iter()
            .flat_map(|(key, value)| [key.as_str(), value.as_str()]);
        if env.into_iter().any(changes) {
            return Err(refusal(
                RefusalKind::EffectEnvRefused,
                format!("tool `{tool}` declares an env value the secret scrub would change"),
            ));
        }
        if self.declaration.argv.iter().any(|arg| changes(arg)) {
            return Err(refusal(
                RefusalKind::EffectEnvRefused,
                format!("tool `{tool}` declares an argument the secret scrub would change"),
            ));
        }
        Ok(())
    }

    /// What only a session knows.
    pub(super) fn preflight_session(
        &self,
        access: SessionAccess,
    ) -> Result<(), ProfileToolRefusal> {
        let context = match access {
            SessionAccess::Normal => return Ok(()),
            SessionAccess::InspectStep => "an inspect step",
            SessionAccess::ScopedRecoveryReads => "a recovery inspection with scoped reads",
        };
        Err(refusal(
            RefusalKind::EffectReadOnly,
            format!("tool `{}` cannot run in {context}", self.name()),
        ))
    }

    fn judge_all_paths(
        &self,
        protected: &[String],
        policy: WorkspacePolicy,
    ) -> Result<(), ProfileToolRefusal> {
        let tool = self.name();
        let output = self.declaration.single_output();
        judge_path(
            &self.workspace,
            &output.path,
            protected,
            policy,
            tool,
            "output",
        )?;
        let inputs = self.input_paths();
        for input in &inputs {
            judge_path(&self.workspace, input, protected, policy, tool, "input")?;
        }
        check_input_files(&self.workspace, &inputs, tool)
    }

    /// Every authority, with the values in force now. Run before the process and again
    /// right before the commit: the start's answer is not trusted.
    fn judge_now(&self, context: &ToolContext, stage: &str) -> Result<(), ProfileToolRefusal> {
        let tool = self.name();
        if context.mode == ExecutionMode::Plan {
            return Err(refusal(
                RefusalKind::EffectReadOnly,
                format!("stage={stage} tool `{tool}` cannot run in plan mode"),
            ));
        }
        authorize(
            &context.root,
            context.auto_approve,
            context.interactive_approval,
        )?;
        let same_root =
            std::fs::canonicalize(&context.root).is_ok_and(|root| root == self.workspace);
        if !same_root {
            return Err(refusal(
                RefusalKind::EffectPathRefused,
                format!("stage={stage} tool `{tool}` was admitted for another workspace"),
            ));
        }
        self.judge_all_paths(&context.protected_paths, context.workspace_policy)
    }

    /// The operator's pins and the adopted schema, read again now.
    fn verify_pins(&self, stage: &str) -> Result<(), ProfileToolRefusal> {
        let tool = self.tool_id.as_str();
        descriptor::verify_descriptor_pin_held(&self.registry, &self.descriptor_sha256)?;
        if descriptor::verify_process_pins(&self.declaration, tool, &self.workspace)? != self.pins {
            return Err(refusal(
                RefusalKind::InvalidPin,
                format!("stage={stage} tool `{tool}` runtime directories changed since admission"),
            ));
        }
        self.guard.verify_adopted(stage)
    }

    fn record(&self, executed: bool, ok: bool) {
        self.receipts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(ToolExecutionReceipt {
                tool_id: self.tool_id.clone(),
                executed,
                ok,
                usage: None::<ToolUsage>,
            });
    }

    /// The whole of one call. The registry reaches this from both of its public functions,
    /// with the same guard and the same cancel.
    pub(crate) fn execute(
        &self,
        arguments: &Value,
        context: &ToolContext,
        is_cancelled: &dyn Fn() -> bool,
    ) -> anyhow::Result<String> {
        emit_call(context, self, arguments);
        let mut notes = RunNotes::default();
        match self.run(arguments, context, is_cancelled, &mut notes) {
            Ok(text) => {
                match notes.stage_reply_ok {
                    Some(ok) => self.record(notes.executed, ok),
                    None => self.record(true, true),
                }
                Ok(text)
            }
            Err(error) => {
                self.record(notes.executed, false);
                emit_rejection(context, self, &error, &notes);
                if let Some(data) = &error.data {
                    emit_stage_refusal(context, self, data);
                }
                if error.kind == RefusalKind::SchemaViolation {
                    // A model can correct an argument, so this goes back to it as data.
                    anyhow::bail!("extension_tool_rejected: `{}`: {error}", self.name());
                }
                Err(error.into())
            }
        }
    }

    fn run(
        &self,
        arguments: &Value,
        context: &ToolContext,
        is_cancelled: &dyn Fn() -> bool,
        notes: &mut RunNotes,
    ) -> Result<String, ProfileToolRefusal> {
        self.judge_now(context, STAGE_PROCESS)?;
        self.check_arguments(arguments)?;
        self.verify_pins(STAGE_PROCESS)?;
        let secrets = self.secret_forms(context)?;
        if is_cancelled() {
            return Err(refusal(
                RefusalKind::ProcessFailed,
                "cancelled before the process started",
            ));
        }
        let adopted = self.guard.adopted();
        let binding = Binding {
            request_id: uuid::Uuid::now_v7().to_string(),
            tool_id: self.tool_id.clone(),
            tool_version: self.tool_version.clone(),
            descriptor_sha256: self.descriptor_sha256.clone(),
            runtime_sha256: self.runtime_sha256.clone(),
            input_revision: adopted.revision.clone(),
            schema_sha256: self.guard.adopted_schema_sha256().to_string(),
        };
        if let Some(stage) = self.stage.get() {
            return self.run_stage(
                &StageCall {
                    stage,
                    arguments,
                    context,
                    is_cancelled,
                    secrets: &secrets,
                    binding: &binding,
                    adopted: &adopted,
                },
                notes,
            );
        }
        let inputs: &[SchemaInput] = &adopted.inputs;
        let output = self.declaration.single_output();
        let request = protocol::request_bytes(&binding, arguments, inputs, output);
        notes.executed = true;
        let observation = self
            .launch(&request, is_cancelled)
            .inspect_err(|error| notes.executed = error.kind != RefusalKind::EffectUnavailable)?;
        // Before anything keeps, cuts, or reports a byte of it, and before how the process ended
        // or what its result says is looked at: with a secret in it, nothing of what the process
        // said goes further. This is the only place that judges it, so `notes` is empty here.
        if secrets.found_in_output(&observation.stdout, &observation.stderr) {
            return Err(secret_in_output(&self.tool_id));
        }
        // What the process said is kept apart from whether it is accepted. It is data for the
        // event, through the scrub, and stays out of every message: later steps classify a
        // message by its words, and the model does not get it.
        notes.diagnostics = protocol::salvage_diagnostics(&observation.stdout);
        notes.stderr_tail = protocol::stderr_tail(&observation.stderr);
        if let Some(code) = observation.failure_code() {
            return Err(refusal(
                RefusalKind::ProcessFailed,
                format!("tool `{}` process ended with `{code}`", self.tool_id),
            ));
        }
        let accepted = protocol::verify_result(
            &observation.stdout,
            observation.stderr.len(),
            &binding,
            output,
            self.max_result_bytes,
        )
        .map_err(|rejection| {
            refusal(
                RefusalKind::ResultRejected,
                format!("tool `{}`: {}", self.tool_id, rejection.code),
            )
        })?;
        self.verify_pins(STAGE_COMMIT)?;
        let mut forbidden = self.input_paths();
        forbidden.extend(context.protected_paths.iter().cloned());
        let receipt = commit::commit(
            &CommitPlan {
                root: &self.root,
                root_display: &self.workspace,
                relative: &output.path,
                bytes: &accepted.bytes,
                committed_sha256: &accepted.committed_sha256,
                forbidden: &forbidden,
                binding: &binding,
                output_id: &accepted.output_id,
                result_sha256: &accepted.result_sha256,
                diagnostics: &accepted.diagnostics,
            },
            &|| self.judge_now(context, STAGE_COMMIT),
            is_cancelled,
        )?;
        self.note_commit(context, &receipt);
        Ok(receipt_for_model(&receipt))
    }

    /// One call of a tool a declared stage names. Everything that does not spend an attempt has
    /// already happened by the time this runs: the authority, the arguments, the pins, the secret
    /// read, and the cancel.
    fn run_stage(
        &self,
        call: &StageCall<'_>,
        notes: &mut RunNotes,
    ) -> Result<String, ProfileToolRefusal> {
        self.stage_call(call, notes)
            .map_err(|error| call.stage.with_ledger_path(error))
    }

    fn stage_call(
        &self,
        call: &StageCall<'_>,
        notes: &mut RunNotes,
    ) -> Result<String, ProfileToolRefusal> {
        let bytes = WorkspaceBytes::new(&self.workspace);
        let now_ms = now_ms();
        let (ledger, contract) = call.stage.open_ledger(now_ms, &bytes)?;
        let target = contract
            .target_for_tool(&self.tool_id)
            .ok_or_else(|| stage_ended("not_a_target"))?;
        let current_output = bytes.observe(&target.output_path);
        let asked = arguments_without_revision(call.arguments);
        let request = call
            .stage
            .attempt_request(
                &contract,
                target,
                StageCallFacts {
                    turn: self.guard.reply_number(),
                    now_ms,
                    expected_revision: call
                        .arguments
                        .get(EXPECTED_REVISION_ARGUMENT)
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    arguments_sha256: arguments_sha256(&asked),
                    request_id: call.binding.request_id.clone(),
                    current_output: &current_output,
                    adopted: call.adopted,
                },
            )
            .ok_or_else(|| stage_ended("not_a_member"))?;
        let decision = {
            let mut held = locked(&ledger)?;
            // Bytes that moved since the last call take their state, and everything below's,
            // before this call is judged against it.
            let _ = held.verdict(&bytes);
            held.begin(&target.key, request).map_err(ledger_refused)?
        };
        let limit_reached = call
            .stage
            .note_call(&target.key, !decision.counts_for_stagnation());
        let ticket = match decision {
            BeginDecision::Proceed(ticket) => ticket,
            other if limit_reached && !other.ends_run() => {
                return Err(stage_ended("no_progress_limit"));
            }
            other => return self.answer_without_process(other, &ledger, target, notes),
        };
        let outcome =
            self.stage_attempt(call, &ledger, (&contract, target), &ticket, &asked, notes);
        if notes.stage_reply_ok.is_some() {
            if notes.stage_moved {
                call.stage.note_progress(&target.key);
            } else {
                call.stage.note_no_progress(&target.key);
            }
        }
        if let Err(error) = &outcome {
            // Past the intent the permit has answered already, and this is refused as stale.
            if let Ok(mut held) = locked(&ledger) {
                let failure = terminal_failure(error.kind, notes.secret_in_output);
                let recorded = held.record_failure(&ticket, AttemptFailure::Terminal(failure));
                // Without its mark on disk a later process would go on comparing the value, so
                // what failed here is the record, and the operator has to hear that.
                if let (true, Err(refusal)) = (notes.secret_in_output, recorded) {
                    return Err(ledger_refused(refusal));
                }
            }
        }
        outcome
    }

    /// The table's rows that start nothing. What a model can correct goes back as data; the
    /// rest ends the run, as any refusal does.
    fn answer_without_process(
        &self,
        decision: BeginDecision,
        ledger: &HeldLedger,
        target: &AdmittedTarget,
        notes: &mut RunNotes,
    ) -> Result<String, ProfileToolRefusal> {
        let plain = |answer: Value| fit_for_model(answer.clone(), "", answer);
        let (ok, answer) = match decision {
            BeginDecision::NoChange { revision } => (
                true,
                plain(json!({"status": "no_change", "revision": revision})),
            ),
            BeginDecision::Stale { current_revision } => (
                false,
                plain(json!({"status": "stale", "revision": current_revision})),
            ),
            BeginDecision::UnknownRevision => (
                false,
                plain(json!({
                    "status": "unknown_revision",
                    "revision": locked(ledger)?.revision_of(&target.key),
                })),
            ),
            BeginDecision::NotReady { waiting_on } => (
                false,
                fit_for_model(
                    json!({"status": "not_ready", "waiting_on": waiting_on}),
                    "/waiting_on",
                    json!({"status": "not_ready"}),
                ),
            ),
            BeginDecision::InputNotCurrent { path } => (
                false,
                fit_for_model(
                    json!({"status": "input_not_current", "path": path}),
                    "",
                    json!({"status": "input_not_current"}),
                ),
            ),
            BeginDecision::EndedBySecretValue => {
                return Err(stage_ended(
                    "ended_by_secret_value: a secret value reached an output earlier in this run; only `--fresh-session` goes on",
                ));
            }
            BeginDecision::AttemptsExhausted(ExhaustedLimit::Target) => {
                return Err(stage_ended("attempts_exhausted_for_target"));
            }
            BeginDecision::AttemptsExhausted(ExhaustedLimit::Run) => {
                return Err(stage_ended("attempts_exhausted_for_run"));
            }
            BeginDecision::DeadlineExceeded => return Err(stage_ended("deadline_exceeded")),
            BeginDecision::FixedInputChanged { path } => {
                return Err(ledger_refused(LedgerRefusal::FixedInputChanged { path }));
            }
            BeginDecision::Refused(refused) => {
                return Err(stage_ended(&format!("refused {refused:?}")));
            }
            BeginDecision::UnknownTarget | BeginDecision::Proceed(_) => {
                return Err(stage_ended("not_a_target"));
            }
        };
        notes.stage_reply_ok = Some(ok);
        Ok(answer)
    }

    /// One attempt, already counted. The ledger's lock is not held across the process or the
    /// commit: another session's call must meet a refusal, not wait for them.
    fn stage_attempt(
        &self,
        call: &StageCall<'_>,
        ledger: &HeldLedger,
        (contract, target): (&StageContract, &AdmittedTarget),
        ticket: &AttemptTicket,
        asked: &Value,
        notes: &mut RunNotes,
    ) -> Result<String, ProfileToolRefusal> {
        let output = self.declaration.single_output();
        let request = protocol::request_bytes(call.binding, asked, &call.adopted.inputs, output);
        notes.executed = true;
        let observation = self
            .launch(&request, call.is_cancelled)
            .inspect_err(|error| notes.executed = error.kind != RefusalKind::EffectUnavailable)?;
        // As in the plain path: nothing of what the process said goes further than this.
        if call
            .secrets
            .found_in_output(&observation.stdout, &observation.stderr)
        {
            notes.secret_in_output = true;
            return Err(secret_in_output(&self.tool_id));
        }
        notes.diagnostics = protocol::salvage_diagnostics(&observation.stdout);
        notes.stderr_tail = protocol::stderr_tail(&observation.stderr);
        if let Some(code) = observation.failure_code() {
            return Err(refusal(
                RefusalKind::ProcessFailed,
                format!("tool `{}` process ended with `{code}`", self.tool_id),
            ));
        }
        let rejected = |rejection: protocol::Rejection| {
            refusal(
                RefusalKind::ResultRejected,
                format!("tool `{}`: {}", self.tool_id, rejection.code),
            )
        };
        let result = protocol::verify_stage_result(
            &observation.stdout,
            observation.stderr.len(),
            call.binding,
            self.max_result_bytes,
        )
        .map_err(rejected)?;
        // Judged here because a failed check has no candidate to report: a failing process need not
        // send an output. `record_local_result` judges again and is what holds on the accepted
        // path, as a defence of the machine's own; its `Resubmit`, `Replay`, and `Conflict` are not
        // reached from this call.
        if let Err(kind) = judge_checks(&target.required_checks, &result.checks) {
            let recorded = locked(ledger)?
                .record_failure(ticket, AttemptFailure::Check(kind))
                .map_err(ledger_refused)?;
            if let FailureDecision::Refused(refused) = recorded {
                return Err(stage_ended(&format!("refused {refused:?}")));
            }
            return Ok(self.answer_failed_check(
                call.context,
                kind,
                result.resubmit_diagnostics(),
                notes,
            ));
        }
        let accepted = result
            .accepted(&observation.stdout, output)
            .map_err(rejected)
            .and_then(|accepted| self.verify_pins(STAGE_COMMIT).map(|()| accepted))?;
        let bytes = WorkspaceBytes::new(&self.workspace);
        let before = bytes.observe(&target.output_path);
        let decision = locked(ledger)?
            .record_local_result(
                ticket,
                &accepted.committed_sha256,
                result.checks.clone(),
                &before,
            )
            .map_err(ledger_refused)?;
        match decision {
            LocalDecision::ReadyToCommit => {}
            LocalDecision::Unchanged => {
                notes.stage_reply_ok = Some(true);
                let answer = json!({
                    "status": "no_change",
                    "revision": locked(ledger)?.revision_of(&target.key),
                });
                return Ok(fit_for_model(answer.clone(), "", answer));
            }
            LocalDecision::Resubmit(kind) => {
                return Ok(self.answer_failed_check(
                    call.context,
                    kind,
                    result.resubmit_diagnostics(),
                    notes,
                ));
            }
            other => {
                return Err(stage_ended(&format!("{other:?}")));
            }
        }
        let permit = stage_ledger::record_intent(ledger, ticket, before.sha256())
            .map_err(ledger_refused)?
            .map_err(|refused| stage_ended(&format!("refused {refused:?}")))?;
        let mut forbidden = self.input_paths();
        forbidden.extend(call.context.protected_paths.iter().cloned());
        let committed = commit::commit(
            &CommitPlan {
                root: &self.root,
                root_display: &self.workspace,
                relative: &output.path,
                bytes: &accepted.bytes,
                committed_sha256: &accepted.committed_sha256,
                forbidden: &forbidden,
                binding: call.binding,
                output_id: &accepted.output_id,
                result_sha256: &accepted.result_sha256,
                diagnostics: &accepted.diagnostics,
            },
            &|| {
                self.judge_now(call.context, STAGE_COMMIT)?;
                let compared = locked(ledger)?.cas_check(ticket).map_err(ledger_refused)?;
                compared.map_err(|refused| stage_ended(&format!("refused {refused:?}")))
            },
            call.is_cancelled,
        );
        let receipt = match committed {
            Ok(receipt) => receipt,
            Err(error) => {
                let observation = commit_failure_observation(&before, &bytes.observe(&output.path));
                let _ = permit.answer(observation);
                return Err(error);
            }
        };
        self.note_commit(call.context, &receipt);
        let decision = permit
            .answer(CommitObservation::Stood(Box::new(receipt.clone())))
            .map_err(ledger_refused)?
            .map_err(|refused| stage_ended(&format!("refused {refused:?}")))?;
        notes.stage_reply_ok = Some(decision.is_progress());
        notes.stage_moved = decision.is_progress();
        if decision.is_progress() {
            call.stage.readopt_readers(contract, target, &receipt)?;
        }
        Ok(stage_receipt_for_model(&receipt, &decision))
    }

    /// The one failure a model may answer with another submission. Its diagnostics are data, in
    /// a field of their own: no message carries them, because later steps read messages by their
    /// words.
    fn answer_failed_check(
        &self,
        context: &ToolContext,
        kind: CheckFailureKind,
        diagnostics: BoundedDiagnostics,
        notes: &mut RunNotes,
    ) -> String {
        let code = check_failure_code(kind);
        notes.stage_reply_ok = Some(false);
        notes.diagnostics.clone_from(&diagnostics.items);
        emit_rejection(
            context,
            self,
            &refusal(
                RefusalKind::ResultRejected,
                format!("tool `{}`: {code}", self.tool_id),
            ),
            notes,
        );
        fit_for_model(
            json!({
                "status": "resubmit",
                "code": code,
                "diagnostics": diagnostics.items,
                "truncated": diagnostics.truncated,
            }),
            "/diagnostics",
            json!({"status": "resubmit", "code": code}),
        )
    }

    fn check_arguments(&self, arguments: &Value) -> Result<(), ProfileToolRefusal> {
        let as_refusal = |error: anyhow::Error| match error.downcast::<ProfileToolRefusal>() {
            Ok(refused) => refused,
            Err(other) => refusal(RefusalKind::SchemaViolation, other.to_string()),
        };
        self.guard.check_received(arguments).map_err(as_refusal)?;
        self.guard
            .check_before_execution(arguments)
            .map_err(as_refusal)?;
        let bytes = serde_json::to_vec(arguments).map_or(usize::MAX, |bytes| bytes.len());
        if bytes > self.max_input_bytes {
            return Err(refusal(
                RefusalKind::EffectLimitExceeded,
                format!(
                    "tool `{}` input of {bytes} bytes exceeds the {} byte limit",
                    self.tool_id, self.max_input_bytes
                ),
            ));
        }
        Ok(())
    }

    /// Every secret this run holds, read now: the sources a run's catalog is built from, what
    /// this workspace's scope already registered, and what the start received. A source that
    /// cannot be read refuses the call, because no judgement of an empty set means anything.
    fn secret_forms(&self, context: &ToolContext) -> Result<SecretForms, ProfileToolRefusal> {
        let sources = crate::config::build_secret_sources(&self.workspace, self.compatible.get())
            .map_err(|_| {
            refusal(
                RefusalKind::EffectEnvRefused,
                format!(
                    "tool `{}`: the secrets of this run cannot be read",
                    self.tool_id
                ),
            )
        })?;
        let registered = [context.eval_events_path.as_deref(), Some(&self.workspace)]
            .map(crate::sensitive_data::registered_for);
        let scoped = registered
            .iter()
            .flatten()
            .flat_map(|scope| scope.catalog_arc().values().to_vec())
            .collect::<Vec<_>>();
        Ok(SecretForms::new(
            sources
                .values()
                .iter()
                .chain(&scoped)
                .map(String::as_str)
                .chain(crate::startup_credentials::held_values()),
        ))
    }

    fn launch(
        &self,
        request: &[u8],
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<process::Observation, ProfileToolRefusal> {
        let declaration = &self.declaration;
        let command = isolation::strict_command(&StrictLaunch {
            executable: &declaration.executable.path,
            argv: &declaration.argv,
            cwd: &declaration.cwd,
            env: &declaration.env,
            read_directories: declaration.read_directories(),
            link_directories: declaration.link_paths(),
        })?;
        process::run(
            command,
            request,
            RunLimits {
                timeout: Duration::from_millis(declaration.limits.timeout_ms),
                max_stream_bytes: self.max_result_bytes,
            },
            is_cancelled,
        )
        .map_err(|error| {
            if isolation::is_descriptor_refusal(&error) {
                unavailable_descriptors(&self.tool_id)
            } else {
                refusal(
                    RefusalKind::ProcessFailed,
                    format!("tool `{}` process could not start", self.tool_id),
                )
            }
        })
    }

    /// Recorded at once, from the receipt: whatever happens to the run after the rename,
    /// the evidence of the commit is already written.
    fn note_commit(&self, context: &ToolContext, receipt: &CommitReceipt) {
        self.commits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(receipt.clone());
        self.pending_paths
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(receipt.path.clone());
        eval_events::emit(
            context.eval_events_path.as_deref(),
            json!({
                "event": "profile_tool_commit",
                "name": self.name(),
                "transport": TRANSPORT,
                "effect": EFFECT_PROPOSE_ARTIFACTS,
                "receipt": receipt,
            }),
        );
    }
}

fn declares_expected_revision(schema: &Value) -> bool {
    let is_plain_string = schema
        .get("properties")
        .and_then(|properties| properties.get(EXPECTED_REVISION_ARGUMENT))
        == Some(&json!({"type": "string"}));
    let is_required = schema
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|required| {
            required
                .iter()
                .any(|name| name == EXPECTED_REVISION_ARGUMENT)
        });
    is_plain_string && is_required
}

/// What a stage call carries through its steps.
struct StageCall<'a> {
    stage: &'a StageRun,
    arguments: &'a Value,
    context: &'a ToolContext,
    is_cancelled: &'a dyn Fn() -> bool,
    secrets: &'a SecretForms,
    binding: &'a Binding,
    adopted: &'a AdoptedInputs,
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// A fixed code and nothing an operator or a process chose: a later step reads a message by its
/// words, and a word in a name could make this end look like something to retry.
fn stage_ended(code: &str) -> ProfileToolRefusal {
    refusal(RefusalKind::StageRunEnded, code)
}

fn locked(ledger: &HeldLedger) -> Result<MutexGuard<'_, StageLedger>, ProfileToolRefusal> {
    ledger
        .lock()
        .map_err(|_| ledger_refused(LedgerRefusal::Unavailable))
}

/// What a call asks for: the arguments without the model's idea of the revision. A sealed call
/// and its repeat name different revisions and ask for the same thing.
fn arguments_without_revision(arguments: &Value) -> Value {
    let mut asked = arguments.clone();
    if let Some(fields) = asked.as_object_mut() {
        fields.remove(EXPECTED_REVISION_ARGUMENT);
    }
    asked
}

fn arguments_sha256(asked: &Value) -> String {
    sha256_hex(&serde_json::to_vec(asked).unwrap_or_default())
}

/// The bytes decide, not the words of the refusal: the same bytes as before the intent mean
/// nothing was adopted, and anything else may mean a rename stood.
fn commit_failure_observation(before: &ObservedBytes, now: &ObservedBytes) -> CommitObservation {
    let unchanged = match (before, now) {
        (ObservedBytes::Absent, ObservedBytes::Absent) => true,
        (ObservedBytes::Present(before), ObservedBytes::Present(now)) => before == now,
        _ => false,
    };
    if unchanged {
        CommitObservation::NotAdopted
    } else {
        CommitObservation::OutcomeUnknown
    }
}

const fn check_failure_code(kind: CheckFailureKind) -> &'static str {
    match kind {
        CheckFailureKind::Failed => "check_failed",
        CheckFailureKind::NotRun => "check_not_run",
        CheckFailureKind::Unknown => "check_unknown",
    }
}

/// How a refusal ends an attempt. The kind decides, and none of these can be answered with
/// another submission.
const fn terminal_failure(kind: RefusalKind, secret_in_output: bool) -> TerminalFailure {
    if secret_in_output {
        return TerminalFailure::SecretValueInOutput;
    }
    match kind {
        RefusalKind::InvalidPin
        | RefusalKind::EffectInvalidPin
        | RefusalKind::InputRevisionMismatch
        | RefusalKind::SchemaMismatch => TerminalFailure::Pin,
        RefusalKind::EffectPathRefused | RefusalKind::EffectReadOnly => TerminalFailure::Path,
        RefusalKind::ProcessFailed | RefusalKind::EffectUnavailable => TerminalFailure::Process,
        // A commit's failure is answered through its permit, which ends the attempt first.
        _ => TerminalFailure::Binding,
    }
}

fn downstream_for_model(invalidated: &[InvalidatedTarget]) -> Vec<Value> {
    invalidated
        .iter()
        .map(|target| {
            json!({
                "stage": target.key.stage,
                "target": target.key.target,
                "revision": target.revision,
            })
        })
        .collect()
}

/// What the history keeps of a tool result that is neither `Read` nor `Bash`, in characters. A
/// longer one is cut wherever the limit falls, which makes a JSON answer unreadable.
const MODEL_RESULT_CHARS: usize = 500;

/// What an answer must still say after the history has been through it.
const MODEL_ANSWER_FIELDS: [&str; 6] = [
    "/status",
    "/code",
    "/revision",
    "/truncated",
    "/stage/status",
    "/stage/revision",
];

/// Whether the model reads this answer as it was meant. The history scrubs, redacts, and cuts, so
/// only what comes out of it can tell: one token that looks like a credential turns the whole
/// text into a marker, and a home path grows when it is redacted.
fn reaches_model_whole(text: &str, answer: &Value) -> bool {
    let kept = eval_events::body_snippet_whole_tokens(text);
    if kept.chars().count() > MODEL_RESULT_CHARS {
        return false;
    }
    serde_json::from_str::<Value>(&kept).is_ok_and(|read| {
        MODEL_ANSWER_FIELDS
            .iter()
            .all(|field| read.pointer(field) == answer.pointer(field))
    })
}

/// The only way a stage tool's answer leaves the host. The list at `pointer` gives way, last item
/// first, and a long message before its item, and `truncated` says so. Past that it falls to
/// `fixed`, which holds no value an operator or a process chose. A model that does not get a
/// revision here gets it from the `stale` its next call meets.
fn fit_for_model(mut answer: Value, pointer: &str, mut fixed: Value) -> String {
    loop {
        let text = answer.to_string();
        if reaches_model_whole(&text, &answer) {
            return text;
        }
        let Some(list) = answer
            .pointer_mut(pointer)
            .and_then(Value::as_array_mut)
            .filter(|list| !list.is_empty())
        else {
            fixed["truncated"] = Value::Bool(true);
            return fixed.to_string();
        };
        let long_message = list
            .last_mut()
            .and_then(|last| last.get_mut("message"))
            .filter(|message| {
                message
                    .as_str()
                    .is_some_and(|text| text.chars().count() > 32)
            });
        match long_message {
            Some(message) => {
                let text = message.as_str().unwrap_or_default();
                let half = text.chars().count() / 2;
                *message = Value::String(text.chars().take(half).collect());
            }
            None => {
                list.pop();
            }
        }
        answer["truncated"] = Value::Bool(true);
    }
}

/// What the model needs of a receipt, where the stage stands now, and the revision each target
/// below it has to name next, so that no call of theirs starts out stale. The whole receipt is on
/// the event and in `commit_receipts()`.
fn stage_receipt_for_model(receipt: &CommitReceipt, decision: &CommitDecision) -> String {
    let (stage, fixed_stage) = match decision {
        CommitDecision::Sealed {
            revision,
            invalidated,
            ..
        } => (
            json!({
                "status": "sealed",
                "revision": revision,
                "downstream": downstream_for_model(invalidated),
            }),
            json!({"status": "sealed", "revision": revision}),
        ),
        CommitDecision::StoodUnverified {
            revision,
            reason,
            invalidated,
        } => (
            json!({
                "status": "unverified",
                "reason": reason,
                "revision": revision,
                "downstream": downstream_for_model(invalidated),
            }),
            json!({"status": "unverified", "reason": reason, "revision": revision}),
        ),
        CommitDecision::NotAdopted => (
            json!({"status": "not_adopted"}),
            json!({"status": "not_adopted"}),
        ),
    };
    fit_for_model(
        json!({
            "committed": receipt.committed,
            "committed_sha256": receipt.committed_sha256,
            "path": receipt.path,
            "stage": stage,
        }),
        "/stage/downstream",
        json!({"committed": receipt.committed, "stage": fixed_stage}),
    )
}

/// One block a terminal shows as it is: no control character of a name reaches it. A tool's
/// spinner is drawing on stderr while this runs, with no newline after its frame, so on a terminal
/// the block first clears that line. The spinner's next frame lands below it.
///
/// A run's terminal is raw, where a line feed does not go back to the left edge. A cooked terminal
/// takes the same bytes to the same place.
fn stage_refusal_block(notice: &str, stderr_is_terminal: bool) -> String {
    if stderr_is_terminal {
        format!("\r\x1b[2K{}\r\n", notice.replace('\n', "\r\n"))
    } else {
        format!("{notice}\n")
    }
}

/// What an operator needs to act on a refusal that carries more than its message may. The path
/// and the input's name are theirs, and go nowhere but here and the event.
fn stage_refusal_notice(data: &Value) -> String {
    // Field by field: no value may end a line or move a cursor, and each is scrubbed as written.
    let field = |name: &str| {
        data.get(name).and_then(Value::as_str).map(|value| {
            let plain = value
                .chars()
                .map(|c| if c.is_control() { '?' } else { c })
                .collect::<String>();
            eval_events::scrub_sensitive_text(&plain)
        })
    };
    let mut lines = vec![format!(
        "stage record refused: {}",
        field("code").unwrap_or_else(|| "unknown".to_string())
    )];
    if let Some(input) = field("input") {
        lines.push(format!("  changed input: {input}"));
    }
    if let Some(ledger) = field("ledger_path") {
        lines.push(format!("  ledger: {ledger}"));
    }
    if let Some(recovery) = field("recovery") {
        lines.push(format!("  to recover: {recovery}"));
    }
    lines.join("\n")
}

/// Shows the operator a refusal of the record, wherever it was first found.
pub(crate) fn show_stage_refusal(data: &Value) {
    let block = stage_refusal_block(
        &stage_refusal_notice(data),
        crate::tui::terminal::stderr_is_tty(),
    );
    // One write under the lock the spinner takes, so no frame lands inside the block.
    let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), block.as_bytes());
}

fn emit_stage_refusal(context: &ToolContext, tool: &ProposalTool, data: &Value) {
    show_stage_refusal(data);
    let mut event = json!({
        "event": "structured_stage_refused",
        "name": tool.name(),
        "transport": TRANSPORT,
    });
    if let (Some(fields), Some(detail)) = (event.as_object_mut(), data.as_object()) {
        for (key, value) in detail {
            let scrubbed = value.as_str().map_or_else(
                || value.clone(),
                |text| json!(eval_events::scrub_sensitive_text(text)),
            );
            fields.insert(key.clone(), scrubbed);
        }
    }
    eval_events::emit(context.eval_events_path.as_deref(), event);
}

/// A fixed code: not the value, not the form, not where it was.
fn secret_in_output(tool: &str) -> ProfileToolRefusal {
    refusal(
        RefusalKind::ResultRejected,
        format!("tool `{tool}`: secret_value_in_output"),
    )
}

/// Nothing was started: a descriptor left open by the host's caller could have reached it.
fn unavailable_descriptors(tool: &str) -> ProfileToolRefusal {
    refusal(
        RefusalKind::EffectUnavailable,
        format!("tool `{tool}` was not started: not every inherited descriptor could be marked"),
    )
}

fn emit_call(context: &ToolContext, tool: &ProposalTool, arguments: &Value) {
    eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "profile_tool_effect_call",
            "name": tool.name(),
            "transport": TRANSPORT,
            "effect": EFFECT_PROPOSE_ARTIFACTS,
            "arguments": eval_events::argument_shape(arguments),
        }),
    );
}

/// The tool's result, as the model gets it: the receipt without what the process said. That
/// stays on the event and in `commit_receipts()`.
fn receipt_for_model(receipt: &CommitReceipt) -> String {
    let mut value = serde_json::to_value(receipt).unwrap_or(Value::Null);
    if let Some(fields) = value.as_object_mut() {
        fields.remove("diagnostics");
    }
    value.to_string()
}

fn emit_rejection(
    context: &ToolContext,
    tool: &ProposalTool,
    error: &ProfileToolRefusal,
    notes: &RunNotes,
) {
    eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "profile_tool_effect_rejected",
            "name": tool.name(),
            "transport": TRANSPORT,
            "effect": EFFECT_PROPOSE_ARTIFACTS,
            "kind": error.kind.as_str(),
            "reason": eval_events::body_snippet(&error.reason),
            "diagnostics": notes.diagnostics,
            "stderr_tail": notes.stderr_tail,
        }),
    );
}

/// A configuration a unit test can judge. Nothing in it reaches a provider.
#[cfg(test)]
pub(crate) fn config_for_test(root: &Path) -> Config {
    use crate::config::{
        ConfigFieldSources, NarrationMode, OpenAiApi, PlanPreset, PromptLayout, Provider,
    };
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join(".state-for-test"),
        eval_events_path: None,
        completion_contract_path: None,
        yes: true,
        offline: true,
        context_budget: 1000,
        model: "m".to_string(),
        provider: Provider::LmStudio,
        tool_protocol: None,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "m".to_string(),
        planner_provider: Provider::LmStudio,
        planner_think: None,
        classifier_model: "m".to_string(),
        classifier_provider: Provider::LmStudio,
        openai_compatible: None,
        ollama_host: "http://127.0.0.1:9".to_string(),
        ollama_think: None,
        lm_studio_host: "http://127.0.0.1:9".to_string(),
        num_predict: 1,
        max_iterations: 1,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 1,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 0,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

/// What one call learned before it ended, for its records.
#[derive(Debug, Default)]
struct RunNotes {
    executed: bool,
    diagnostics: Vec<Diagnostic>,
    stderr_tail: String,
    /// A stage call that returned as data: whether what it asked for happened.
    stage_reply_ok: Option<bool>,
    /// A commit stood and sealed. A commit that stood unverified moved the bytes, not the stage.
    stage_moved: bool,
    secret_in_output: bool,
}

/// The whole of one call on every platform. The launch is unconfined here, which only a
/// test build can ask for: `tests/profile_tool_commit.rs` holds the real isolation and the
/// public entries, and this holds what is decided around them, also where no backend exists.
#[cfg(all(test, unix))]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};

    use super::super::descriptor::{RegistryBinding, directory_pin};
    use super::super::isolation::test_hook::{Unconfined, unconfined};
    use super::super::protocol::{
        DirectoryPin, FilePin, OutputDeclaration, ProcessDeclaration, ProcessLimits, encode_base64,
    };
    use super::super::schema::binding::{self as schema_binding, ToolPins};
    use super::super::schema::invocation::ReceiveState;
    use super::super::secret_output::SecretForms;
    use super::super::{ProfileToolRefusal, RefusalKind};
    use super::{ProposalParts, ProposalTool, SessionAccess};
    use crate::mode::ExecutionMode;
    use crate::tools::allow_policy::{self, AllowTarget};
    use crate::tools::registry::{ToolContext, ToolRegistry};
    use crate::tools::workspace_policy::WorkspacePolicy;

    const CANDIDATE: &[u8] = b"{\"mode\":\"fast\"}\n";
    const OUTPUT: &str = "out/settings.json";
    const INPUT: &str = "inputs/allowed.txt";
    const PRELUDE: &str = r#"request=$(cat)
echo started >> "$MARKS/started"
field() { printf '%s' "$request" | sed -n "s/.*\"$1\":\"\([^\"]*\)\".*/\1/p"; }
"#;

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    /// A correct result, with one piece replaced per scenario.
    fn result_script(
        request_id: &str,
        runtime: &str,
        revision: &str,
        schema: &str,
        checks: &str,
        outputs: &str,
    ) -> String {
        format!(
            r#"{PRELUDE}printf '{{"protocol":"profile-tool.result/v1","request_id":"%s","tool_id":"%s","runtime_sha256":"%s","input_revision":"%s","schema_sha256":"%s","checks":{checks},"outputs":{outputs},"diagnostics":[]}}' "{request_id}" "$(field tool_id)" "{runtime}" "{revision}" "{schema}"
"#
        )
    }

    fn good_output() -> String {
        format!(
            r#"[{{"id":"artifact","sha256":"{}","bytes_base64":"{}"}}]"#,
            sha256(CANDIDATE),
            encode_base64(CANDIDATE)
        )
    }

    const CHECKS_PASSED: &str = r#"[{"id":"shape","status":"passed"}]"#;

    fn good_script() -> String {
        result_script(
            "$(field request_id)",
            "$(field runtime_sha256)",
            "$(field input_revision)",
            "$(field schema_sha256)",
            CHECKS_PASSED,
            &good_output(),
        )
    }

    struct Bench {
        _guard: Unconfined,
        _temp: tempfile::TempDir,
        workspace: PathBuf,
        marks: PathBuf,
        tool_dir: PathBuf,
        tool: Arc<ProposalTool>,
    }

    struct Options {
        timeout_ms: u64,
        max_result_bytes: usize,
        max_input_bytes: usize,
        env: Vec<(&'static str, &'static str)>,
        /// The executable is a copy in a directory of its own, instead of the system's.
        own_executable: bool,
    }

    impl Default for Options {
        fn default() -> Self {
            Self {
                timeout_ms: 20_000,
                max_result_bytes: 64 * 1024,
                max_input_bytes: 1 << 20,
                env: Vec::new(),
                own_executable: false,
            }
        }
    }

    fn bench(script: &str) -> Bench {
        bench_with(script, Options::default())
    }

    fn bench_with(script: &str, options: Options) -> Bench {
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let (workspace, marks, tool_dir, registry) = (
            base.join("workspace"),
            base.join("marks"),
            base.join("tool"),
            base.join("registry"),
        );
        for directory in [
            &workspace,
            &marks,
            &tool_dir,
            &registry.join("profile-tools"),
        ] {
            std::fs::create_dir_all(directory).unwrap();
        }
        std::fs::create_dir_all(workspace.join("inputs")).unwrap();
        std::fs::write(workspace.join(INPUT), "alpha\n").unwrap();
        let script_path = tool_dir.join("tool.sh");
        std::fs::write(&script_path, script).unwrap();
        let mut shell = std::fs::canonicalize("/bin/sh").unwrap();
        if options.own_executable {
            let directory = base.join("executable");
            std::fs::create_dir_all(&directory).unwrap();
            let copy = directory.join("sh");
            std::fs::copy(&shell, &copy).unwrap();
            shell = copy;
        }
        let mut env =
            std::collections::BTreeMap::from([("MARKS".to_string(), marks.display().to_string())]);
        env.insert("PATH_FOR_TEST".to_string(), "unused".to_string());
        for (key, value) in options.env {
            env.insert(key.to_string(), value.to_string());
        }
        let declaration = ProcessDeclaration {
            protocol: super::super::protocol::PROCESS_PROTOCOL.to_string(),
            executable: FilePin {
                sha256: sha256(&std::fs::read(&shell).unwrap()),
                path: shell,
            },
            dependency_directories: vec![DirectoryPin {
                path: tool_dir.clone(),
                sha256: directory_pin(&tool_dir).unwrap(),
            }],
            link_directories: Vec::new(),
            argv: vec![script_path.display().to_string()],
            cwd: tool_dir.clone(),
            env,
            limits: ProcessLimits {
                timeout_ms: options.timeout_ms,
                max_result_bytes: options.max_result_bytes,
            },
            outputs: vec![OutputDeclaration {
                id: "artifact".to_string(),
                path: OUTPUT.to_string(),
            }],
        };
        let schema = json!({
            "protocol": "profile-tool.schema/v1",
            "root": {
                "type": "object",
                "additionalProperties": false,
                "properties": {"mode": {"type": "string"}},
            },
            "inputs": [INPUT],
        });
        let descriptor_text = json!({"process": declaration, "schema": schema}).to_string();
        std::fs::write(
            registry.join("profile-tools/unit-tool.json"),
            &descriptor_text,
        )
        .unwrap();
        let binding = RegistryBinding::capture_for_test(&registry, "unit-tool");
        let descriptor_sha256 = sha256(descriptor_text.as_bytes());
        let pins =
            super::super::descriptor::verify_process_pins(&declaration, "unit-tool", &workspace)
                .unwrap();
        let adoption = schema_binding::adopt(
            &schema,
            &ToolPins {
                tool_id: "unit-tool",
                version: "1.0.0",
                descriptor_sha256: &descriptor_sha256,
                max_input_bytes: 1 << 20,
            },
            &workspace,
            "unit-profile",
        )
        .unwrap();
        let mut state = ReceiveState::default();
        let guard = state.add(
            "ProposeUnit",
            "Propose one object.",
            adoption,
            binding.clone(),
            &descriptor_sha256,
        );
        let tool = ProposalTool::new(ProposalParts {
            name: "ProposeUnit".to_string(),
            description: "Propose one object.".to_string(),
            tool_id: "unit-tool".to_string(),
            tool_version: "1.0.0".to_string(),
            descriptor_sha256,
            declaration,
            registry: binding,
            pins,
            workspace: workspace.clone(),
            guard,
            max_input_bytes: options.max_input_bytes,
            max_output_bytes: 64 * 1024,
            receipts: Arc::new(Mutex::new(Vec::new())),
            commits: Arc::new(Mutex::new(Vec::new())),
        })
        .unwrap();
        Bench {
            _guard: unconfined(),
            _temp: temp,
            workspace,
            marks,
            tool_dir,
            tool: Arc::new(tool),
        }
    }

    impl Bench {
        fn context(&self) -> ToolContext {
            ToolContext {
                root: self.workspace.clone(),
                mode: ExecutionMode::Act,
                auto_approve: true,
                interactive_approval: false,
                offline: false,
                workspace_policy: WorkspacePolicy::for_task_request(),
                eval_events_path: None,
                expected_paths: Vec::new(),
                protected_paths: Vec::new(),
            }
        }

        fn starts(&self) -> usize {
            std::fs::read_to_string(self.marks.join("started"))
                .map_or(0, |text| text.lines().count())
        }

        fn saved(&self) -> Option<Vec<u8>> {
            std::fs::read(self.workspace.join(OUTPUT)).ok()
        }

        fn registry(&self) -> ToolRegistry {
            ToolRegistry::with_profile_tools_and_proposals(
                [],
                [self.tool.clone()],
                &["Read".to_string()],
            )
            .unwrap()
        }

        fn call(&self) -> anyhow::Result<String> {
            self.tool
                .execute(&json!({"mode": "fast"}), &self.context(), &|| false)
        }
    }

    fn kind(error: &anyhow::Error) -> RefusalKind {
        error
            .downcast_ref::<ProfileToolRefusal>()
            .unwrap_or_else(|| panic!("not a refusal: {error:#}"))
            .kind
    }

    #[test]
    fn the_whole_call_commits_exactly_the_candidate_and_returns_its_receipt() {
        let bench = bench(&good_script());
        let text = bench.call().unwrap();
        let receipt: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(bench.saved().as_deref(), Some(CANDIDATE));
        assert_eq!(receipt["committed"], true);
        assert_eq!(receipt["committed_sha256"], sha256(CANDIDATE));
        assert_eq!(receipt["committed_bytes"], CANDIDATE.len());
        assert_eq!(receipt["path"], OUTPUT);
        assert_eq!(bench.starts(), 1);
        assert_eq!(bench.tool.take_committed_paths(), [OUTPUT.to_string()]);
        assert!(bench.tool.take_committed_paths().is_empty());
    }

    #[test]
    fn the_process_gets_the_adopted_inputs_on_stdin_and_only_its_declared_env() {
        // SAFETY: this name is used by this test alone.
        unsafe { std::env::set_var("G3_UNIT_PARENT_SECRET", "dummy-parent-value") };
        let script = format!(
            "{}env > \"$MARKS/env\"\nprintf '%s' \"$request\" > \"$MARKS/request\"\n{}",
            PRELUDE,
            good_script().replace(PRELUDE, "")
        );
        let bench = bench_with(
            &script,
            Options {
                env: vec![("TOOL_MODE", "unit")],
                ..Options::default()
            },
        );
        bench.call().unwrap();
        // SAFETY: as above.
        unsafe { std::env::remove_var("G3_UNIT_PARENT_SECRET") };
        let env = std::fs::read_to_string(bench.marks.join("env")).unwrap();
        assert!(env.contains("TOOL_MODE=unit"), "{env}");
        assert!(!env.contains("G3_UNIT_PARENT_SECRET"), "{env}");
        assert!(!env.contains("dummy-parent-value"), "{env}");
        let names = env
            .lines()
            .filter_map(|line| line.split('=').next())
            .collect::<Vec<_>>();
        for name in names {
            assert!(
                [
                    "TOOL_MODE",
                    "MARKS",
                    "PATH_FOR_TEST",
                    "PWD",
                    "OLDPWD",
                    "SHLVL",
                    "_",
                    "PATH",
                    "__CF_USER_TEXT_ENCODING"
                ]
                .contains(&name),
                "{name}"
            );
        }
        let request: Value =
            serde_json::from_str(&std::fs::read_to_string(bench.marks.join("request")).unwrap())
                .unwrap();
        assert_eq!(request["protocol"], "profile-tool.request/v1");
        assert_eq!(request["arguments"], json!({"mode": "fast"}));
        assert_eq!(request["inputs"][0]["path"], INPUT);
        assert_eq!(
            request["inputs"][0]["bytes_base64"],
            encode_base64(b"alpha\n")
        );
        assert_eq!(request["outputs"], json!(["artifact"]));
        assert!(
            request.to_string().find(OUTPUT).is_none(),
            "the destination is hidden"
        );
    }

    #[test]
    fn a_result_that_does_not_bind_or_pass_saves_nothing() {
        let bad_outputs = format!(
            r#"[{{"id":"artifact","sha256":"{}","bytes_base64":"{}"}}]"#,
            "0".repeat(64),
            encode_base64(CANDIDATE)
        );
        let rid = "$(field request_id)";
        let run = "$(field runtime_sha256)";
        let rev = "$(field input_revision)";
        let sch = "$(field schema_sha256)";
        let good = good_output();
        for (label, script, code) in [
            (
                "request",
                result_script("other", run, rev, sch, CHECKS_PASSED, &good),
                "request_id_mismatch",
            ),
            (
                "runtime",
                result_script(rid, "other", rev, sch, CHECKS_PASSED, &good),
                "runtime_pin_mismatch",
            ),
            (
                "revision",
                result_script(rid, run, "other", sch, CHECKS_PASSED, &good),
                "input_revision_mismatch",
            ),
            (
                "schema",
                result_script(rid, run, rev, "other", CHECKS_PASSED, &good),
                "schema_hash_mismatch",
            ),
            (
                "digest",
                result_script(rid, run, rev, sch, CHECKS_PASSED, &bad_outputs),
                "output_digest_mismatch",
            ),
            (
                "missing",
                result_script(rid, run, rev, sch, CHECKS_PASSED, "[]"),
                "missing_output",
            ),
            (
                "failed",
                result_script(
                    rid,
                    run,
                    rev,
                    sch,
                    r#"[{"id":"a","status":"failed"}]"#,
                    &good,
                ),
                "check_failed",
            ),
            (
                "not_run",
                result_script(
                    rid,
                    run,
                    rev,
                    sch,
                    r#"[{"id":"a","status":"not_run"}]"#,
                    &good,
                ),
                "check_not_run",
            ),
            (
                "unknown",
                result_script(
                    rid,
                    run,
                    rev,
                    sch,
                    r#"[{"id":"a","status":"unknown"}]"#,
                    &good,
                ),
                "check_unknown",
            ),
            (
                "exit 0 prose",
                format!("{PRELUDE}echo PASS accepted"),
                "result_malformed",
            ),
            (
                "exit 0 silent",
                format!("{PRELUDE}exit 0"),
                "result_malformed",
            ),
        ] {
            let bench = bench(&script);
            std::fs::create_dir_all(bench.workspace.join("out")).unwrap();
            std::fs::write(bench.workspace.join(OUTPUT), b"old").unwrap();
            let error = bench.call().unwrap_err();
            assert_eq!(
                kind(&error),
                RefusalKind::ResultRejected,
                "{label}: {error:#}"
            );
            assert!(error.to_string().contains(code), "{label}: {error:#}");
            assert_eq!(bench.saved().as_deref(), Some(&b"old"[..]), "{label}");
            assert_eq!(bench.starts(), 1, "{label}: no retry hides a failure");
            assert!(bench.tool.take_committed_paths().is_empty(), "{label}");
        }
    }

    /// A declaration that names no stage: one failed check ends it, as it always did.
    #[test]
    fn a_tool_outside_a_stage_set_still_ends_at_one_failed_check() {
        let failing = result_script(
            "$(field request_id)",
            "$(field runtime_sha256)",
            "$(field input_revision)",
            "$(field schema_sha256)",
            r#"[{"id":"shape","status":"failed"}]"#,
            &good_output(),
        );
        let bench = bench(&failing);
        let error = bench.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::ResultRejected);
        assert!(error.to_string().contains("check_failed"), "{error:#}");
        assert_eq!(bench.starts(), 1);
        // Nor can it join a stage set by what its schema does not have.
        assert_eq!(
            bench.tool.stage_member().unwrap_err().kind,
            RefusalKind::StageContractRefused
        );
    }

    mod stages {
        use std::collections::BTreeMap;
        use std::path::PathBuf;
        use std::sync::{Arc, Mutex};

        use serde_json::{Value, json};

        use super::super::super::RefusalKind;
        use super::super::super::commit::test_hook::{self, Stage};
        use super::super::super::descriptor::{RegistryBinding, directory_pin};
        use super::super::super::isolation::test_hook::{Unconfined, unconfined};
        use super::super::super::protocol::{
            DirectoryPin, FilePin, OutputDeclaration, PROCESS_PROTOCOL, ProcessDeclaration,
            ProcessLimits, encode_base64,
        };
        use super::super::super::schema::binding::{self as schema_binding, ToolPins};
        use super::super::super::schema::invocation::{
            ReceiveState, enter_session, for_provider_worker, receive_native,
        };
        use super::super::{
            BoundedDiagnostics, CheckFailureKind, CommitReceipt, Diagnostic,
            EXPECTED_REVISION_ARGUMENT, MODEL_RESULT_CHARS, ProposalParts, ProposalTool, RunNotes,
            arguments_sha256, arguments_without_revision, fit_for_model,
        };
        use super::{DUMMY, kind, sha256};
        use crate::mode::ExecutionMode;
        use crate::planner::structured_stages::contract::{
            HostToolFacts, NarrowedStageLimits, StageContract, StageContractDraft,
            StageDeclaration, StageLimits, StageRunFacts, StageSetDeclaration, TargetDeclaration,
            TargetKey,
        };
        use crate::planner::structured_stages::ledger::{
            HeldLedger, LedgerEntry, LedgerRefusal, LedgerRegistry, WorkspaceBytes,
        };
        use crate::planner::structured_stages::state::{
            BeginDecision, CommitDecision, Phase, UnverifiedReason,
        };
        use crate::planner::structured_stages::{
            StageMember, StageRun, StageScope, refuse_unmet_session,
        };
        use crate::tools::registry::ToolContext;
        use crate::tools::workspace_policy::WorkspacePolicy;

        const PROFILE: &str = "stage-unit-profile";
        const UPPER: &str = "stage-upper";
        const LOWER: &str = "stage-lower";
        const UPPER_OUTPUT: &str = "out/upper.json";
        const LOWER_OUTPUT: &str = "out/lower.json";
        const UPPER_FIXED: &str = "fixed/upper.json";
        const LOWER_FIXED: &str = "fixed/lower.json";
        const VERSION_ONE: &[u8] = b"{\"version\":1}\n";
        const VERSION_TWO: &[u8] = b"{\"version\":2}\n";

        fn output_for(bytes: &[u8]) -> String {
            format!(
                r#"[{{"id":"artifact","sha256":"{}","bytes_base64":"{}"}}]"#,
                sha256(bytes),
                encode_base64(bytes)
            )
        }

        /// A process steered by files only the test writes, so one script serves every call.
        fn script(tool: &str) -> String {
            let (one, two) = (output_for(VERSION_ONE), output_for(VERSION_TWO));
            format!(
                r#"request=$(cat)
echo started >> "$MARKS/started-{tool}"
printf '%s' "$request" > "$MARKS/request-{tool}"
[ -f "$MARKS/clobber" ] && printf changed > "$(cat "$MARKS/clobber")"
[ -f "$MARKS/broken" ] && {{ printf 'not a result'; exit 0; }}
field() {{ printf '%s' "$request" | sed -n "s/.*\"$1\":\"\([^\"]*\)\".*/\1/p"; }}
checks='[{{"id":"shape","status":"passed"}}]'
[ -f "$MARKS/checks-{tool}" ] && checks=$(cat "$MARKS/checks-{tool}")
outputs='{one}'
[ -f "$MARKS/second-{tool}" ] && outputs='{two}'
diagnostics='[]'
[ -f "$MARKS/diagnostics-{tool}" ] && diagnostics=$(cat "$MARKS/diagnostics-{tool}")
printf '{{"protocol":"profile-tool.result/v1","request_id":"%s","tool_id":"%s","runtime_sha256":"%s","input_revision":"%s","schema_sha256":"%s","checks":%s,"outputs":%s,"diagnostics":%s}}' "$(field request_id)" "$(field tool_id)" "$(field runtime_sha256)" "$(field input_revision)" "$(field schema_sha256)" "$checks" "$outputs" "$diagnostics"
"#
            )
        }

        struct StageBench {
            _guard: Unconfined,
            temp: tempfile::TempDir,
            attempts_per_target: u32,
            workspace: PathBuf,
            marks: PathBuf,
            upper: Arc<ProposalTool>,
            lower: Arc<ProposalTool>,
            run: Arc<StageRun>,
            state: Arc<ReceiveState>,
        }

        fn facts(tool: &str, output: &str, inputs: &[&str], attempts: u32) -> HostToolFacts {
            HostToolFacts {
                tool_id: tool.to_string(),
                output_id: "artifact".to_string(),
                output_path: output.to_string(),
                schema_inputs: inputs.iter().map(|path| (*path).to_string()).collect(),
                has_revision_argument: false,
                has_compiler: false,
                required_checks: vec!["shape".to_string()],
                limits: StageLimits {
                    attempts_per_target: attempts,
                    attempts_per_run: 64,
                    deadline_seconds: 3600,
                },
            }
        }

        fn stage(id: &str, tool: &str) -> StageDeclaration {
            StageDeclaration {
                id: id.to_string(),
                targets: vec![TargetDeclaration {
                    id: "main".to_string(),
                    tool_id: tool.to_string(),
                }],
                depends_on: Vec::new(),
                required_checks: Vec::new(),
            }
        }

        fn stage_bench() -> StageBench {
            stage_bench_with(8)
        }

        fn stage_bench_with(attempts_per_target: u32) -> StageBench {
            let temp = tempfile::tempdir().unwrap();
            let base = std::fs::canonicalize(temp.path()).unwrap();
            for directory in [
                base.join("workspace/fixed"),
                base.join("marks"),
                base.join("registry/profile-tools"),
            ] {
                std::fs::create_dir_all(directory).unwrap();
            }
            std::fs::write(base.join("workspace").join(UPPER_FIXED), b"upper seed").unwrap();
            std::fs::write(base.join("workspace").join(LOWER_FIXED), b"lower seed").unwrap();
            assemble(temp, attempts_per_target, LedgerEntry::Fresh)
        }

        /// The tools over directories that are already there. Called again with a registry of its
        /// own, this stands for a new process: nothing in memory but G2's adoptions carries over.
        fn assemble(
            temp: tempfile::TempDir,
            attempts_per_target: u32,
            entry: LedgerEntry,
        ) -> StageBench {
            let base = std::fs::canonicalize(temp.path()).unwrap();
            let (workspace, marks, registry) = (
                base.join("workspace"),
                base.join("marks"),
                base.join("registry"),
            );
            let host_facts = [
                facts(UPPER, UPPER_OUTPUT, &[UPPER_FIXED], attempts_per_target),
                facts(
                    LOWER,
                    LOWER_OUTPUT,
                    &[UPPER_OUTPUT, LOWER_FIXED],
                    attempts_per_target,
                ),
            ];
            let draft = StageContractDraft::new(
                &StageSetDeclaration {
                    stages: vec![stage("lower", LOWER), stage("upper", UPPER)],
                    limits: NarrowedStageLimits::default(),
                },
                &host_facts,
            )
            .unwrap();

            let mut state = ReceiveState::default();
            let mut build = |id: &str, name: &str, output: &str, inputs: &[&str]| {
                let tool_dir = base.join(format!("tool-{id}"));
                std::fs::create_dir_all(&tool_dir).unwrap();
                let script_path = tool_dir.join("tool.sh");
                std::fs::write(&script_path, script(id)).unwrap();
                let shell = std::fs::canonicalize("/bin/sh").unwrap();
                let declaration = ProcessDeclaration {
                    protocol: PROCESS_PROTOCOL.to_string(),
                    executable: FilePin {
                        sha256: sha256(&std::fs::read(&shell).unwrap()),
                        path: shell,
                    },
                    dependency_directories: vec![DirectoryPin {
                        path: tool_dir.clone(),
                        sha256: directory_pin(&tool_dir).unwrap(),
                    }],
                    link_directories: Vec::new(),
                    argv: vec![script_path.display().to_string()],
                    cwd: tool_dir.clone(),
                    env: BTreeMap::from([("MARKS".to_string(), marks.display().to_string())]),
                    limits: ProcessLimits {
                        timeout_ms: 20_000,
                        max_result_bytes: 64 * 1024,
                    },
                    outputs: vec![OutputDeclaration {
                        id: "artifact".to_string(),
                        path: output.to_string(),
                    }],
                };
                let schema = json!({
                    "protocol": "profile-tool.schema/v1",
                    "root": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "mode": {"type": "string"},
                            EXPECTED_REVISION_ARGUMENT: {"type": "string"},
                        },
                        "required": [EXPECTED_REVISION_ARGUMENT],
                    },
                    "inputs": inputs,
                });
                let descriptor_text = json!({"process": declaration, "schema": schema}).to_string();
                std::fs::write(
                    registry.join(format!("profile-tools/{id}.json")),
                    &descriptor_text,
                )
                .unwrap();
                let binding = RegistryBinding::capture_for_test(&registry, id);
                let descriptor_sha256 = sha256(descriptor_text.as_bytes());
                let pins = super::super::super::descriptor::verify_process_pins(
                    &declaration,
                    id,
                    &workspace,
                )
                .unwrap();
                let adoption = schema_binding::adopt_with_upstream(
                    &schema,
                    &ToolPins {
                        tool_id: id,
                        version: "1.0.0",
                        descriptor_sha256: &descriptor_sha256,
                        max_input_bytes: 1 << 20,
                    },
                    &workspace,
                    PROFILE,
                    &draft.upstream_inputs(id),
                )
                .unwrap();
                let guard = state.add(
                    name,
                    "Propose one object.",
                    adoption,
                    binding.clone(),
                    &descriptor_sha256,
                );
                Arc::new(
                    ProposalTool::new(ProposalParts {
                        name: name.to_string(),
                        description: "Propose one object.".to_string(),
                        tool_id: id.to_string(),
                        tool_version: "1.0.0".to_string(),
                        descriptor_sha256,
                        declaration,
                        registry: binding,
                        pins,
                        workspace: workspace.clone(),
                        guard,
                        max_input_bytes: 1 << 20,
                        max_output_bytes: 64 * 1024,
                        receipts: Arc::new(Mutex::new(Vec::new())),
                        commits: Arc::new(Mutex::new(Vec::new())),
                    })
                    .unwrap(),
                )
            };
            let upper = build(UPPER, "ProposeUpper", UPPER_OUTPUT, &[UPPER_FIXED]);
            let lower = build(
                LOWER,
                "ProposeLower",
                LOWER_OUTPUT,
                &[UPPER_OUTPUT, LOWER_FIXED],
            );
            let members: BTreeMap<String, StageMember> = BTreeMap::from([
                (UPPER.to_string(), upper.stage_member().unwrap()),
                (LOWER.to_string(), lower.stage_member().unwrap()),
            ]);
            let run = Arc::new(StageRun::new(
                draft,
                members,
                workspace.clone(),
                PROFILE.to_string(),
                Arc::new(LedgerRegistry::default()),
            ));
            upper.attach_stage(run.clone());
            lower.attach_stage(run.clone());
            run.bind_session(
                StageRunFacts::for_test(true, false),
                &base.join("state"),
                entry,
            )
            .unwrap();
            StageBench {
                _guard: unconfined(),
                temp,
                attempts_per_target,
                workspace,
                marks,
                upper,
                lower,
                run,
                state: Arc::new(state),
            }
        }

        fn upper_key() -> TargetKey {
            TargetKey {
                stage: "upper".to_string(),
                target: "main".to_string(),
            }
        }

        fn lower_key() -> TargetKey {
            TargetKey {
                stage: "lower".to_string(),
                target: "main".to_string(),
            }
        }

        impl StageBench {
            /// This process ends, and another starts with this entry.
            fn restarted(self, entry: LedgerEntry) -> Self {
                let Self {
                    _guard,
                    temp,
                    attempts_per_target,
                    workspace,
                    marks,
                    upper,
                    lower,
                    run,
                    state,
                } = self;
                // Everything of the old one goes first: its ledger's lock, and its guard, whose
                // end would otherwise come after the new one's start.
                drop((_guard, workspace, marks, upper, lower, run, state));
                assemble(temp, attempts_per_target, entry)
            }

            /// A later session of this process, which binds again with what it was started with.
            fn next_session(&self, entry: LedgerEntry) {
                self.run
                    .bind_session(
                        StageRunFacts::for_test(true, false),
                        &self.workspace.parent().unwrap().join("state"),
                        entry,
                    )
                    .unwrap();
            }

            fn context(&self) -> ToolContext {
                ToolContext {
                    root: self.workspace.clone(),
                    mode: ExecutionMode::Act,
                    auto_approve: true,
                    interactive_approval: false,
                    offline: false,
                    workspace_policy: WorkspacePolicy::for_task_request(),
                    eval_events_path: None,
                    expected_paths: Vec::new(),
                    protected_paths: Vec::new(),
                }
            }

            fn call(
                &self,
                tool: &ProposalTool,
                mode: &str,
                revision: &str,
            ) -> anyhow::Result<Value> {
                self.call_cancelling(tool, mode, revision, &|| false)
            }

            fn call_cancelling(
                &self,
                tool: &ProposalTool,
                mode: &str,
                revision: &str,
                is_cancelled: &dyn Fn() -> bool,
            ) -> anyhow::Result<Value> {
                let arguments = json!({"mode": mode, EXPECTED_REVISION_ARGUMENT: revision});
                let text = tool.execute(&arguments, &self.context(), is_cancelled)?;
                Ok(serde_json::from_str(&text).unwrap())
            }

            fn starts(&self, tool: &str) -> usize {
                std::fs::read_to_string(self.marks.join(format!("started-{tool}")))
                    .map_or(0, |text| text.lines().count())
            }

            fn mark(&self, name: &str, text: &str) {
                std::fs::write(self.marks.join(name), text).unwrap();
            }

            fn unmark(&self, name: &str) {
                std::fs::remove_file(self.marks.join(name)).unwrap();
            }

            fn saved(&self, path: &str) -> Option<Vec<u8>> {
                std::fs::read(self.workspace.join(path)).ok()
            }

            fn ledger(&self) -> HeldLedger {
                let bytes = WorkspaceBytes::new(&self.workspace);
                self.run
                    .open_ledger(super::super::now_ms(), &bytes)
                    .unwrap()
                    .0
            }

            /// Phase, generation, and reason, by the bytes there now.
            fn status(&self, key: &TargetKey) -> (Phase, u64, Option<UnverifiedReason>) {
                let bytes = WorkspaceBytes::new(&self.workspace);
                let held = self.ledger();
                let verdict = held.lock().unwrap().verdict(&bytes);
                let status = verdict
                    .obligations()
                    .iter()
                    .find(|status| status.key == *key)
                    .unwrap();
                (status.phase, status.generation, status.unverified_reason)
            }

            fn attempts(&self, key: &TargetKey) -> u32 {
                self.ledger()
                    .lock()
                    .unwrap()
                    .attempt_identity(key)
                    .map_or(0, |identity| identity.attempt)
            }

            fn revision(&self, key: &TargetKey) -> String {
                self.ledger().lock().unwrap().revision_of(key).unwrap()
            }

            fn seal_upper(&self) -> Value {
                let answer = self.call(&self.upper, "fast", "initial").unwrap();
                assert_eq!(answer["stage"]["status"], "sealed", "{answer}");
                answer
            }
        }

        fn request_of(bench: &StageBench, tool: &str) -> Value {
            let text =
                std::fs::read_to_string(bench.marks.join(format!("request-{tool}"))).unwrap();
            serde_json::from_str(&text).unwrap()
        }

        fn downstream_revision(answer: &Value) -> String {
            let downstream = answer["stage"]["downstream"].as_array().unwrap();
            assert_eq!(downstream.len(), 1, "{answer}");
            assert_eq!(downstream[0]["stage"], "lower");
            assert_eq!(downstream[0]["target"], "main");
            downstream[0]["revision"].as_str().unwrap().to_string()
        }

        /// What a model sees: A's answer names B's revision, and B goes through on that alone.
        #[test]
        fn a_commit_tells_the_model_the_revision_below_and_the_next_call_is_not_stale() {
            let bench = stage_bench();
            let answer = bench.seal_upper();
            let lower_revision = downstream_revision(&answer);
            assert_ne!(lower_revision, "initial");
            assert_eq!(lower_revision, bench.revision(&lower_key()));
            assert_eq!(bench.saved(UPPER_OUTPUT).as_deref(), Some(VERSION_ONE));

            let answer = bench.call(&bench.lower, "fast", &lower_revision).unwrap();
            assert_eq!(answer["stage"]["status"], "sealed", "{answer}");
            assert_eq!(answer["stage"]["downstream"], json!([]));
            assert_eq!(bench.starts(LOWER), 1, "no call was spent on a stale");
            assert_eq!(bench.saved(LOWER_OUTPUT).as_deref(), Some(VERSION_ONE));
            assert!(answer.get("diagnostics").is_none(), "{answer}");

            // The process got the upstream's bytes as adopted, and not the model's revision.
            let request = request_of(&bench, LOWER);
            assert_eq!(request["arguments"], json!({"mode": "fast"}));
            assert_eq!(request["inputs"][0]["path"], UPPER_OUTPUT);
            assert_eq!(
                request["inputs"][0]["bytes_base64"],
                encode_base64(VERSION_ONE)
            );
            assert_eq!(request["inputs"][1]["path"], LOWER_FIXED);

            let verdict = {
                let bytes = WorkspaceBytes::new(&bench.workspace);
                bench.ledger().lock().unwrap().verdict(&bytes)
            };
            assert!(verdict.complete());
        }

        #[test]
        fn a_stage_below_cannot_start_before_the_one_above_it_is_sealed() {
            let bench = stage_bench();
            let answer = bench.call(&bench.lower, "fast", "initial").unwrap();
            assert_eq!(answer["status"], "not_ready", "{answer}");
            assert_eq!(
                answer["waiting_on"],
                json!([{"stage": "upper", "target": "main"}])
            );
            assert_eq!(bench.starts(LOWER), 0);
            assert_eq!(bench.attempts(&lower_key()), 0);
            assert_eq!(bench.saved(LOWER_OUTPUT), None);
        }

        /// The revision a sealed call named and the one its repeat names differ, so a digest that
        /// held the revision would never match and this row could never be reached.
        #[test]
        fn row_one_the_same_call_on_the_sealed_bytes_changes_nothing_and_starts_nothing() {
            let bench = stage_bench();
            let sealed_with = "initial";
            let answer = bench.seal_upper();
            let current = answer["stage"]["revision"].as_str().unwrap().to_string();
            assert_ne!(current, sealed_with);
            let before = bench.status(&upper_key());
            assert_eq!(bench.starts(UPPER), 1);

            let again = bench.call(&bench.upper, "fast", &current).unwrap();
            assert_eq!(again, json!({"status": "no_change", "revision": current}));
            assert_eq!(bench.starts(UPPER), 1, "no process");
            assert_eq!(bench.attempts(&upper_key()), 1, "no attempt");
            assert_eq!(bench.status(&upper_key()), before);
        }

        #[test]
        fn the_digest_of_a_call_leaves_out_the_revision_and_nothing_else() {
            let call = |mode: &str, revision: &str| {
                arguments_sha256(&arguments_without_revision(&json!({
                    "mode": mode,
                    EXPECTED_REVISION_ARGUMENT: revision,
                })))
            };
            assert_eq!(call("fast", "initial"), call("fast", "g1-0123456789abcdef"));
            assert_ne!(call("fast", "initial"), call("slow", "initial"));
            assert_eq!(
                arguments_without_revision(&json!({
                    "mode": "fast",
                    EXPECTED_REVISION_ARGUMENT: "initial",
                })),
                json!({"mode": "fast"})
            );
        }

        #[test]
        fn row_two_another_call_is_a_new_version_that_takes_the_stage_below_with_it() {
            let bench = stage_bench();
            let first = bench.seal_upper();
            let lower_revision = downstream_revision(&first);
            bench.call(&bench.lower, "fast", &lower_revision).unwrap();
            let upper_before = bench.status(&upper_key()).1;
            let lower_before = bench.status(&lower_key()).1;

            bench.mark(&format!("second-{UPPER}"), "");
            let current = first["stage"]["revision"].as_str().unwrap();
            let second = bench.call(&bench.upper, "slow", current).unwrap();
            assert_eq!(second["stage"]["status"], "sealed", "{second}");
            assert_eq!(bench.status(&upper_key()).1, upper_before + 1);
            let (phase, generation, _) = bench.status(&lower_key());
            assert_eq!(phase, Phase::NotReady);
            assert_eq!(generation, lower_before + 1);
            assert_eq!(bench.saved(UPPER_OUTPUT).as_deref(), Some(VERSION_TWO));

            // The reader took up the new version from the receipt: this is what a run ended on
            // with `input_revision_mismatch` before.
            let new_revision = downstream_revision(&second);
            assert_ne!(new_revision, lower_revision);
            let answer = bench.call(&bench.lower, "fast", &new_revision).unwrap();
            assert_eq!(answer["stage"]["status"], "sealed", "{answer}");
            assert_eq!(
                request_of(&bench, LOWER)["inputs"][0]["bytes_base64"],
                encode_base64(VERSION_TWO)
            );
        }

        #[test]
        fn row_three_an_older_revision_is_stale_and_row_four_an_unknown_one_is_refused() {
            let bench = stage_bench();
            let answer = bench.seal_upper();
            let current = answer["stage"]["revision"].as_str().unwrap().to_string();

            let stale = bench.call(&bench.upper, "other", "initial").unwrap();
            assert_eq!(stale, json!({"status": "stale", "revision": current}));
            assert_eq!(bench.starts(UPPER), 1);
            assert_eq!(bench.attempts(&upper_key()), 1);

            for revision in ["g9-0123456789abcdef", "made up", ""] {
                let unknown = bench.call(&bench.upper, "other", revision).unwrap();
                assert_eq!(unknown["status"], "unknown_revision", "{revision}");
                assert_eq!(bench.starts(UPPER), 1, "{revision}");
                assert_eq!(bench.attempts(&upper_key()), 1, "{revision}");
            }
        }

        #[test]
        fn each_failed_check_can_be_answered_and_the_next_submission_seals() {
            for (status, code) in [
                ("failed", "check_failed"),
                ("not_run", "check_not_run"),
                ("unknown", "check_unknown"),
            ] {
                let bench = stage_bench();
                bench.mark(
                    &format!("checks-{UPPER}"),
                    &format!(r#"[{{"id":"shape","status":"{status}"}}]"#),
                );
                let answer = bench.call(&bench.upper, "fast", "initial").unwrap();
                assert_eq!(answer["status"], "resubmit", "{answer}");
                assert_eq!(answer["code"], code, "{answer}");
                assert_eq!(bench.saved(UPPER_OUTPUT), None, "{status}");
                assert_eq!(bench.status(&upper_key()).0, Phase::Proposed, "{status}");
                assert_eq!(bench.attempts(&upper_key()), 1, "{status}");

                bench.unmark(&format!("checks-{UPPER}"));
                let answer = bench.call(&bench.upper, "fast", "initial").unwrap();
                assert_eq!(answer["stage"]["status"], "sealed", "{status}: {answer}");
                assert_eq!(bench.starts(UPPER), 2, "{status}");
                assert_eq!(bench.attempts(&upper_key()), 2, "{status}");
            }
        }

        /// The operator's IDs decide: a passing report of something else is not a pass.
        #[test]
        fn a_passing_report_that_is_not_the_required_ids_seals_nothing() {
            for (checks, code) in [
                (r#"[{"id":"other","status":"passed"}]"#, "check_unknown"),
                (
                    r#"[{"id":"shape","status":"passed"},{"id":"other","status":"passed"}]"#,
                    "check_unknown",
                ),
            ] {
                let bench = stage_bench();
                bench.mark(&format!("checks-{UPPER}"), checks);
                let answer = bench.call(&bench.upper, "fast", "initial").unwrap();
                assert_eq!(answer["code"], code, "{checks}");
                assert_eq!(bench.saved(UPPER_OUTPUT), None, "{checks}");
            }
        }

        #[test]
        fn a_failed_check_that_said_too_much_comes_back_cut_as_data_and_never_in_a_message() {
            let bench = stage_bench();
            bench.mark(
                &format!("checks-{UPPER}"),
                r#"[{"id":"shape","status":"failed"}]"#,
            );
            let items = (0..9)
                .map(|index| json!({"code": format!("c{index}"), "message": "said a plain word"}))
                .collect::<Vec<_>>();
            bench.mark(
                &format!("diagnostics-{UPPER}"),
                &Value::Array(items).to_string(),
            );
            let answer = bench.call(&bench.upper, "fast", "initial").unwrap();
            assert_eq!(answer["status"], "resubmit");
            assert_eq!(answer["diagnostics"].as_array().unwrap().len(), 8);
            assert_eq!(answer["truncated"], true);
            assert_eq!(answer["diagnostics"][7]["code"], "c7");
            assert!(answer["code"].as_str().unwrap().starts_with("check_"));
        }

        /// The history cuts any other tool's result at 500 characters, wherever that falls.
        #[test]
        fn an_answer_reaches_the_model_whole_at_the_edge_of_what_it_keeps() {
            let answer = |message: String| {
                json!({"status": "resubmit", "code": "c", "diagnostics": [
                    {"code": "n", "message": message},
                ], "truncated": false})
            };
            let overhead = answer(String::new()).to_string().chars().count();
            let at_edge = "m".repeat(MODEL_RESULT_CHARS - overhead);
            let fixed = json!({"status": "resubmit", "code": "c"});
            let whole = fit_for_model(answer(at_edge.clone()), "/diagnostics", fixed.clone());
            assert_eq!(whole.chars().count(), MODEL_RESULT_CHARS);
            assert!(whole.contains(&at_edge));
            assert!(whole.contains(r#""truncated":false"#), "{whole}");
            assert_eq!(crate::eval_events::body_snippet_whole_tokens(&whole), whole);

            let over = fit_for_model(answer(format!("{at_edge}m")), "/diagnostics", fixed);
            assert!(over.chars().count() <= MODEL_RESULT_CHARS);
            let read: Value = serde_json::from_str(&over).unwrap();
            assert_eq!(read["truncated"], true);
            assert!(
                !read["diagnostics"][0]["message"]
                    .as_str()
                    .unwrap()
                    .is_empty()
            );
        }

        #[test]
        fn a_list_too_long_for_the_model_gives_way_from_its_end_and_says_so() {
            let bench = stage_bench();
            bench.mark(
                &format!("checks-{UPPER}"),
                r#"[{"id":"shape","status":"failed"}]"#,
            );
            let items = (0..8)
                .map(|index| json!({"code": format!("c{index}"), "message": "m".repeat(512)}))
                .collect::<Vec<_>>();
            bench.mark(
                &format!("diagnostics-{UPPER}"),
                &Value::Array(items).to_string(),
            );
            let text = bench
                .upper
                .execute(
                    &json!({"mode": "fast", EXPECTED_REVISION_ARGUMENT: "initial"}),
                    &bench.context(),
                    &|| false,
                )
                .unwrap();
            assert!(text.chars().count() <= MODEL_RESULT_CHARS, "{text}");
            assert_eq!(crate::eval_events::body_snippet_whole_tokens(&text), text);
            let read: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(read["truncated"], true);
            assert_eq!(read["diagnostics"][0]["code"], "c0", "the first ones stay");

            let many = (0..40)
                .map(|index| json!({"stage": format!("stage-{index}"), "target": "main"}))
                .collect::<Vec<_>>();
            let fitted = fit_for_model(
                json!({"waiting_on": many}),
                "/waiting_on",
                json!({"status": "not_ready"}),
            );
            assert!(fitted.chars().count() <= MODEL_RESULT_CHARS);
            let read: Value = serde_json::from_str(&fitted).unwrap();
            assert_eq!(read["truncated"], true);
            assert_eq!(read["waiting_on"][0]["stage"], "stage-0");
        }

        /// A failed check with the dummy in what it says is not a failed check. It ends the run
        /// like any secret in an output, or a model could probe a value against the comparison.
        #[test]
        fn a_secret_in_what_a_failed_check_says_ends_the_run_and_shows_nothing() {
            let bench = stage_bench();
            let mut catalog = crate::sensitive_data::SecretCatalog::new();
            catalog.register_credential("DUMMY_API_KEY", DUMMY).unwrap();
            crate::sensitive_data::install_scope(catalog, Some(&bench.workspace), None);
            bench.mark(
                &format!("checks-{UPPER}"),
                r#"[{"id":"shape","status":"failed"}]"#,
            );
            bench.mark(
                &format!("diagnostics-{UPPER}"),
                &json!([{"code": "note", "message": DUMMY}]).to_string(),
            );
            let error = bench.call(&bench.upper, "fast", "initial").unwrap_err();
            assert_eq!(kind(&error), RefusalKind::ResultRejected);
            let message = format!("{error:#}");
            assert!(message.contains("secret_value_in_output"), "{message}");
            assert!(!message.contains(DUMMY), "{message}");
            assert_eq!(bench.saved(UPPER_OUTPUT), None);
            assert_eq!(bench.attempts(&upper_key()), 1);
        }

        /// A failed check with nothing of a secret in it goes back with what it said.
        #[test]
        fn what_a_failed_check_says_goes_back_when_no_secret_is_in_it() {
            let bench = stage_bench();
            let mut catalog = crate::sensitive_data::SecretCatalog::new();
            catalog.register_credential("DUMMY_API_KEY", DUMMY).unwrap();
            crate::sensitive_data::install_scope(catalog, Some(&bench.workspace), None);
            bench.mark(
                &format!("checks-{UPPER}"),
                r#"[{"id":"shape","status":"failed"}]"#,
            );
            bench.mark(
                &format!("diagnostics-{UPPER}"),
                &json!([{"code": "note", "message": "said a plain word"}]).to_string(),
            );
            let answer = bench.call(&bench.upper, "fast", "initial").unwrap();
            assert_eq!(answer["status"], "resubmit", "{answer}");
            assert_eq!(answer["diagnostics"][0]["message"], "said a plain word");
            assert!(!answer.to_string().contains(DUMMY), "{answer}");
        }

        /// One secret in an output ends this run for every later way of asking. Comparing again
        /// and again would make the comparison an oracle for the value.
        #[test]
        fn a_secret_in_an_output_ends_the_run_for_every_later_call_and_only_a_fresh_one_goes_on() {
            let bench = stage_bench();
            let mut catalog = crate::sensitive_data::SecretCatalog::new();
            catalog.register_credential("DUMMY_API_KEY", DUMMY).unwrap();
            crate::sensitive_data::install_scope(catalog, Some(&bench.workspace), None);
            bench.mark(
                &format!("diagnostics-{UPPER}"),
                &json!([{"code": "note", "message": DUMMY}]).to_string(),
            );
            let error = bench.call(&bench.upper, "fast", "initial").unwrap_err();
            assert!(error.to_string().contains("secret_value_in_output"));
            bench.unmark(&format!("diagnostics-{UPPER}"));
            let refused = |bench: &StageBench, label: &str| {
                let error = bench
                    .call(&bench.upper, "fast", "initial")
                    .expect_err(label);
                assert_eq!(kind(&error), RefusalKind::StageRunEnded, "{label}");
                assert!(
                    error.to_string().contains("ended_by_secret_value"),
                    "{label}: {error:#}"
                );
                assert_eq!(bench.starts(UPPER), 1, "{label}: no process");
            };

            refused(&bench, "the next command");
            bench.next_session(LedgerEntry::Fresh);
            refused(&bench, "another session");
            let bench = bench.restarted(LedgerEntry::Resume);
            refused(&bench, "a resume in another process");

            let bench = bench.restarted(LedgerEntry::FreshSession);
            let answer = bench.call(&bench.upper, "fast", "initial").unwrap();
            assert_eq!(answer["stage"]["status"], "sealed", "{answer}");
        }

        /// P2: a process that was only started does not take the mark away. Comparing the same
        /// value once per process would be the same oracle, a little slower.
        #[test]
        fn a_new_process_that_did_not_ask_cannot_replace_a_record_a_secret_ended() {
            let bench = stage_bench();
            let mut catalog = crate::sensitive_data::SecretCatalog::new();
            catalog.register_credential("DUMMY_API_KEY", DUMMY).unwrap();
            crate::sensitive_data::install_scope(catalog, Some(&bench.workspace), None);
            bench.mark(
                &format!("diagnostics-{UPPER}"),
                &json!([{"code": "note", "message": DUMMY}]).to_string(),
            );
            bench.call(&bench.upper, "fast", "initial").unwrap_err();
            bench.unmark(&format!("diagnostics-{UPPER}"));
            let file = crate::planner::structured_stages::ledger::ledger_file(
                &bench.workspace.parent().unwrap().join("state"),
                &bench.workspace,
                PROFILE,
            )
            .unwrap();
            let record = std::fs::read(&file).unwrap();

            let bench = bench.restarted(LedgerEntry::Fresh);
            let error = bench.call(&bench.upper, "fast", "initial").unwrap_err();
            assert_eq!(kind(&error), RefusalKind::StageLedgerRefused, "{error:#}");
            assert_eq!(bench.starts(UPPER), 1, "nothing ran");
            assert_eq!(std::fs::read(&file).unwrap(), record, "nothing replaced");
            let message = error.to_string();
            assert!(message.contains("ended_by_secret_value"), "{message}");
            assert!(message.contains("`--fresh-session`"), "{message}");

            let bench = bench.restarted(LedgerEntry::FreshSession);
            let answer = bench.call(&bench.upper, "fast", "initial").unwrap();
            assert_eq!(answer["stage"]["status"], "sealed", "{answer}");
        }

        /// A new REPL that says `/resume` goes on with the run. The same process's own `/resume`
        /// holds the ledger it always held, so nothing about it changes.
        #[test]
        fn a_restarted_process_that_resumes_keeps_its_counts_and_seals_nothing() {
            let bench = stage_bench();
            let answer = bench.seal_upper();
            let current = answer["stage"]["revision"].as_str().unwrap().to_string();
            let (_, generation, _) = bench.status(&upper_key());

            // The same process.
            bench.next_session(LedgerEntry::Resume);
            assert_eq!(bench.status(&upper_key()).0, Phase::Sealed);
            assert_eq!(bench.attempts(&upper_key()), 1);
            assert_eq!(
                bench.call(&bench.upper, "fast", &current).unwrap()["status"],
                "no_change"
            );

            let mut config = super::super::config_for_test(&bench.workspace);
            config.resume = Some("a-saved-run".to_string());
            let bench = bench.restarted(LedgerEntry::of_config(&config));
            let (phase, moved, reason) = bench.status(&upper_key());
            assert_eq!(phase, Phase::Unverified);
            assert_eq!(reason, Some(UnverifiedReason::Resumed));
            assert_eq!(moved, generation);
            assert_eq!(bench.attempts(&upper_key()), 1, "no attempt comes back");

            let answer = bench.call(&bench.upper, "fast", &current).unwrap();
            assert_eq!(answer["stage"]["status"], "sealed", "{answer}");
            assert_eq!(bench.attempts(&upper_key()), 2);
        }

        fn in_flight_context(bench: &StageBench) -> (HeldLedger, Arc<StageContract>) {
            bench
                .run
                .open_ledger(
                    super::super::now_ms(),
                    &WorkspaceBytes::new(&bench.workspace),
                )
                .unwrap()
        }

        fn kept_by_history(text: &str) -> Value {
            let kept = crate::eval_events::body_snippet_whole_tokens(text);
            assert!(kept.chars().count() <= MODEL_RESULT_CHARS, "{kept}");
            serde_json::from_str(&kept).unwrap_or_else(|_| panic!("not data: {kept}"))
        }

        /// What the history does to a value can turn the whole answer into one marker, lengthen
        /// it past what it keeps, or leave no room. What it keeps must still say what happened.
        #[test]
        fn what_the_history_keeps_of_a_failed_check_still_says_what_it_is() {
            let bench = stage_bench();
            let answer = |code: &str, message: String, truncated: bool| {
                let mut notes = RunNotes::default();
                bench.upper.answer_failed_check(
                    &bench.context(),
                    CheckFailureKind::Failed,
                    BoundedDiagnostics {
                        items: vec![Diagnostic {
                            code: code.to_string(),
                            message,
                        }],
                        truncated,
                    },
                    &mut notes,
                )
            };
            let overhead = answer("n", String::new(), false).chars().count();
            let room = MODEL_RESULT_CHARS - overhead;
            for (label, text) in [
                (
                    "a credential word in a code",
                    answer("missing_api_key", "m".to_string(), false),
                ),
                (
                    "one in a message",
                    answer("n", "needsapi_keyhere".to_string(), false),
                ),
                (
                    "one among other words",
                    answer("n", "the api_key is not here".to_string(), false),
                ),
                (
                    "a home path up to the edge",
                    answer("n", "/home/ab/".repeat(room / 9), false),
                ),
            ] {
                let read = kept_by_history(&text);
                assert_eq!(read["status"], "resubmit", "{label}: {text}");
                assert_eq!(read["code"], "check_failed", "{label}: {text}");
                assert!(read.get("truncated").is_some(), "{label}: {text}");
            }
            // Nothing the history changes, and nothing is cut: nothing says it was.
            let read = kept_by_history(&answer("n", "said a plain word".to_string(), false));
            assert_eq!(read["truncated"], false);
            assert_eq!(read["diagnostics"][0]["message"], "said a plain word");
            // A cut that came before this function is still reported.
            let read = kept_by_history(&answer("n", "said a plain word".to_string(), true));
            assert_eq!(read["truncated"], true);
            // A whole answer lost to the history reports the cut too.
            let read = kept_by_history(&answer("missing_api_key", "m".to_string(), false));
            assert_eq!(read["truncated"], true);
        }

        fn sealed_receipt(path: &str) -> (CommitReceipt, CommitDecision) {
            let receipt = CommitReceipt {
                protocol: "profile-tool.commit-receipt/v1".to_string(),
                request_id: "request-history".to_string(),
                tool_id: "tool-history".to_string(),
                output_id: "artifact".to_string(),
                path: path.to_string(),
                committed: true,
                durable: true,
                cancelled_after_commit: false,
                runtime_sha256: "a".repeat(64),
                input_revision: "b".repeat(64),
                schema_sha256: "c".repeat(64),
                result_sha256: "d".repeat(64),
                committed_sha256: "e".repeat(64),
                committed_bytes: 1,
                diagnostics: Vec::new(),
            };
            let decision = CommitDecision::Sealed {
                revision: "g1-0123456789abcdef".to_string(),
                receipt_sha256: "f".repeat(64),
                invalidated: Vec::new(),
            };
            (receipt, decision)
        }

        #[test]
        fn what_the_history_keeps_of_a_commit_still_says_it_sealed_and_at_which_revision() {
            for path in [
                "config/api_key.json".to_string(),
                format!("{}b", "a/".repeat(400)),
            ] {
                let (receipt, decision) = sealed_receipt(&path);
                let text = super::super::stage_receipt_for_model(&receipt, &decision);
                let read = kept_by_history(&text);
                assert_eq!(read["committed"], true, "{path}: {text}");
                assert_eq!(read["stage"]["status"], "sealed", "{path}: {text}");
                assert_eq!(
                    read["stage"]["revision"], "g1-0123456789abcdef",
                    "{path}: {text}"
                );
                assert_eq!(read["truncated"], true, "{path}: {text}");
                assert!(
                    !text.contains(&path),
                    "the operator's value is gone: {text}"
                );
            }
            let (receipt, decision) = sealed_receipt("out/plain.json");
            let read = kept_by_history(&super::super::stage_receipt_for_model(&receipt, &decision));
            assert_eq!(read["path"], "out/plain.json");
            assert!(read.get("truncated").is_none_or(|cut| cut == false));
        }

        #[test]
        fn what_the_history_keeps_of_any_answer_with_no_process_still_says_what_it_is() {
            let bench = stage_bench();
            let (ledger, contract) = in_flight_context(&bench);
            let target = contract.target(&upper_key()).unwrap();
            let long = format!("{}b", "a/".repeat(400));
            for (decision, status) in [
                (
                    BeginDecision::InputNotCurrent { path: long.clone() },
                    "input_not_current",
                ),
                (
                    BeginDecision::InputNotCurrent {
                        path: "config/api_key.json".to_string(),
                    },
                    "input_not_current",
                ),
                (
                    BeginDecision::NotReady {
                        waiting_on: vec![TargetKey {
                            stage: "api_keys".to_string(),
                            target: "main".to_string(),
                        }],
                    },
                    "not_ready",
                ),
            ] {
                let mut notes = RunNotes::default();
                let text = bench
                    .upper
                    .answer_without_process(decision, &ledger, target, &mut notes)
                    .unwrap();
                let read = kept_by_history(&text);
                assert_eq!(read["status"], status, "{text}");
                assert_eq!(read["truncated"], true, "{text}");
            }
        }

        /// A path is the operator's value, and the classifier reads words. Neither reaches it.
        #[test]
        fn a_path_in_a_refusal_cannot_make_it_look_recoverable() {
            let bench = stage_bench();
            let (ledger, contract) = in_flight_context(&bench);
            let target = contract.target(&upper_key()).unwrap();
            for path in [
                "fixed/tool_args_path_malformed.json",
                "fixed/edit_noop.json",
                "fixed/path_not_found_recoverable.json",
                "fixed/command_timeout/is a directory.json",
            ] {
                let mut notes = RunNotes::default();
                let ended = bench
                    .upper
                    .answer_without_process(
                        BeginDecision::FixedInputChanged {
                            path: path.to_string(),
                        },
                        &ledger,
                        target,
                        &mut notes,
                    )
                    .unwrap_err();
                let on_ledger = crate::planner::structured_stages::ledger_refused(
                    LedgerRefusal::FixedInputChanged {
                        path: path.to_string(),
                    },
                );
                for refusal in [ended, on_ledger] {
                    assert!(!refusal.reason.contains(path), "{refusal}");
                    let error = anyhow::Error::new(refusal);
                    assert_eq!(
                        crate::tools::registry::tool_error_kind(&error),
                        "tool_execution_error",
                        "{path}: {error:#}"
                    );
                    assert!(!crate::tools::registry::recoverable_tool_error(&error));
                }
            }
        }

        #[test]
        fn a_result_that_does_not_bind_is_not_something_to_resubmit() {
            let bench = stage_bench();
            bench.mark(&format!("checks-{UPPER}"), r#"[{"id":"shape"}]"#);
            let error = bench.call(&bench.upper, "fast", "initial").unwrap_err();
            assert_eq!(kind(&error), RefusalKind::ResultRejected);
            assert!(error.to_string().contains("checks_invalid"), "{error:#}");
        }

        #[test]
        fn submissions_stop_at_the_operators_limit_with_no_process_past_it() {
            let bench = stage_bench_with(2);
            bench.mark(
                &format!("checks-{UPPER}"),
                r#"[{"id":"shape","status":"failed"}]"#,
            );
            for _ in 0..2 {
                let answer = bench.call(&bench.upper, "fast", "initial").unwrap();
                assert_eq!(answer["status"], "resubmit");
            }
            let error = bench.call(&bench.upper, "fast", "initial").unwrap_err();
            assert_eq!(kind(&error), RefusalKind::StageRunEnded);
            assert!(
                error.to_string().contains("attempts_exhausted_for_target"),
                "{error:#}"
            );
            assert_eq!(bench.starts(UPPER), 2);
        }

        #[test]
        fn a_fixed_input_changed_during_the_run_still_ends_it_at_the_pins() {
            let bench = stage_bench();
            bench.seal_upper();
            std::fs::write(bench.workspace.join(LOWER_FIXED), b"rewritten").unwrap();
            let revision = bench.revision(&lower_key());
            let error = bench.call(&bench.lower, "fast", &revision).unwrap_err();
            assert_eq!(
                kind(&error),
                RefusalKind::InputRevisionMismatch,
                "{error:#}"
            );
            assert_eq!(bench.starts(LOWER), 0);
        }

        #[test]
        fn the_first_call_pins_every_fixed_input_on_disk_for_a_later_process() {
            let bench = stage_bench();
            let answer = bench.call(&bench.lower, "fast", "initial").unwrap();
            assert_eq!(answer["status"], "not_ready");
            let directory = std::fs::read_dir(bench.workspace.parent().unwrap().join("state"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let ledger_dir = std::fs::read_dir(directory)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let saved = std::fs::read_to_string(ledger_dir.join("ledger.json")).unwrap();
            assert!(saved.contains(LOWER_FIXED), "the target below is pinned");
            assert!(saved.contains(UPPER_FIXED));
        }

        #[test]
        fn a_commit_stopped_before_the_rename_leaves_the_generation_where_it_was() {
            let bench = stage_bench();
            let answer = bench.seal_upper();
            let current = answer["stage"]["revision"].as_str().unwrap().to_string();
            let lower_before = bench.status(&lower_key()).1;
            let generation = bench.status(&upper_key()).1;
            bench.mark(&format!("second-{UPPER}"), "");

            let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let raised = cancel.clone();
            let _reset = test_hook::install(move |stage| {
                if stage == Stage::BeforeRename {
                    raised.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            });
            let error = bench
                .call_cancelling(&bench.upper, "slow", &current, &|| {
                    cancel.load(std::sync::atomic::Ordering::SeqCst)
                })
                .unwrap_err();
            assert_eq!(kind(&error), RefusalKind::CommitFailed, "{error:#}");
            assert_eq!(bench.saved(UPPER_OUTPUT).as_deref(), Some(VERSION_ONE));
            let (phase, moved, _) = bench.status(&upper_key());
            assert_eq!(moved, generation, "the old bytes are still there");
            assert_eq!(phase, Phase::Sealed);
            assert_eq!(bench.status(&lower_key()).1, lower_before);
            // The attempt is over, so the next call is not refused for an open intent.
            assert!(bench.call(&bench.upper, "slow", &current).is_ok());
        }

        #[test]
        fn a_commit_that_failed_with_other_bytes_there_counts_as_moved() {
            let bench = stage_bench();
            let generation = bench.status(&upper_key()).1;
            let destination = bench.workspace.join(UPPER_OUTPUT);
            let _reset = test_hook::install(move |stage| {
                if stage == Stage::AfterRename {
                    std::fs::write(&destination, b"replaced in that instant").unwrap();
                }
            });
            let error = bench.call(&bench.upper, "fast", "initial").unwrap_err();
            assert_eq!(kind(&error), RefusalKind::CommitFailed, "{error:#}");
            let (phase, moved, reason) = bench.status(&upper_key());
            assert_eq!(phase, Phase::Unverified);
            assert_eq!(moved, generation + 1);
            assert_eq!(reason, Some(UnverifiedReason::CommitOutcomeUnknown));
        }

        #[test]
        fn a_commit_that_stands_but_is_not_durable_is_no_seal_and_the_reader_stays_waiting() {
            let bench = stage_bench();
            test_hook::fail_directory_sync();
            let _reset = test_hook::install(|_| {});
            let answer = bench.call(&bench.upper, "fast", "initial").unwrap();
            assert_eq!(answer["stage"]["status"], "unverified", "{answer}");
            assert_eq!(answer["stage"]["reason"], "commit_not_durable", "{answer}");
            let revision = downstream_revision(&answer);
            let waiting = bench.call(&bench.lower, "fast", &revision).unwrap();
            assert_eq!(waiting["status"], "not_ready", "{waiting}");
        }

        #[test]
        fn output_bytes_edited_behind_the_host_take_the_state_before_the_next_call() {
            let bench = stage_bench();
            let answer = bench.seal_upper();
            let lower_revision = downstream_revision(&answer);
            std::fs::write(bench.workspace.join(UPPER_OUTPUT), b"edited by a command").unwrap();
            let moved = bench.call(&bench.lower, "fast", &lower_revision).unwrap();
            assert_eq!(moved["status"], "stale", "{moved}");
            let current = moved["revision"].as_str().unwrap();
            let waiting = bench.call(&bench.lower, "fast", current).unwrap();
            assert_eq!(waiting["status"], "not_ready", "{waiting}");
            assert_eq!(bench.starts(LOWER), 0);
        }

        #[test]
        fn a_session_that_cannot_answer_for_its_run_gets_no_stages() {
            let bench = stage_bench();
            let state_dir = bench.workspace.parent().unwrap().join("state");
            let refused = |facts| {
                bench
                    .run
                    .bind_session(facts, &state_dir, LedgerEntry::Fresh)
                    .unwrap_err()
                    .kind
            };
            assert_eq!(
                refused(StageRunFacts::for_test(false, false)),
                RefusalKind::StageContractRefused
            );
            assert_eq!(
                refused(StageRunFacts::for_test(true, true)),
                RefusalKind::StageContractRefused
            );
            let inside = bench.workspace.join(".state");
            assert_eq!(
                bench
                    .run
                    .bind_session(
                        StageRunFacts::for_test(true, false),
                        &inside,
                        LedgerEntry::Fresh
                    )
                    .unwrap_err()
                    .kind,
                RefusalKind::StageLedgerRefused
            );
        }

        /// Neither spends an attempt, and the first does not even open the ledger.
        #[test]
        fn a_cancel_or_an_unreadable_secret_before_the_start_spends_no_attempt() {
            let bench = stage_bench();
            let error = bench
                .call_cancelling(&bench.upper, "fast", "initial", &|| true)
                .unwrap_err();
            assert_eq!(kind(&error), RefusalKind::ProcessFailed, "{error:#}");
            assert_eq!(bench.starts(UPPER), 0);
            assert!(
                !bench.workspace.parent().unwrap().join("state").exists(),
                "a cancel opens nothing"
            );

            std::fs::write(bench.workspace.join(".env"), "DUMMY_API_KEY=Zq7_x3\n").unwrap();
            let error = bench.call(&bench.upper, "fast", "initial").unwrap_err();
            assert_eq!(kind(&error), RefusalKind::EffectEnvRefused, "{error:#}");
            assert_eq!(bench.starts(UPPER), 0);
            std::fs::remove_file(bench.workspace.join(".env")).unwrap();
            assert_eq!(bench.attempts(&upper_key()), 0);
        }

        fn funnel(bench: &StageBench, scope: StageScope) -> anyhow::Result<&'static str> {
            refuse_unmet_session(
                Ok("All stages are sealed and every check passed."),
                Some(&bench.run),
                scope,
                None,
            )
        }

        fn stage_incomplete(result: anyhow::Result<&'static str>, code: &str) {
            let error = result.expect_err("the run must not be a success");
            let message = error.to_string();
            assert!(
                message.starts_with(&format!("profile_tool_refused: stage_incomplete: {code}")),
                "{message}"
            );
        }

        /// T6: nothing a model says or an answer says reaches the host's decision.
        #[test]
        fn only_the_ledger_decides_whether_a_run_succeeds() {
            // A failed check.
            let bench = stage_bench();
            bench.mark(
                &format!("checks-{UPPER}"),
                r#"[{"id":"shape","status":"failed"}]"#,
            );
            bench.call(&bench.upper, "fast", "initial").unwrap();
            stage_incomplete(funnel(&bench, StageScope::All), "obligations_unmet");

            // The upper sealed, and the lower never submitted.
            let bench = stage_bench();
            let answer = bench.seal_upper();
            stage_incomplete(funnel(&bench, StageScope::All), "obligations_unmet");

            // Everything sealed: the control.
            let revision = downstream_revision(&answer);
            bench.call(&bench.lower, "fast", &revision).unwrap();
            assert!(funnel(&bench, StageScope::All).is_ok());

            // Then the bytes move behind the host.
            std::fs::write(bench.workspace.join(LOWER_OUTPUT), b"edited").unwrap();
            stage_incomplete(funnel(&bench, StageScope::All), "obligations_unmet");
            assert_eq!(bench.status(&lower_key()).0, Phase::Unverified);
        }

        #[test]
        fn a_commit_that_stood_unverified_leaves_the_run_unsuccessful() {
            let bench = stage_bench();
            test_hook::fail_directory_sync();
            let _reset = test_hook::install(|_| {});
            let answer = bench.call(&bench.upper, "fast", "initial").unwrap();
            assert_eq!(answer["stage"]["status"], "unverified", "{answer}");
            assert!(bench.saved(UPPER_OUTPUT).is_some(), "the bytes moved");
            stage_incomplete(funnel(&bench, StageScope::All), "obligations_unmet");
            stage_incomplete(funnel(&bench, StageScope::Touched), "obligations_unmet");
            let notes = bench.run.take_notes();
            assert!(notes.iter().all(|note| !note.progress), "{notes:?}");
        }

        #[test]
        fn a_record_that_cannot_be_vouched_for_is_a_named_failure_and_not_a_hold() {
            let bench = stage_bench();
            bench.seal_upper();
            let file = crate::planner::structured_stages::ledger::ledger_file(
                &bench.workspace.parent().unwrap().join("state"),
                &bench.workspace,
                PROFILE,
            )
            .unwrap();
            std::fs::write(file, b"not a record").unwrap();
            let shortfall = bench.run.judge(StageScope::All).unwrap();
            assert_eq!(shortfall.code, "ledger_refused");
            assert!(!shortfall.can_be_resubmitted());
            stage_incomplete(funnel(&bench, StageScope::All), "ledger_refused");
        }

        /// N2: the first to find the damage may be a funnel or the final gate, not a call.
        #[test]
        fn a_record_found_broken_by_a_judgment_says_where_it_is_and_what_to_do() {
            let bench = stage_bench();
            bench.seal_upper();
            let state_dir = bench.workspace.parent().unwrap().join("state");
            let file = crate::planner::structured_stages::ledger::ledger_file(
                &state_dir,
                &bench.workspace,
                PROFILE,
            )
            .unwrap();
            std::fs::write(&file, b"not a record").unwrap();
            let shortfall = bench.run.judge(StageScope::All).unwrap();
            let record = shortfall.record.clone().expect("what the refusal carried");
            assert_eq!(record["ledger_path"], file.display().to_string());
            assert!(record["recovery"].as_str().is_some(), "{record}");

            let event = shortfall.event();
            assert_eq!(event["record"], record, "{event}");
            // The message stays a fixed sentence, as in every other refusal of the record.
            let message = shortfall.to_refusal().to_string();
            assert!(
                !message.contains(&state_dir.display().to_string()),
                "{message}"
            );
        }

        /// N7: a repair prompt shows this message, and has to say which one.
        #[test]
        fn the_failure_names_the_stages_it_is_about_and_no_path() {
            let bench = stage_bench();
            let answer = bench.seal_upper();
            let message = bench
                .run
                .judge(StageScope::All)
                .unwrap()
                .to_refusal()
                .to_string();
            assert!(message.contains("lower/main"), "{message}");
            assert!(!message.contains("upper/main"), "sealed: {message}");
            assert!(!message.contains(LOWER_OUTPUT), "{message}");
            assert!(answer["stage"]["status"] == "sealed");
        }

        /// M12: after a secret, every seal in the set does not make a success.
        #[test]
        fn a_secret_after_every_seal_still_leaves_the_run_unsuccessful() {
            let bench = stage_bench();
            let answer = bench.seal_upper();
            let revision = downstream_revision(&answer);
            bench.call(&bench.lower, "fast", &revision).unwrap();
            assert!(funnel(&bench, StageScope::All).is_ok());

            let mut catalog = crate::sensitive_data::SecretCatalog::new();
            catalog.register_credential("DUMMY_API_KEY", DUMMY).unwrap();
            crate::sensitive_data::install_scope(catalog, Some(&bench.workspace), None);
            bench.mark(
                &format!("diagnostics-{UPPER}"),
                &json!([{"code": "note", "message": DUMMY}]).to_string(),
            );
            let current = answer["stage"]["revision"].as_str().unwrap();
            bench.call(&bench.upper, "other", current).unwrap_err();
            stage_incomplete(funnel(&bench, StageScope::All), "obligations_unmet");
        }

        /// T-C (N1): a judgment before the first call is not that call.
        #[test]
        fn asking_whether_the_stages_are_met_does_not_start_the_deadline() {
            let bench = stage_bench();
            assert!(bench.run.judge(StageScope::All).is_some());
            let before = bench.ledger().lock().unwrap().started_at_ms();
            std::thread::sleep(std::time::Duration::from_millis(5));
            bench.seal_upper();
            let after = bench.ledger().lock().unwrap().started_at_ms();
            assert!(after > before, "the first stage call is what starts it");
        }

        #[test]
        fn a_step_that_called_no_stage_opens_nothing_and_answers_for_nothing() {
            let bench = stage_bench();
            assert!(funnel(&bench, StageScope::Touched).is_ok());
            assert!(!bench.workspace.parent().unwrap().join("state").exists());
            // A step that called one answers for that one, and the other is not its to answer.
            bench.seal_upper();
            assert!(funnel(&bench, StageScope::Touched).is_ok());
            stage_incomplete(funnel(&bench, StageScope::All), "obligations_unmet");
        }

        /// S8: a call that starts nothing spends no attempt, so it needs a limit of its own.
        #[test]
        fn repeating_a_stale_call_is_counted_and_ends_the_run_at_its_limit() {
            let bench = stage_bench();
            let answer = bench.seal_upper();
            bench.run.take_notes();
            assert!(
                answer["stage"]["revision"].as_str().unwrap() != "initial",
                "{answer}"
            );
            for repeat in 0..7 {
                let stale = bench.call(&bench.upper, "other", "initial").unwrap();
                assert_eq!(stale["status"], "stale", "{repeat}");
            }
            let notes = bench.run.take_notes();
            assert_eq!(notes.len(), 7);
            assert!(notes.iter().all(|note| !note.progress));

            let error = bench.call(&bench.upper, "other", "initial").unwrap_err();
            assert_eq!(kind(&error), RefusalKind::StageRunEnded);
            assert!(error.to_string().contains("no_progress_limit"), "{error:#}");
            assert_eq!(bench.starts(UPPER), 1, "nothing ran");
        }

        #[test]
        fn a_call_that_starts_a_process_clears_the_count_of_those_that_did_not() {
            let bench = stage_bench();
            for _ in 0..7 {
                let waiting = bench.call(&bench.lower, "fast", "initial").unwrap();
                assert_eq!(waiting["status"], "not_ready");
            }
            let answer = bench.seal_upper();
            assert_eq!(answer["stage"]["status"], "sealed");
            let revision = downstream_revision(&answer);
            for _ in 0..7 {
                let waiting = bench.call(&bench.upper, "other", "initial").unwrap();
                assert_eq!(waiting["status"], "stale");
            }
            let notes_end = bench.call(&bench.lower, "fast", &revision).unwrap();
            assert_eq!(notes_end["stage"]["status"], "sealed", "{notes_end}");
            let notes = bench.run.take_notes();
            assert!(notes.last().is_some_and(|note| note.progress), "{notes:?}");
        }

        /// T2: calls in the reply that committed were written before they knew of it.
        #[test]
        fn a_call_in_the_reply_that_committed_is_stale_and_the_next_reply_goes_through() {
            let bench = stage_bench();
            let _scope = enter_session(bench.state.clone());
            for_provider_worker().unwrap();
            let raw = |revision: &str| {
                json!({"mode": "fast", EXPECTED_REVISION_ARGUMENT: revision}).to_string()
            };
            let run = |tool: &ProposalTool, revision: &str| -> Value {
                let call = receive_native(
                    tool.name(),
                    || "call".to_string(),
                    Some(&Value::String(raw(revision))),
                    true,
                )
                .unwrap();
                let text = tool
                    .execute(&call.arguments, &bench.context(), &|| false)
                    .unwrap();
                serde_json::from_str(&text).unwrap()
            };
            let answer = run(&bench.upper, "initial");
            assert_eq!(answer["stage"]["status"], "sealed", "{answer}");
            let revision = downstream_revision(&answer);

            let same_reply = run(&bench.lower, &revision);
            assert_eq!(same_reply["status"], "stale", "{same_reply}");
            assert_eq!(bench.starts(LOWER), 0);

            for_provider_worker().unwrap();
            let next = run(&bench.lower, &revision);
            assert_eq!(next["stage"]["status"], "sealed", "{next}");
        }

        /// A raw terminal, as a tool's run has one: a line feed moves down and keeps the column,
        /// and only a return goes back to the left edge.
        fn screen_after(writes: &[&str]) -> Vec<String> {
            let (mut rows, mut row, mut column) = (vec![Vec::<char>::new()], 0usize, 0usize);
            for write in writes {
                let mut rest = *write;
                while let Some(c) = rest.chars().next() {
                    if let Some(after) = rest.strip_prefix("\x1b[2K") {
                        rows[row].clear();
                        rest = after;
                        continue;
                    }
                    rest = &rest[c.len_utf8()..];
                    match c {
                        '\r' => column = 0,
                        '\n' => {
                            row += 1;
                            if rows.len() == row {
                                rows.push(Vec::new());
                            }
                        }
                        other => {
                            if rows[row].len() <= column {
                                rows[row].resize(column + 1, ' ');
                            }
                            rows[row][column] = other;
                            column += 1;
                        }
                    }
                }
            }
            rows.into_iter()
                .map(|line| line.into_iter().collect())
                .collect()
        }

        const SPINNER_FRAME: &str = "\r\x1b[2K| tool ProposeUpper 2s";
        const NOTICE: &str = "stage record refused: corrupt\n  ledger: /state/ledger.json";

        /// The spinner leaves its frame with no newline, and a run's terminal is raw: its line
        /// feed returns to no edge. A block written as it stands would start on the frame's line
        /// and its second line would start where the first one ended.
        #[test]
        fn the_notice_starts_on_a_line_of_its_own_and_the_spinner_goes_on_below_it() {
            let block = super::super::stage_refusal_block(NOTICE, true);
            let screen = screen_after(&[SPINNER_FRAME, &block, SPINNER_FRAME]);
            assert_eq!(
                screen,
                [
                    "stage record refused: corrupt",
                    "  ledger: /state/ledger.json",
                    "| tool ProposeUpper 2s",
                ]
            );
            // The model's own check: without the returns, the second line drifts to the right.
            let plain = format!("{NOTICE}\n");
            let broken = screen_after(&[&plain]);
            assert_eq!(
                broken[1],
                format!(
                    "{}  ledger: /state/ledger.json",
                    " ".repeat("stage record refused: corrupt".len())
                )
            );

            // Piped or logged, there is nothing to clear and no control character at all.
            let piped = super::super::stage_refusal_block(NOTICE, false);
            assert_eq!(piped, plain);
        }

        /// A real terminal device set the way a run sets it: what the block's bytes do there, and
        /// not only what the model above takes them to do. No process is started.
        #[test]
        fn the_notice_goes_through_a_raw_pty_with_its_left_edge() {
            use std::io::{Read, Write};
            use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
            let (mut master, mut slave) = (-1, -1);
            // SAFETY: all five are valid for this call, and the three nulls mean no name and
            // default settings.
            let opened = unsafe {
                libc::openpty(
                    &raw mut master,
                    &raw mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(opened, 0, "{}", std::io::Error::last_os_error());
            // SAFETY: `openpty` returned these two and nothing else owns them.
            let master = unsafe { OwnedFd::from_raw_fd(master) };
            // SAFETY: as above.
            let slave = unsafe { OwnedFd::from_raw_fd(slave) };
            // SAFETY: an all-zero `termios` is a valid value for `tcgetattr` to fill in.
            let mut modes = unsafe { std::mem::zeroed::<libc::termios>() };
            // SAFETY: `slave` is open and `modes` is a valid place to write.
            let got = unsafe { libc::tcgetattr(slave.as_raw_fd(), &raw mut modes) };
            assert_eq!(got, 0);
            // SAFETY: `modes` was just filled in.
            unsafe { libc::cfmakeraw(&raw mut modes) };
            // SAFETY: `slave` is open and `modes` is valid.
            let set =
                unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &raw const modes) };
            assert_eq!(set, 0);

            let written = format!(
                "{SPINNER_FRAME}{}",
                super::super::stage_refusal_block(NOTICE, true)
            );
            let mut writer = std::fs::File::from(slave.try_clone().unwrap());
            writer.write_all(written.as_bytes()).unwrap();
            let mut read = vec![0; written.len()];
            std::fs::File::from(master).read_exact(&mut read).unwrap();
            // The slave stays open past the read: closing it first may end the read on some
            // systems before the bytes come out.
            drop(writer);
            drop(slave);
            let screen = screen_after(&[std::str::from_utf8(&read).unwrap()]);
            assert_eq!(screen[0], "stage record refused: corrupt");
            assert_eq!(screen[1], "  ledger: /state/ledger.json");
        }

        /// For the eye, on a real terminal with the footer on: run it under a named `tmux` session
        /// and look at the pane. The block's left edge, the footer's row, and the spinner below it.
        #[test]
        #[ignore = "needs a real terminal and someone to read it"]
        fn the_notice_on_a_real_terminal_with_the_footer_and_the_spinner() {
            use crate::tui::InteractionUi;
            let temp = tempfile::tempdir().unwrap();
            let config = super::super::config_for_test(temp.path());
            let ui = crate::tui::TerminalUi::new(&config);
            let guard = ui.before_tool_call("ProposeUpper");
            std::thread::sleep(std::time::Duration::from_secs(1));
            let block =
                super::super::stage_refusal_block(NOTICE, crate::tui::terminal::stderr_is_tty());
            // The same write as `emit_stage_refusal`.
            let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), block.as_bytes());
            std::thread::sleep(std::time::Duration::from_secs(3));
            drop(guard);
        }

        /// A name is the operator's and a path can hold anything. None of it moves a cursor.
        #[test]
        fn no_control_character_of_a_name_reaches_the_terminal() {
            let data = json!({
                "code": "fixed_input_changed",
                "input": "fixed/\x1b[2J\x1b[1;1Hnew\rline\u{7}.json",
                "ledger_path": "/state/a\nb/ledger.json",
                "recovery": "only `--fresh-session` goes on",
            });
            let block = super::super::stage_refusal_block(
                &super::super::stage_refusal_notice(&data),
                false,
            );
            assert!(!block.contains('\x1b'), "{block:?}");
            assert!(!block.contains('\r'), "{block:?}");
            assert!(!block.contains('\u{7}'), "{block:?}");
            assert!(
                block.contains("changed input: fixed/?[2J?[1;1Hnew?line?.json"),
                "{block:?}"
            );
            assert!(
                block.contains("ledger: /state/a?b/ledger.json"),
                "{block:?}"
            );
            assert_eq!(block.lines().count(), 4, "one line for each: {block:?}");
        }

        /// T9: another call on the same bytes is the whole procedure, not row 1.
        #[test]
        fn another_call_that_gives_the_same_bytes_runs_and_changes_no_generation() {
            let bench = stage_bench();
            let answer = bench.seal_upper();
            let current = answer["stage"]["revision"].as_str().unwrap().to_string();
            let lower_revision = downstream_revision(&answer);
            bench.call(&bench.lower, "fast", &lower_revision).unwrap();
            let upper_before = bench.status(&upper_key());
            let lower_before = bench.status(&lower_key());

            let again = bench.call(&bench.upper, "slow", &current).unwrap();
            assert_eq!(again, json!({"status": "no_change", "revision": current}));
            assert_eq!(bench.starts(UPPER), 2, "the process ran");
            assert_eq!(bench.attempts(&upper_key()), 2, "the attempt is spent");
            assert_eq!(bench.status(&upper_key()), upper_before);
            assert_eq!(bench.status(&lower_key()), lower_before);
        }

        /// T3: the pin is what G2 adopted, whatever the file says by the first call.
        #[test]
        fn the_pin_of_a_target_that_has_not_run_is_what_was_adopted_and_not_what_is_there_now() {
            let bench = stage_bench();
            std::fs::write(bench.workspace.join(LOWER_FIXED), b"rewritten").unwrap();
            bench.seal_upper();
            let saved = saved_ledger(&bench);
            assert!(saved.contains(&sha256(b"lower seed")), "{saved}");
            assert!(!saved.contains(&sha256(b"rewritten")), "{saved}");
        }

        fn saved_ledger(bench: &StageBench) -> String {
            let (_, contract) = in_flight_context(bench);
            drop(contract);
            let file = crate::planner::structured_stages::ledger::ledger_file(
                &bench.workspace.parent().unwrap().join("state"),
                &bench.workspace,
                PROFILE,
            )
            .unwrap();
            std::fs::read_to_string(file).unwrap()
        }

        /// F11: the contract and the tool come from two reads of one descriptor.
        #[test]
        fn a_tool_that_is_not_what_its_target_was_admitted_as_is_refused() {
            let bench = stage_bench();
            let (_, contract) = in_flight_context(&bench);
            let upper = contract.target(&upper_key()).unwrap();
            let lower = contract.target(&lower_key()).unwrap();
            bench.upper.matches_target(upper).unwrap();
            bench.lower.matches_target(lower).unwrap();
            assert_eq!(
                bench.upper.matches_target(lower).unwrap_err().kind,
                RefusalKind::StageContractRefused
            );
            let mut other_output = upper.clone();
            other_output.output_id = "another".to_string();
            assert!(bench.upper.matches_target(&other_output).is_err());
            let mut other_inputs = upper.clone();
            other_inputs.inputs.clear();
            assert!(bench.upper.matches_target(&other_inputs).is_err());
        }

        /// T10: required, and exactly `type: string`. Anything else leaves a model without a way
        /// to answer a `stale`, or narrows what it may name.
        #[test]
        fn the_revision_argument_is_required_and_exactly_a_string() {
            let schema = |required: &[&str], property: Value| {
                json!({
                    "properties": {EXPECTED_REVISION_ARGUMENT: property},
                    "required": required,
                })
            };
            let plain = json!({"type": "string"});
            assert!(super::super::declares_expected_revision(&schema(
                &[EXPECTED_REVISION_ARGUMENT],
                plain.clone()
            )));
            for (label, bad) in [
                ("not required", schema(&[], plain.clone())),
                (
                    "narrowed",
                    schema(
                        &[EXPECTED_REVISION_ARGUMENT],
                        json!({"type": "string", "enum": ["initial"]}),
                    ),
                ),
                (
                    "another type",
                    schema(&[EXPECTED_REVISION_ARGUMENT], json!({"type": "number"})),
                ),
                (
                    "no property",
                    json!({"required": [EXPECTED_REVISION_ARGUMENT]}),
                ),
            ] {
                assert!(!super::super::declares_expected_revision(&bad), "{label}");
            }
        }

        /// P4, P5: the operator's path and input go to the event and the display, not the message.
        #[test]
        fn a_refused_record_shows_its_path_and_way_out_and_only_data_holds_the_path() {
            let bench = stage_bench();
            let events = bench.marks.join("events.jsonl");
            let mut context = bench.context();
            context.eval_events_path = Some(events.clone());
            let arguments = json!({"mode": "fast", EXPECTED_REVISION_ARGUMENT: "initial"});
            bench
                .upper
                .execute(&arguments, &context, &|| false)
                .unwrap();
            let file = crate::planner::structured_stages::ledger::ledger_file(
                &bench.workspace.parent().unwrap().join("state"),
                &bench.workspace,
                PROFILE,
            )
            .unwrap();
            let bench = bench.restarted(LedgerEntry::Fresh);
            let mut context = bench.context();
            context.eval_events_path = Some(events.clone());
            std::fs::write(&file, b"not a record").unwrap();

            let error = bench
                .upper
                .execute(&arguments, &context, &|| false)
                .unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains("stage_ledger_refused"), "{message}");
            assert!(message.contains("corrupt"), "{message}");
            assert!(!message.contains("ledger.json"), "{message}");
            assert_eq!(
                crate::tools::registry::tool_error_kind(&error),
                "tool_execution_error"
            );
            let text = std::fs::read_to_string(&events).unwrap();
            let line = text
                .lines()
                .find(|line| line.contains("structured_stage_refused"))
                .unwrap_or_else(|| panic!("no event: {text}"));
            let event: Value = serde_json::from_str(line).unwrap();
            assert_eq!(event["code"], "corrupt");
            assert!(
                event["ledger_path"]
                    .as_str()
                    .unwrap()
                    .ends_with("/ledger.json"),
                "{event}"
            );
            assert!(
                event["ledger_path"]
                    .as_str()
                    .unwrap()
                    .contains("structured-stages/"),
                "{event}"
            );
            assert!(event["recovery"].as_str().unwrap().contains("move it away"));
        }

        #[test]
        fn the_notice_names_the_input_the_ledger_and_the_way_out() {
            let refusal = crate::planner::structured_stages::ledger_refused(
                LedgerRefusal::FixedInputChanged {
                    path: "fixed/seed.json".to_string(),
                },
            );
            assert!(refusal.reason.contains("--fresh-session"), "{refusal}");
            let mut data = refusal.data.unwrap();
            data["ledger_path"] = json!("/state/structured-stages/abc/ledger.json");
            let notice = super::super::stage_refusal_notice(&data);
            assert!(notice.contains("fixed_input_changed"), "{notice}");
            assert!(
                notice.contains("changed input: fixed/seed.json"),
                "{notice}"
            );
            assert!(notice.contains("/state/structured-stages/abc/ledger.json"));
            assert!(
                notice.contains(
                    "`--fresh-session`, or any new process that does not resume, starts over"
                ),
                "{notice}"
            );
        }

        /// Every fixed sentence, read the way a later step reads it.
        #[test]
        fn no_refusal_of_the_record_reads_as_something_to_retry() {
            for refusal in [
                LedgerRefusal::Missing,
                LedgerRefusal::Corrupt,
                LedgerRefusal::Held,
                LedgerRefusal::ContractChanged,
                LedgerRefusal::FixedInputChanged {
                    path: "a".to_string(),
                },
                LedgerRefusal::StateDirInsideWorkspace,
                LedgerRefusal::StateDirWritableByCommands,
                LedgerRefusal::Diverged,
                LedgerRefusal::EndedBySecretValue,
                LedgerRefusal::Unavailable,
            ] {
                let error = anyhow::Error::new(crate::planner::structured_stages::ledger_refused(
                    refusal.clone(),
                ));
                assert_eq!(
                    crate::tools::registry::tool_error_kind(&error),
                    "tool_execution_error",
                    "{refusal:?}: {error:#}"
                );
                assert!(!crate::tools::registry::recoverable_tool_error(&error));
            }
        }

        #[test]
        fn a_stage_tool_outside_any_session_is_refused_before_a_ledger_or_a_process() {
            let bench = stage_bench();
            let unbound = Arc::new(StageRun::new(
                StageContractDraft::new(
                    &StageSetDeclaration {
                        stages: vec![stage("upper", UPPER)],
                        limits: NarrowedStageLimits::default(),
                    },
                    &[facts(UPPER, UPPER_OUTPUT, &[UPPER_FIXED], 8)],
                )
                .unwrap(),
                BTreeMap::new(),
                bench.workspace.clone(),
                PROFILE.to_string(),
                Arc::new(LedgerRegistry::default()),
            ));
            let refusal = unbound
                .open_ledger(1, &WorkspaceBytes::new(&bench.workspace))
                .unwrap_err();
            assert_eq!(refusal.kind, RefusalKind::StageContractRefused);
        }

        /// T3 (acceptance 4): a new time on a real file moves nothing and completes nothing.
        #[test]
        fn a_touched_output_moves_nothing_and_completes_nothing() {
            let bench = stage_bench();
            let touch = |path: &str| {
                let file = std::fs::File::options()
                    .write(true)
                    .open(bench.workspace.join(path))
                    .unwrap();
                file.set_modified(
                    std::time::SystemTime::now() + std::time::Duration::from_secs(60),
                )
                .unwrap();
            };
            let answer = bench.seal_upper();
            let before = bench.status(&upper_key());
            touch(UPPER_OUTPUT);
            assert_eq!(bench.status(&upper_key()), before);
            stage_incomplete(funnel(&bench, StageScope::All), "obligations_unmet");

            std::fs::create_dir_all(bench.workspace.join(LOWER_OUTPUT).parent().unwrap()).unwrap();
            std::fs::write(bench.workspace.join(LOWER_OUTPUT), b"made by hand").unwrap();
            touch(LOWER_OUTPUT);
            stage_incomplete(funnel(&bench, StageScope::All), "obligations_unmet");

            std::fs::remove_file(bench.workspace.join(LOWER_OUTPUT)).unwrap();
            bench
                .call(&bench.lower, "fast", &downstream_revision(&answer))
                .unwrap();
            touch(LOWER_OUTPUT);
            assert!(
                funnel(&bench, StageScope::All).is_ok(),
                "the bytes are the same"
            );
        }

        /// T4 (acceptance 2): a new version above leaves the end unsuccessful until the stage
        /// below is checked again.
        #[test]
        fn a_new_upper_version_leaves_the_end_unsuccessful_until_the_lower_is_checked_again() {
            let bench = stage_bench();
            let first = bench.seal_upper();
            bench
                .call(&bench.lower, "fast", &downstream_revision(&first))
                .unwrap();
            assert!(funnel(&bench, StageScope::All).is_ok(), "control");

            bench.mark(&format!("second-{UPPER}"), "");
            let current = first["stage"]["revision"].as_str().unwrap();
            let second = bench.call(&bench.upper, "slow", current).unwrap();
            let error = funnel(&bench, StageScope::All).unwrap_err().to_string();
            assert!(
                error.contains("stage_incomplete: obligations_unmet"),
                "{error}"
            );
            assert!(error.contains("lower/main"), "{error}");

            bench
                .call(&bench.lower, "fast", &downstream_revision(&second))
                .unwrap();
            assert!(funnel(&bench, StageScope::All).is_ok(), "checked again");
        }

        /// Acceptance 5, counted: a launch with no OS limit starts nothing, adopts nothing, and
        /// leaves no receipt that a tool ran or a target's state that one was adopted.
        #[test]
        fn a_tool_with_no_os_limit_leaves_no_receipt() {
            use super::super::super::isolation::test_hook::{DescriptorFault, descriptor_fault};
            let bench = stage_bench();
            {
                let _fault = descriptor_fault(DescriptorFault::MarkRefused);
                let error = bench.call(&bench.upper, "fast", "initial").unwrap_err();
                assert_eq!(kind(&error), RefusalKind::EffectUnavailable, "{error:#}");
            }
            assert_eq!(bench.starts(UPPER), 0);
            assert_eq!(bench.saved(UPPER_OUTPUT), None);
            let ran = bench
                .upper
                .receipts
                .lock()
                .unwrap()
                .iter()
                .filter(|receipt| receipt.executed || receipt.ok)
                .count();
            assert_eq!(ran, 0, "no tool ran");
            assert_eq!(bench.upper.commits.lock().unwrap().len(), 0, "no commit");
            let saved = saved_ledger(&bench);
            assert!(!saved.contains("\"committed_sha256\""), "{saved}");
            assert_ne!(bench.status(&upper_key()).0, Phase::Sealed);
            stage_incomplete(funnel(&bench, StageScope::All), "obligations_unmet");
        }

        /// V3: only a secret's mark makes the record the thing that failed. Any other refusal
        /// keeps its own kind, whatever became of the record.
        #[test]
        fn a_refusal_with_no_secret_keeps_its_kind_when_the_record_cannot_be_written() {
            let bench = stage_bench();
            let file = crate::planner::structured_stages::ledger::ledger_file(
                &bench.workspace.parent().unwrap().join("state"),
                &bench.workspace,
                PROFILE,
            )
            .unwrap();
            bench.mark("clobber", &file.display().to_string());
            bench.mark("broken", "");
            let error = bench.call(&bench.upper, "fast", "initial").unwrap_err();
            assert_eq!(kind(&error), RefusalKind::ResultRejected, "{error:#}");
        }

        /// M18: at the limit, a call that ends the run for its own reason keeps that reason.
        #[test]
        fn at_the_limit_a_call_that_ends_the_run_says_why_it_did() {
            let bench = stage_bench_with(1);
            let answer = bench.seal_upper();
            let current = answer["stage"]["revision"].as_str().unwrap().to_string();
            for _ in 0..7 {
                let stale = bench.call(&bench.upper, "other", "initial").unwrap();
                assert_eq!(stale["status"], "stale");
            }
            let error = bench.call(&bench.upper, "other", &current).unwrap_err();
            assert_eq!(kind(&error), RefusalKind::StageRunEnded, "{error:#}");
            assert!(
                error.to_string().contains("attempts_exhausted_for_target"),
                "{error:#}"
            );
        }

        /// Section 5: a secret's mark that could not be saved must not pass for one that was. A
        /// later process would read the old file and start comparing again.
        #[test]
        fn a_secret_whose_mark_cannot_be_saved_says_the_record_is_what_failed() {
            let bench = stage_bench();
            let mut catalog = crate::sensitive_data::SecretCatalog::new();
            catalog.register_credential("DUMMY_API_KEY", DUMMY).unwrap();
            crate::sensitive_data::install_scope(catalog, Some(&bench.workspace), None);
            let file = crate::planner::structured_stages::ledger::ledger_file(
                &bench.workspace.parent().unwrap().join("state"),
                &bench.workspace,
                PROFILE,
            )
            .unwrap();
            bench.mark("clobber", &file.display().to_string());
            bench.mark(
                &format!("diagnostics-{UPPER}"),
                &json!([{"code": "note", "message": DUMMY}]).to_string(),
            );
            let error = bench.call(&bench.upper, "fast", "initial").unwrap_err();
            assert_eq!(kind(&error), RefusalKind::StageLedgerRefused, "{error:#}");
            assert!(error.to_string().contains("diverged"), "{error:#}");
            assert!(!error.to_string().contains(DUMMY), "{error:#}");
        }

        /// The history's real function, not the snippet it calls: an answer has to survive what
        /// a name the history redacts does to it, on every OS.
        #[test]
        fn what_the_real_history_keeps_of_a_commit_still_says_it_sealed() {
            let bench = stage_bench();
            let answer = bench.seal_upper();
            let kept = crate::state::tool_history::history_content(
                Some("ProposeUpper"),
                &answer.to_string(),
            );
            let kept: Value = serde_json::from_str(&kept).unwrap();
            assert_eq!(kept["stage"]["status"], "sealed", "{kept}");
            assert_eq!(kept["stage"]["revision"], answer["stage"]["revision"]);

            let long = json!({"status": "resubmit", "note": "x".repeat(4000)}).to_string();
            let cut = crate::state::tool_history::history_content(Some("ProposeUpper"), &long);
            assert!(cut.chars().count() <= MODEL_RESULT_CHARS, "{cut}");
            let answered = super::super::fit_for_model(
                json!({"status": "resubmit", "code": "check_failed", "note": "x".repeat(4000)}),
                "/note",
                json!({"status": "resubmit", "code": "check_failed"}),
            );
            let kept = crate::state::tool_history::history_content(Some("P"), &answered);
            let kept: Value = serde_json::from_str(&kept).unwrap();
            assert_eq!(kept["code"], "check_failed", "{kept}");
            assert_eq!(kept["truncated"], true, "{kept}");
        }

        /// A cooked terminal, as with `COMMANDAGENT_NO_INTERRUPT` or a stdin that is no terminal:
        /// it turns each line feed into a return and a line feed, on top of the block's own.
        #[test]
        fn the_notice_goes_through_a_cooked_pty_with_its_left_edge() {
            use std::io::{Read, Write};
            use std::os::fd::{FromRawFd, OwnedFd};
            let (mut master, mut slave) = (-1, -1);
            // SAFETY: all five are valid for this call, and the three nulls mean no name and
            // default settings, which are cooked.
            let opened = unsafe {
                libc::openpty(
                    &raw mut master,
                    &raw mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(opened, 0, "{}", std::io::Error::last_os_error());
            // SAFETY: `openpty` returned these two and nothing else owns them.
            let master = unsafe { OwnedFd::from_raw_fd(master) };
            // SAFETY: as above.
            let slave = unsafe { OwnedFd::from_raw_fd(slave) };
            let block = super::super::stage_refusal_block(NOTICE, true);
            let mut writer = std::fs::File::from(slave.try_clone().unwrap());
            writer.write_all(block.as_bytes()).unwrap();
            // Every line feed gains a return, and the block holds two.
            let mut read = vec![0; block.len() + 2];
            std::fs::File::from(master).read_exact(&mut read).unwrap();
            drop(writer);
            drop(slave);
            let text = std::str::from_utf8(&read).unwrap();
            assert!(text.contains("\r\r\n"), "{text:?}");
            let screen = screen_after(&[text]);
            assert_eq!(screen[0], "stage record refused: corrupt");
            assert_eq!(screen[1], "  ledger: /state/ledger.json");
        }

        /// Whole sessions through the real admission, on every OS: the operator's files, a
        /// provider on 127.0.0.1, and the unconfined launch only a test build has. What these ask
        /// of the process is global (the registry variable and the registered profiles), so each
        /// runs in a process of its own, and the rest of this test binary never sees them.
        mod sessions {
            use std::io::{Read as _, Write as _};
            use std::path::{Path, PathBuf};
            use std::process::{Command, Stdio};
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::{Arc, Mutex};

            use serde_json::{Value, json};

            use super::super::super::super::descriptor::directory_pin;
            use super::super::super::super::isolation::test_hook::{Unconfined, unconfined};
            use super::{LOWER, LOWER_OUTPUT, UPPER, UPPER_OUTPUT, script, sha256};
            use crate::config::{Config, OllamaThink};
            use crate::minimal_loop::loop_run::run_session_with_outcome_with_ui;
            use crate::planner::runner::{run_step_plan, run_ultra_plan};
            use crate::planner::step_plan::{PlanStep, StepPlan};
            use crate::planner::ultra_plan::{UltraPhase, UltraPlan};
            use crate::providers::client_from_config;
            use crate::state::SessionSnapshot;
            use crate::tools::os_sandbox::{
                SandboxAvailability, SandboxMode, SandboxPolicy, SandboxTestState, install_for_test,
            };
            use crate::tui::NOOP_UI;

            /// A filter that matches no test, so a child can tell it is one with no variable.
            const CHILD_MARK: &str = "--skip=__stage_session_child__";
            const WORLD_ENV: &str = "STAGE_SESSION_WORLD";
            const ROLE_ENV: &str = "STAGE_SESSION_ROLE";
            const REGISTRY_ENV: &str = "COMMANDAGENT_ADAPTER_CONFIG_ROOT";
            const PROFILE: &str = "stage-session-config";
            const INDEX_PROFILE: &str = "stage-session-index";
            const PLAIN_PROFILE: &str = "stage-session-plain";
            const WRITES_PROFILE: &str = "stage-session-writes";
            const UPPER_TOOL: &str = "ProposeUpper";
            const LOWER_TOOL: &str = "ProposeLower";
            const FIXED: &str = "fixed/seed.json";
            const HOLD: &str = "Stage obligations are not met:";
            const CHILD_LIMIT: std::time::Duration = std::time::Duration::from_secs(120);

            struct World {
                root: PathBuf,
                evidence: PathBuf,
                marks: PathBuf,
                base: PathBuf,
            }

            /// What a scene needs alive: the installed boundary and the unconfined launch.
            struct Live {
                _sandbox: SandboxTestState,
                _unconfined: Unconfined,
            }

            impl World {
                fn at(base: &Path) -> Self {
                    Self {
                        root: base.join("workspace"),
                        evidence: base.join("evidence"),
                        marks: base.join("marks"),
                        base: base.to_path_buf(),
                    }
                }

                fn enter(&self) -> Live {
                    crate::planner::extension_profiles::register(&self.base.join("extension"))
                        .unwrap();
                    let sandbox = SandboxTestState::acquire();
                    let tmp = self.evidence.join("sandbox/run");
                    std::fs::create_dir_all(&tmp).unwrap();
                    install_for_test(
                        SandboxMode::Auto,
                        SandboxPolicy::new(&self.root, &tmp),
                        SandboxAvailability::Available,
                    );
                    Live {
                        _sandbox: sandbox,
                        _unconfined: unconfined(),
                    }
                }

                fn events(&self) -> String {
                    std::fs::read_to_string(self.evidence.join("events.jsonl")).unwrap_or_default()
                }

                fn config(&self, url: &str) -> Config {
                    let mut config = super::super::super::config_for_test(&self.root);
                    config.state_dir = self.evidence.join("state");
                    config.eval_events_path = Some(self.evidence.join("events.jsonl"));
                    config.yes = true;
                    config.offline = false;
                    config.context_budget = 100_000;
                    config.planner_think = Some(OllamaThink::False);
                    config.lm_studio_host = url.to_string();
                    config.num_predict = 100;
                    config.max_iterations = 8;
                    config.chat_timeout_secs = 30;
                    config.chat_retries = 1;
                    config.profile = PROFILE.to_string();
                    config
                }

                /// A prompt, as `--prompt` runs it.
                fn prompt(&self, responder: Responder) -> (Vec<Value>, Result<(), String>) {
                    let http = FakeHttp::start(responder);
                    let config = self.config(&http.url);
                    let mut client = client_from_config(&config, false).unwrap();
                    let result = run_session_with_outcome_with_ui(
                        client.as_mut(),
                        &mut SessionSnapshot::new(),
                        "Submit both stages.",
                        &[],
                        &config,
                        &NOOP_UI,
                    )
                    .map(|_| ())
                    .map_err(|error| format!("{error:#}"));
                    (http.finish(), result)
                }

                fn plan(
                    &self,
                    steps: Vec<PlanStep>,
                    responder: Responder,
                ) -> (Vec<Value>, Result<String, String>) {
                    let http = FakeHttp::start(responder);
                    let config = self.config(&http.url);
                    let mut client = client_from_config(&config, false).unwrap();
                    let plan = StepPlan {
                        goal: "Submit the stages.".to_string(),
                        steps,
                    };
                    let result = run_step_plan(client.as_mut(), &plan, &config)
                        .map_err(|error| format!("{error:#}"));
                    (http.finish(), result)
                }

                fn ultra(&self, responder: Responder) -> Result<String, String> {
                    self.ultra_with_notes(true, responder)
                }

                fn ultra_with_notes(
                    &self,
                    notes: bool,
                    responder: Responder,
                ) -> Result<String, String> {
                    self.ultra_as(PROFILE, notes, responder)
                }

                fn ultra_as(
                    &self,
                    profile: &str,
                    notes: bool,
                    responder: Responder,
                ) -> Result<String, String> {
                    if notes {
                        std::fs::write(self.root.join("notes.txt"), "the notes\n").unwrap();
                    }
                    let http = FakeHttp::start(responder);
                    let mut config = self.config(&http.url);
                    config.profile = profile.to_string();
                    let mut planner = client_from_config(&config, true).unwrap();
                    let mut execution = client_from_config(&config, false).unwrap();
                    let plan = UltraPlan {
                        goal: "Submit the stages".to_string(),
                        profile: profile.to_string(),
                        style: "default".to_string(),
                        intent: "create".to_string(),
                        phases: vec![
                            UltraPhase {
                                id: "first".to_string(),
                                prompt: "Phase-first: submit the upper stage.".to_string(),
                            },
                            UltraPhase {
                                id: "second".to_string(),
                                prompt: "Phase-second: submit the lower stage.".to_string(),
                            },
                        ],
                    };
                    let result =
                        run_ultra_plan(planner.as_mut(), execution.as_mut(), &plan, &config)
                            .map_err(|error| format!("{error:#}"));
                    http.finish();
                    result
                }
            }

            fn manifest(profile: &str, upper: &str, lower: &str) -> String {
                format!(
                    "[metadata]\nid = \"{profile}\"\ndisplay_name = \"Stage session\"\nschema_version = \"v2\"\ntask_family = \"generic\"\n[plan]\nintent = \"create\"\nphases = [\n  {{ id = \"implementation\", prompt = \"Write the notes for {{goal}}.\" }},\n]\n[artifacts]\nrequired = [\"notes.txt\"]\n[guidance.variants.contract]\ntriggers = [{{ condition = \"always\" }}]\nmessages = {{ instruction = \"Write notes.txt.\" }}\n[[checks.final]]\nid = \"scaffold_files_present\"\nparams = {{ files = [\"notes.txt\"] }}\n[profile_tools]\nschema_version = \"profile-tool.declaration/v2\"\nbuiltin_tools = [\"Read\"]\ntools = [\n  {{ id = \"{upper}\", effect = \"propose-artifacts\" }},\n  {{ id = \"{lower}\", effect = \"propose-artifacts\" }},\n]\n[profile_tools.stages]\nstages = [\n  {{ id = \"upper\", targets = [{{ id = \"main\", tool_id = \"{upper}\" }}] }},\n  {{ id = \"lower\", targets = [{{ id = \"main\", tool_id = \"{lower}\" }}] }},\n]\n"
                )
            }

            fn write_descriptor(base: &Path, tool: (&str, &str, &str), inputs: &[&str]) {
                let (id, name, output) = tool;
                let directory = base.join("tools").join(id);
                std::fs::create_dir_all(&directory).unwrap();
                let tool_script = directory.join("tool.sh");
                std::fs::write(&tool_script, script(id)).unwrap();
                let shell = std::fs::canonicalize("/bin/sh").unwrap();
                let schema = json!({
                    "protocol": "profile-tool.schema/v1",
                    "root": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["mode", "expected_revision"],
                        "properties": {
                            "mode": {"type": "string"},
                            "expected_revision": {"type": "string"},
                        },
                    },
                    "inputs": inputs,
                });
                let descriptor = json!({
                    "protocol": "profile-tool.descriptor/v1",
                    "tool_id": id,
                    "name": name,
                    "version": "1.0.0",
                    "description": "Propose one artifact for a synthetic fixture.",
                    "effect": "propose-artifacts",
                    "parameters": [],
                    "schema_sha256": sha256(serde_json::to_string(&schema).unwrap().as_bytes()),
                    "schema": schema,
                    "runtime": {"kind": "pinned_process", "id": "fixed-script"},
                    "limits": {"max_input_bytes": 1 << 20, "max_output_bytes": 64 * 1024},
                    "process": {
                        "protocol": "profile-tool.process/v1",
                        "executable": {
                            "path": shell,
                            "sha256": sha256(&std::fs::read(&shell).unwrap()),
                        },
                        "dependency_directories": [{
                            "path": directory,
                            "sha256": directory_pin(&directory).unwrap(),
                        }],
                        "argv": [tool_script],
                        "cwd": directory,
                        "env": {"MARKS": base.join("marks")},
                        "limits": {"timeout_ms": 20_000, "max_result_bytes": 64 * 1024},
                        "outputs": [{"id": "artifact", "path": output}],
                    },
                    "stage": {
                        "required_checks": ["shape"],
                        "attempts_per_target": 8,
                        "attempts_per_run": 32,
                        "deadline_seconds": 600,
                    },
                });
                std::fs::write(
                    base.join(format!("registry/profile-tools/{id}.json")),
                    descriptor.to_string(),
                )
                .unwrap();
            }

            /// The operator's files, in a directory the child is told about.
            fn prepared() -> tempfile::TempDir {
                let temp = tempfile::tempdir().unwrap();
                let base = std::fs::canonicalize(temp.path()).unwrap();
                for directory in [
                    "workspace/fixed",
                    "evidence",
                    "marks",
                    "registry/profile-tools",
                ] {
                    std::fs::create_dir_all(base.join(directory)).unwrap();
                }
                std::fs::write(base.join("workspace").join(FIXED), b"a fixed seed").unwrap();
                let profiles = [
                    (PROFILE, UPPER, LOWER),
                    (INDEX_PROFILE, "stage-index-upper", "stage-index-lower"),
                ];
                for (id, upper, lower) in profiles {
                    let profile = base.join(format!("extension/profiles/{id}"));
                    std::fs::create_dir_all(&profile).unwrap();
                    std::fs::write(profile.join("manifest.toml"), manifest(id, upper, lower))
                        .unwrap();
                }
                // The first manifest with its stage declaration taken out, and the schema version
                // that goes with having none.
                let whole = manifest(PLAIN_PROFILE, UPPER, LOWER);
                let plain = whole
                    .split("[profile_tools.stages]")
                    .next()
                    .unwrap()
                    .replace("profile-tool.declaration/v2", "profile-tool.declaration/v1");
                let profile = base.join(format!("extension/profiles/{PLAIN_PROFILE}"));
                std::fs::create_dir_all(&profile).unwrap();
                std::fs::write(profile.join("manifest.toml"), plain).unwrap();
                // The first manifest, keeping a tool that changes a file with no stage.
                let writes = manifest(WRITES_PROFILE, UPPER, LOWER).replace(
                    "builtin_tools = [\"Read\"]",
                    "builtin_tools = [\"Read\", \"Write\"]",
                );
                let profile = base.join(format!("extension/profiles/{WRITES_PROFILE}"));
                std::fs::create_dir_all(&profile).unwrap();
                std::fs::write(profile.join("manifest.toml"), writes).unwrap();
                write_descriptor(&base, (UPPER, UPPER_TOOL, UPPER_OUTPUT), &[FIXED]);
                write_descriptor(
                    &base,
                    (LOWER, LOWER_TOOL, LOWER_OUTPUT),
                    &[UPPER_OUTPUT, FIXED],
                );
                write_descriptor(
                    &base,
                    ("stage-index-upper", "ProposeIndexUpper", "index/upper.json"),
                    &[FIXED],
                );
                write_descriptor(
                    &base,
                    ("stage-index-lower", "ProposeIndexLower", "index/lower.json"),
                    &["index/upper.json", FIXED],
                );
                temp
            }

            struct Child {
                success: bool,
                stdout: String,
                stderr: Vec<u8>,
            }

            /// This test again in a process of its own, through the bounded runner: it gets what is
            /// named here and what that runner lets through, and its group is stopped if it does
            /// not end.
            fn run_child(name: &str, base: &Path, env: &[(&str, &str)]) -> Child {
                run_child_writing(name, base, env, false)
            }

            /// Its stderr goes to a file or, what it writes being short, to a pipe.
            fn run_child_writing(
                name: &str,
                base: &Path,
                env: &[(&str, &str)],
                to_a_pipe: bool,
            ) -> Child {
                start_child(name, base, env, to_a_pipe, true)
            }

            /// Whether this process is a child some parent here started.
            fn is_a_child() -> bool {
                std::env::args().any(|argument| argument == CHILD_MARK)
            }

            /// The world's directory, in a child. A child the variable did not reach must not
            /// take itself for a parent: it would start another, and that one another.
            fn world_base() -> Option<std::ffi::OsString> {
                if let Some(base) = std::env::var_os(WORLD_ENV) {
                    return Some(base);
                }
                assert!(
                    !is_a_child(),
                    "the world's variable did not reach this child: it must not start another"
                );
                None
            }

            fn start_child(
                name: &str,
                base: &Path,
                env: &[(&str, &str)],
                to_a_pipe: bool,
                with_world: bool,
            ) -> Child {
                // The second stop: whatever got past `world_base`, no child starts a child. It
                // leaves a mark for the test below to find.
                if is_a_child() {
                    if let Some(mark) = std::env::var_os(ROLE_ENV) {
                        std::fs::write(mark, "tried to start another").unwrap();
                    }
                    panic!("a child must not start another");
                }
                let log = base.join(format!("child-{}.log", uuid::Uuid::now_v7()));
                let err = base.join(format!("child-{}.err", uuid::Uuid::now_v7()));
                let path = format!("{}::{name}", module_path!().split_once("::").unwrap().1);
                let mut command = Command::new(std::env::current_exe().unwrap());
                command
                    .args(["--exact", path.as_str(), "--test-threads=1", "--nocapture"])
                    .arg(CHILD_MARK)
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin")
                    .env("HOME", base.join("evidence"))
                    .env(REGISTRY_ENV, base.join("registry"))
                    .stdin(Stdio::null())
                    .stdout(std::fs::File::create(&log).unwrap())
                    .stderr(if to_a_pipe {
                        Stdio::piped()
                    } else {
                        Stdio::from(std::fs::File::create(&err).unwrap())
                    });
                if with_world {
                    command.env(WORLD_ENV, base);
                }
                for (key, value) in env {
                    command.env(key, value);
                }
                // A timeout stops this child's process group. A tool's process is in a group of
                // its own and stays, until its short script ends with its stdin closed.
                let output =
                    crate::bounded_process::run_with_timeout(&mut command, CHILD_LIMIT).unwrap();
                assert_eq!(
                    output.kind,
                    crate::bounded_process::BoundedProcessOutcomeKind::Exited,
                    "{name} did not end"
                );
                let mut stderr = std::fs::read(&err).unwrap_or_default();
                stderr.extend_from_slice(&output.stderr);
                Child {
                    success: output.success(),
                    stdout: std::fs::read_to_string(&log).unwrap_or_default(),
                    stderr,
                }
            }

            /// The world in the child. In the parent it runs the child and answers `None`.
            fn one_process(name: &str) -> Option<World> {
                if let Some(base) = world_base() {
                    return Some(World::at(Path::new(&base)));
                }
                let temp = prepared();
                let base = std::fs::canonicalize(temp.path()).unwrap();
                let child = run_child(name, &base, &[]);
                assert!(
                    child.success,
                    "{name}: {}\n{}",
                    child.stdout,
                    String::from_utf8_lossy(&child.stderr)
                );
                // Each start leaves a log here: one test, one child.
                let logs = std::fs::read_dir(&base)
                    .unwrap()
                    .flatten()
                    .filter(|entry| entry.file_name().to_string_lossy().ends_with(".log"))
                    .count();
                assert_eq!(logs, 1, "{name}");
                None
            }

            /// T2 (acceptance 4): two stages open get the one repair budget final acceptance has,
            /// and not one each.
            #[test]
            fn stages_left_open_get_no_final_repairs_of_their_own() {
                const NAME: &str = "stages_left_open_get_no_final_repairs_of_their_own";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                // A repair that keeps changing a file and never calls a stage, so each attempt
                // ends well, the bytes moved, and final acceptance still says no. Only the budget
                // can stop this.
                let written = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let responder: Responder = Arc::new(move |_, body| {
                    if !body["tools"].to_string().contains(UPPER_TOOL) {
                        return text_reply(
                            &json!({"goal": "Look", "steps": [{
                                "id": "step-setup", "kind": "setup", "expected_result": "pass",
                                "instruction": "Look", "expected_paths": [], "verify": [],
                            }]})
                            .to_string(),
                        );
                    }
                    let answered = messages(body).last().is_some_and(|m| m["role"] == "tool");
                    if answered {
                        return text_reply("done");
                    }
                    if body
                        .to_string()
                        .contains("Repair the final acceptance failure")
                    {
                        let round = written.fetch_add(1, Ordering::SeqCst);
                        return call_reply(
                            "Write",
                            // A source file, the kind the repair's before and after compares.
                            &json!({"path": "repair.py", "content": format!("# round {round}\n")}),
                        );
                    }
                    call_reply("Read", &json!({"path": FIXED}))
                });
                world
                    .ultra_as(WRITES_PROFILE, true, responder)
                    .expect_err("two stages are open");
                let events = world.events();
                let unmet = events
                    .lines()
                    .rfind(|line| line.contains("\"event\":\"structured_stage_incomplete\""))
                    .expect("final acceptance asked");
                assert!(
                    unmet.contains("upper") && unmet.contains("lower"),
                    "both still open at the end: {unmet}"
                );
                let starts = events
                    .matches("\"event\":\"final_acceptance_repair_start\"")
                    .count();
                // `FINAL_ACCEPTANCE_REPAIR_MAX_ATTEMPTS`, which this module cannot name.
                assert_eq!(starts, 2, "two stages, the one budget, all of it: {events}");
                let failed = events
                    .lines()
                    .find(|line| line.contains("\"event\":\"ultra_final_acceptance_failed\""))
                    .expect("final acceptance failed");
                assert!(failed.contains("\"max_attempts\":2"), "{failed}");
                let exhausted = events
                    .lines()
                    .find(|line| line.contains("\"event\":\"final_acceptance_repair_exhausted\""))
                    .expect("the repair ran out");
                assert!(
                    exhausted.contains("\"exhausted_reason\":\"bounded_repair_exhausted\""),
                    "the budget is what stopped it: {exhausted}"
                );
            }

            /// The revision the upper stage's seal named, as a model has it in its history.
            fn revision_the_upper_seal_named(body: &Value) -> String {
                messages(body)
                    .iter()
                    .filter(|message| message["role"] == "tool")
                    .filter_map(|message| message["content"].as_str())
                    .filter_map(|text| serde_json::from_str::<Value>(text).ok())
                    .find_map(|answer| {
                        answer["stage"]["downstream"][0]["revision"]
                            .as_str()
                            .map(str::to_string)
                    })
                    .expect("the seal's answer is in the history")
            }

            /// The plan's steps leave the lower stage open. The final repair's first reply seals
            /// that stage and makes these writes. It has to be one reply: a repair with its
            /// required file there ends at its first batch of tools.
            fn a_repair_that_seals_and_writes(
                writes: &'static [(&'static str, &'static str)],
            ) -> Responder {
                let steps = ultra_steps(false);
                let replied = Arc::new(AtomicBool::new(false));
                Arc::new(move |index, body| {
                    if !body
                        .to_string()
                        .contains("Repair the final acceptance failure")
                        || !body["tools"].to_string().contains(UPPER_TOOL)
                    {
                        return steps(index, body);
                    }
                    if replied.swap(true, Ordering::SeqCst) {
                        return text_reply("done");
                    }
                    let revision = revision_the_upper_seal_named(body);
                    let mut calls = vec![(
                        LOWER_TOOL,
                        json!({"mode": "fast", "expected_revision": revision}),
                    )];
                    calls.extend(writes.iter().map(|(path, content)| {
                        ("Write", json!({"path": path, "content": content}))
                    }));
                    calls_reply(&calls)
                })
            }

            /// The other side of the known limit below. A repair that also changes a compared file
            /// has final acceptance asked again, and a new judgment is what ends it as a success.
            /// That judgment reads the bytes there at the time.
            #[test]
            fn a_final_repair_that_seals_and_changes_a_source_is_asked_again() {
                const NAME: &str = "a_final_repair_that_seals_and_changes_a_source_is_asked_again";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world
                    .ultra_as(
                        WRITES_PROFILE,
                        true,
                        a_repair_that_seals_and_writes(&[("repair.py", "# the repair\n")]),
                    )
                    .expect("asked again, with every stage sealed");
                let events = world.events();
                for first_judgment_only in [
                    "\"event\":\"final_acceptance_repair_start\"",
                    "\"event\":\"structured_stage_incomplete\"",
                ] {
                    assert_eq!(
                        events.matches(first_judgment_only).count(),
                        1,
                        "{first_judgment_only}: {events}"
                    );
                }
                assert!(!events.contains("\"reprobe_skipped\":true"), "{events}");
                let again = events
                    .lines()
                    .find(|line| line.contains("\"event\":\"final_acceptance_cycle_complete\""))
                    .expect("no new judgment");
                assert!(again.contains("\"ok\":true"), "{again}");
                assert!(again.contains("repair.py"), "{again}");
                let complete = events
                    .lines()
                    .rfind(|line| line.contains("\"event\":\"ultra_plan_complete\""))
                    .expect("the run's end");
                assert!(complete.contains("\"ok\":true"), "{complete}");

                // The same judgment, on other bytes.
                let mut config = world.config("http://127.0.0.1:9");
                config.profile = WRITES_PROFILE.to_string();
                assert_eq!(
                    crate::planner::structured_stages::final_gate_failure(&config),
                    None
                );
                std::fs::write(world.root.join(LOWER_OUTPUT), "changed after the seal\n").unwrap();
                let failure = crate::planner::structured_stages::final_gate_failure(&config)
                    .expect("the seal no longer stands for these bytes");
                assert!(failure.contains("stage_incomplete"), "{failure}");
                assert!(failure.contains("lower/main"), "{failure}");
            }

            /// The control: the same reply, which also changes the stage's output after sealing it.
            /// The end of the repair's session judges those bytes before acceptance can ask.
            #[test]
            fn a_final_repair_whose_stage_output_then_changes_is_no_success() {
                const NAME: &str = "a_final_repair_whose_stage_output_then_changes_is_no_success";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                let error = world
                    .ultra_as(
                        WRITES_PROFILE,
                        true,
                        a_repair_that_seals_and_writes(&[
                            ("repair.py", "# the repair\n"),
                            (LOWER_OUTPUT, "changed after the seal\n"),
                        ]),
                    )
                    .expect_err("the seal no longer stands for these bytes");
                assert!(error.contains("repair failed"), "{error}");
                assert!(error.contains("stage_incomplete"), "{error}");
                assert!(error.contains("lower/main"), "{error}");
                let events = world.events();
                let unmet = events
                    .lines()
                    .rfind(|line| line.contains("\"event\":\"structured_stage_incomplete\""))
                    .expect("the session's end asked");
                assert!(unmet.contains("\"phase\":\"unverified\""), "{unmet}");
                assert!(
                    !events.contains("\"event\":\"final_acceptance_cycle_complete\""),
                    "{events}"
                );
            }

            /// A known limit, measured. A final repair that seals the open stage changes nothing the
            /// repair's before and after compare, so acceptance does not ask again and the run ends
            /// as a failure about a stage that is sealed by then. A failure, not a false pass.
            #[test]
            fn a_final_repair_that_seals_the_open_stage_still_ends_as_a_failure() {
                const NAME: &str =
                    "a_final_repair_that_seals_the_open_stage_still_ends_as_a_failure";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                let steps = ultra_steps(false);
                let responder: Responder = Arc::new(move |index, body| {
                    let text = body.to_string();
                    if !text.contains("Repair the final acceptance failure")
                        || !body["tools"].to_string().contains(UPPER_TOOL)
                    {
                        return steps(index, body);
                    }
                    let last = messages(body).last();
                    if last.is_some_and(|message| message["role"] == "tool") {
                        let answer = last_answer(body);
                        return match answer["revision"].as_str() {
                            Some(revision) if answer["status"] == "stale" => {
                                submit(LOWER_TOOL, "fast", revision)
                            }
                            _ => text_reply("done"),
                        };
                    }
                    submit(LOWER_TOOL, "fast", &revision_the_upper_seal_named(body))
                });
                let error = world.ultra(responder).expect_err("the known limit");
                assert!(error.contains("stage_incomplete"), "{error}");
                // The stage is sealed by now, and the run never asked again.
                let config = world.config("http://127.0.0.1:9");
                assert_eq!(
                    crate::planner::structured_stages::final_gate_failure(&config),
                    None,
                    "{error}"
                );
                let events = world.events();
                assert!(
                    events
                        .contains("\"failure_kind\":\"final_acceptance_repair_no_source_change\""),
                    "{events}"
                );
                assert!(events.contains("\"reprobe_skipped\":true"), "{events}");
            }

            /// V9: on a real terminal the notice is what the terminal needs. This asks through the
            /// way the host picks, and not by passing `true`.
            #[test]
            fn a_terminal_on_stderr_gets_the_notice_with_its_returns() {
                use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
                const NAME: &str = "a_terminal_on_stderr_gets_the_notice_with_its_returns";
                if world_base().is_some() {
                    super::super::super::show_stage_refusal(&json!({
                        "code": "corrupt",
                        "ledger_path": "/state/ledger.json",
                    }));
                    return;
                }
                let (mut master, mut slave) = (-1, -1);
                // SAFETY: all five are valid for this call, and the nulls mean no name and
                // default settings.
                let opened = unsafe {
                    libc::openpty(
                        &raw mut master,
                        &raw mut slave,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                assert_eq!(opened, 0, "{}", std::io::Error::last_os_error());
                // SAFETY: `openpty` returned these two and nothing else owns them.
                let master = unsafe { OwnedFd::from_raw_fd(master) };
                // SAFETY: as above.
                let slave = unsafe { OwnedFd::from_raw_fd(slave) };
                // SAFETY: an all-zero `termios` is a valid value for `tcgetattr` to fill in.
                let mut modes = unsafe { std::mem::zeroed::<libc::termios>() };
                // SAFETY: `slave` is open and `modes` is a valid place to write.
                let got = unsafe { libc::tcgetattr(slave.as_raw_fd(), &raw mut modes) };
                assert_eq!(got, 0);
                // SAFETY: `modes` was just filled in.
                unsafe { libc::cfmakeraw(&raw mut modes) };
                // SAFETY: `slave` is open and `modes` is valid.
                let set =
                    unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &raw const modes) };
                assert_eq!(set, 0);

                let temp = prepared();
                let base = std::fs::canonicalize(temp.path()).unwrap();
                let path = format!("{}::{NAME}", module_path!().split_once("::").unwrap().1);
                let mut command = Command::new(std::env::current_exe().unwrap());
                command
                    .args(["--exact", path.as_str(), "--test-threads=1", "--nocapture"])
                    .arg(CHILD_MARK)
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin")
                    .env(WORLD_ENV, &base)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(std::fs::File::from(slave.try_clone().unwrap()));
                let output =
                    crate::bounded_process::run_with_timeout(&mut command, CHILD_LIMIT).unwrap();
                assert!(output.success(), "{:?}", output.kind);
                // SAFETY: `master` is open; this only sets a flag on it.
                let flagged =
                    unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) };
                assert_eq!(flagged, 0);
                let mut bytes = vec![0; 4096];
                let read = std::fs::File::from(master).read(&mut bytes).unwrap();
                drop(slave);
                let text = String::from_utf8_lossy(&bytes[..read]);
                assert!(
                    text.contains("\r\x1b[2Kstage record refused: corrupt\r\n"),
                    "{text:?}"
                );
                assert!(
                    text.contains("  ledger: /state/ledger.json\r\n"),
                    "{text:?}"
                );
            }

            /// `stage_has_unmet_obligation`: the second Read of a committed output would end this
            /// run (the control below). With a stage open it is asked again instead.
            #[test]
            fn a_read_after_a_commit_goes_on_while_a_stage_is_open() {
                let Some(world) =
                    one_process("a_read_after_a_commit_goes_on_while_a_stage_is_open")
                else {
                    return;
                };
                let _live = world.enter();
                let responder: Responder = Arc::new(|index, _| match index {
                    1 => submit(UPPER_TOOL, "fast", "initial"),
                    2 | 3 => call_reply("Read", &json!({"path": UPPER_OUTPUT})),
                    _ => text_reply("stopping"),
                });
                let (requests, result) = world.prompt(responder);
                assert_stage_incomplete(&result);
                assert!(
                    requests.len() > 3,
                    "ended at the second Read: {}",
                    requests.len()
                );
                assert!(
                    !world
                        .events()
                        .contains("post_write_read_confirmation_completed")
                );
            }

            /// The control of the test above: the same two Reads with every stage sealed do end the
            /// session there, so those Reads are the ones that path counts.
            #[test]
            fn two_reads_after_every_stage_is_sealed_end_the_session() {
                let Some(world) =
                    one_process("two_reads_after_every_stage_is_sealed_end_the_session")
                else {
                    return;
                };
                let _live = world.enter();
                let responder: Responder = Arc::new(|index, body| match index {
                    1 => submit(UPPER_TOOL, "fast", "initial"),
                    2 => submit(LOWER_TOOL, "fast", &revision_below(&last_answer(body))),
                    3 | 4 => call_reply("Read", &json!({"path": UPPER_OUTPUT})),
                    _ => text_reply("stopping"),
                });
                let (requests, result) = world.prompt(responder);
                result.unwrap();
                assert_eq!(requests.len(), 4);
                assert!(
                    world
                        .events()
                        .contains("post_write_read_confirmation_completed"),
                    "{}",
                    world.events()
                );
            }

            /// `unread` in the tool-only hold: nothing is open, and the answer still goes back to
            /// the model. This step's one call moves nothing, so only an unread answer can hold it.
            #[test]
            fn an_answer_that_moved_nothing_goes_back_with_no_obligation_open() {
                let Some(world) =
                    one_process("an_answer_that_moved_nothing_goes_back_with_no_obligation_open")
                else {
                    return;
                };
                let _live = world.enter();
                let responder: Responder = Arc::new(|index, body| match index {
                    1 => submit(UPPER_TOOL, "fast", "initial"),
                    2 => submit(LOWER_TOOL, "fast", "initial"),
                    3 => {
                        let answer = last_answer(body);
                        submit(LOWER_TOOL, "fast", answer["revision"].as_str().unwrap())
                    }
                    4 => {
                        let answer = last_answer(body);
                        submit(
                            LOWER_TOOL,
                            "fast",
                            answer["stage"]["revision"].as_str().unwrap(),
                        )
                    }
                    _ => text_reply("done"),
                });
                let steps = vec![
                    step("Submit the upper stage", "step-upper"),
                    step("Submit the lower stage", "step-lower"),
                    step("Submit the lower stage again", "step-again"),
                ];
                let (requests, result) = world.plan(steps, responder);
                result.unwrap();
                assert_eq!(requests.len(), 5, "ended at the tool's answer");
                let last = &requests[4];
                assert_eq!(last_answer(last)["status"], "no_change", "{last}");
                assert!(!last.to_string().contains(HOLD), "nothing was open: {last}");
            }

            /// `stages_declared_without_a_read`, on every OS: with no step there is no session, so
            /// nothing bound the stages, and they are declared all the same.
            #[test]
            fn a_plan_with_no_step_answers_for_no_stage_and_is_no_success() {
                let Some(world) =
                    one_process("a_plan_with_no_step_answers_for_no_stage_and_is_no_success")
                else {
                    return;
                };
                let _live = world.enter();
                let (requests, result) =
                    world.plan(Vec::new(), Arc::new(|_, _| text_reply("nobody asked")));
                assert_eq!(requests.len(), 0);
                let error = result.expect_err("no stage was ever bound");
                assert!(
                    error.contains("stage_incomplete: no_session_bound_the_declared_stages"),
                    "{error}"
                );
            }

            /// An implement step's stagnation notices, with a file nothing here writes.
            fn notices_of(world: &World, replies: Vec<&'static str>) -> usize {
                let second = world.marks.join(format!("second-{UPPER}"));
                let responder: Responder =
                    Arc::new(move |index, body| match replies.get(index - 1) {
                        Some(&"seal") => {
                            let _ = std::fs::remove_file(&second);
                            let current = last_answer_or_initial(body);
                            submit(UPPER_TOOL, "fast", &current)
                        }
                        Some(&"nothing") => submit(UPPER_TOOL, "fast", "not-a-revision"),
                        _ => text_reply("stopping"),
                    });
                let mut implement = step("Submit the upper stage", "step-implement");
                implement.kind = "implement".to_string();
                implement.expected_paths = vec!["notes.txt".to_string()];
                let before = world
                    .events()
                    .matches("\"event\":\"read_only_stagnation_feedback\"")
                    .count();
                let _ = world.plan(vec![implement], responder);
                world
                    .events()
                    .matches("\"event\":\"read_only_stagnation_feedback\"")
                    .count()
                    - before
            }

            fn last_answer_or_initial(body: &Value) -> String {
                if has_answer(body) {
                    let answer = last_answer(body);
                    if let Some(revision) = answer["revision"].as_str() {
                        return revision.to_string();
                    }
                }
                "initial".to_string()
            }

            /// `is_stage_tool`: calls that start nothing read as stagnation.
            #[test]
            fn calls_that_start_nothing_count_as_reading_to_the_loop() {
                let Some(world) =
                    one_process("calls_that_start_nothing_count_as_reading_to_the_loop")
                else {
                    return;
                };
                let _live = world.enter();
                let notices = notices_of(&world, vec!["nothing", "nothing", "nothing", "nothing"]);
                assert!(notices >= 1, "{}", world.events());
            }

            /// `batch_all_read_only_tools`: a seal between them starts the count over.
            #[test]
            fn a_seal_starts_the_loops_count_over() {
                let Some(world) = one_process("a_seal_starts_the_loops_count_over") else {
                    return;
                };
                let _live = world.enter();
                let notices = notices_of(
                    &world,
                    vec!["nothing", "nothing", "seal", "nothing", "nothing"],
                );
                assert_eq!(notices, 0, "{}", world.events());
            }

            /// A plan of the given intent, as an operator's run would make it: a reproducer that
            /// fails first, then the stages. Its own adjudication gets what it asks for.
            fn run_intent(
                world: &World,
                intent: &str,
                reproducer: &'static str,
                seal_lower: bool,
            ) -> Result<String, String> {
                std::fs::write(world.root.join("notes.txt"), "the notes\n").unwrap();
                let responder: Responder = Arc::new(move |_, body| {
                    let text = body.to_string();
                    if !body["tools"].to_string().contains(UPPER_TOOL) {
                        if text.contains("Phase-reproduce") {
                            return text_reply(
                                &json!({"goal": "reproduce", "steps": [{
                                    "id": "step-reproduce",
                                    "kind": "verify",
                                    "expected_result": "fail",
                                    "instruction": "Reproduce the failure",
                                    "expected_paths": [],
                                    "verify": [reproducer],
                                }]})
                                .to_string(),
                            );
                        }
                        let goal = if text.contains("Phase-lower") {
                            "Submit the lower stage"
                        } else {
                            "Submit the upper stage"
                        };
                        return text_reply(
                            &json!({"goal": goal, "steps": [{
                                "id": "step-setup",
                                "kind": "setup",
                                "expected_result": "pass",
                                "instruction": goal,
                                "expected_paths": [],
                                "verify": [],
                            }]})
                            .to_string(),
                        );
                    }
                    if !text.contains("Submit the lower stage") {
                        return if has_answer(body) {
                            text_reply("done")
                        } else {
                            submit(UPPER_TOOL, "fast", "initial")
                        };
                    }
                    let answer = last_answer(body);
                    if answer["status"] == "stale" {
                        submit(LOWER_TOOL, "fast", answer["revision"].as_str().unwrap())
                    } else if answer["path"] == LOWER_OUTPUT {
                        text_reply("done")
                    } else {
                        submit(LOWER_TOOL, "fast", "initial")
                    }
                });
                let http = FakeHttp::start(responder);
                let mut config = world.config(&http.url);
                if intent == "investigate" {
                    config.intent_override =
                        Some(crate::planner::adjudication::contract::IntentId::Investigate);
                }
                let mut planner = client_from_config(&config, true).unwrap();
                let mut execution = client_from_config(&config, false).unwrap();
                let phase = |id: &str, prompt: &str| UltraPhase {
                    id: id.to_string(),
                    prompt: prompt.to_string(),
                };
                let mut phases = vec![
                    phase("reproduce", "Phase-reproduce: reproduce the failure."),
                    phase("upper", "Phase-upper: submit the upper stage."),
                ];
                if seal_lower {
                    phases.push(phase("lower", "Phase-lower: submit the lower stage."));
                }
                let plan = UltraPlan {
                    goal: "Find out why it fails".to_string(),
                    profile: PROFILE.to_string(),
                    style: "default".to_string(),
                    intent: intent.to_string(),
                    phases,
                };
                let result = run_ultra_plan(planner.as_mut(), execution.as_mut(), &plan, &config)
                    .map_err(|error| format!("{error:#}"));
                http.finish();
                result
            }

            fn write_investigation_evidence(world: &World) {
                std::fs::create_dir_all(world.root.join("output")).unwrap();
                std::fs::write(world.root.join("output/diagnosis.md"), "# Diagnosis\n").unwrap();
                std::fs::create_dir_all(world.root.join("evidence")).unwrap();
                std::fs::write(
                    world.root.join("evidence/investigation-binding.json"),
                    serde_json::to_vec(
                        &crate::planner::adjudication::investigate::InvestigationBindingEvidence::new(
                            Vec::new(),
                        ),
                    )
                    .unwrap(),
                )
                .unwrap();
            }

            #[test]
            fn a_fix_succeeds_only_with_every_stage_sealed() {
                const NAME: &str = "a_fix_succeeds_only_with_every_stage_sealed";
                let Some(base) = world_base() else {
                    for role in ["open", "sealed"] {
                        let temp = prepared();
                        let base = std::fs::canonicalize(temp.path()).unwrap();
                        let child = run_child(NAME, &base, &[(ROLE_ENV, role)]);
                        assert!(
                            child.success,
                            "{role}: {}",
                            String::from_utf8_lossy(&child.stderr)
                        );
                    }
                    return;
                };
                let world = World::at(Path::new(&base));
                let _live = world.enter();
                let sealed = std::env::var_os(ROLE_ENV).is_some_and(|role| role == "sealed");
                let result = run_intent(&world, "fix", "test -f out/upper.json", sealed);
                if sealed {
                    assert!(result.unwrap().contains("complete"));
                } else {
                    assert_stage_incomplete(&result);
                }
            }

            #[test]
            fn an_investigation_succeeds_only_with_every_stage_sealed() {
                const NAME: &str = "an_investigation_succeeds_only_with_every_stage_sealed";
                let Some(base) = world_base() else {
                    for role in ["open", "sealed"] {
                        let temp = prepared();
                        let base = std::fs::canonicalize(temp.path()).unwrap();
                        let child = run_child(NAME, &base, &[(ROLE_ENV, role)]);
                        assert!(
                            child.success,
                            "{role}: {}",
                            String::from_utf8_lossy(&child.stderr)
                        );
                    }
                    return;
                };
                let world = World::at(Path::new(&base));
                let _live = world.enter();
                write_investigation_evidence(&world);
                let sealed = std::env::var_os(ROLE_ENV).is_some_and(|role| role == "sealed");
                let result = run_intent(&world, "investigate", "test -f no-such-marker", sealed);
                if sealed {
                    assert!(result.unwrap().contains("complete"));
                } else {
                    assert_stage_incomplete(&result);
                }
            }

            /// T1 of stage 4b, where every OS runs it: a fix's reproducer that rewrites what a stage
            /// sealed leaves bytes the seal does not stand for.
            #[test]
            fn a_fix_whose_reproducer_rewrites_a_sealed_output_is_refused_at_its_end() {
                let Some(world) = one_process(
                    "a_fix_whose_reproducer_rewrites_a_sealed_output_is_refused_at_its_end",
                ) else {
                    return;
                };
                let _live = world.enter();
                std::fs::write(
                    world.root.join("rewrite.sh"),
                    "test -f out/upper.json || exit 1\n[ ! -f out/lower.json ] || printf x >> out/lower.json\n",
                )
                .unwrap();
                let result = run_intent(&world, "fix", "sh rewrite.sh", true);
                assert_stage_incomplete(&result);
                assert!(result.unwrap_err().contains("lower/main"));
            }

            /// The same two steps, on tools with no stage: each answers once and that is all.
            fn each_tool_once() -> Responder {
                Arc::new(|_, body| {
                    let text = body.to_string();
                    if !body["tools"].to_string().contains(UPPER_TOOL) {
                        let goal = if text.contains("Phase-second") {
                            "Submit the lower stage"
                        } else {
                            "Submit the upper stage"
                        };
                        return text_reply(
                            &json!({"goal": goal, "steps": [{
                                "id": "step-setup",
                                "kind": "setup",
                                "expected_result": "pass",
                                "instruction": goal,
                                "expected_paths": [],
                                "verify": [],
                            }]})
                            .to_string(),
                        );
                    }
                    if messages(body)
                        .last()
                        .is_some_and(|message| message["role"] == "tool")
                    {
                        return text_reply("done");
                    }
                    let tool = if text.contains("Submit the lower stage") {
                        LOWER_TOOL
                    } else {
                        UPPER_TOOL
                    };
                    submit(tool, "fast", "initial")
                })
            }

            /// The level of each of the two events that report one, in this run's last.
            fn levels(events: &str) -> (String, String) {
                let level_of = |name: &str| {
                    let line = events
                        .lines()
                        .rfind(|line| line.contains(&format!("\"event\":\"{name}\"")))
                        .unwrap_or_else(|| panic!("no {name}: {events}"));
                    let value: Value = serde_json::from_str(line).unwrap();
                    value["assurance_level"].as_str().unwrap().to_string()
                };
                (
                    level_of("ultra_final_acceptance"),
                    level_of("ultra_plan_complete"),
                )
            }

            /// Acceptance 6: a seal raises no level. The same profile with its stage declaration
            /// taken out, and nothing else, ends at the same two levels as with every stage sealed.
            #[test]
            fn a_seal_raises_no_assurance_level() {
                const NAME: &str = "a_seal_raises_no_assurance_level";
                let Some(base) = world_base() else {
                    let run = |role: &str| {
                        let temp = prepared();
                        let base = std::fs::canonicalize(temp.path()).unwrap();
                        let child = run_child(NAME, &base, &[(ROLE_ENV, role)]);
                        assert!(
                            child.success,
                            "{role}: {}",
                            String::from_utf8_lossy(&child.stderr)
                        );
                        let events =
                            std::fs::read_to_string(base.join("evidence/events.jsonl")).unwrap();
                        levels(&events)
                    };
                    let sealed = run("sealed");
                    assert_eq!(run("plain"), sealed, "the stages changed a level");
                    assert_eq!(
                        sealed.1, "full",
                        "what a manifest profile reports: {sealed:?}"
                    );
                    return;
                };
                let world = World::at(Path::new(&base));
                let _live = world.enter();
                let sealed = std::env::var_os(ROLE_ENV).is_some_and(|role| role == "sealed");
                if sealed {
                    world.ultra_as(PROFILE, true, ultra_steps(true)).unwrap();
                } else {
                    // With no stage above it, the lower tool's input has to be there to be adopted.
                    std::fs::create_dir_all(world.root.join("out")).unwrap();
                    std::fs::write(world.root.join(UPPER_OUTPUT), b"{\"version\":1}\n").unwrap();
                    world
                        .ultra_as(PLAIN_PROFILE, true, each_tool_once())
                        .unwrap();
                }
                assert!(world.root.join(LOWER_OUTPUT).is_file(), "both tools ran");
                assert_eq!(
                    world.evidence.join("state/structured-stages").exists(),
                    sealed,
                    "only the declared run has a record"
                );
            }

            /// A child the world's variable did not reach fails at once, and starts nothing. The
            /// mark is what the second stop would leave, so a chain cannot go past one child even
            /// if the first stop is gone.
            #[test]
            fn a_child_the_world_did_not_reach_starts_nothing_more() {
                const NAME: &str = "a_child_the_world_did_not_reach_starts_nothing_more";
                if is_a_child() {
                    one_process(NAME);
                    panic!("this child went on as if it were a parent");
                }
                let temp = prepared();
                let base = std::fs::canonicalize(temp.path()).unwrap();
                let mark = base.join("tried");
                let role = mark.display().to_string();
                let child = start_child(NAME, &base, &[(ROLE_ENV, role.as_str())], false, false);
                assert!(!child.success, "{}", child.stdout);
                let stderr = String::from_utf8_lossy(&child.stderr);
                assert!(stderr.contains("did not reach this child"), "{stderr}");
                assert!(!mark.exists(), "it tried to start another: {stderr}");
            }

            type Responder = Arc<dyn Fn(usize, &Value) -> String + Send + Sync>;

            struct FakeHttp {
                url: String,
                requests: Arc<Mutex<Vec<Value>>>,
                stop: Arc<AtomicBool>,
                thread: Option<std::thread::JoinHandle<()>>,
            }

            impl FakeHttp {
                fn start(responder: Responder) -> Self {
                    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                    listener.set_nonblocking(true).unwrap();
                    let url = format!("http://{}", listener.local_addr().unwrap());
                    let requests: Arc<Mutex<Vec<Value>>> = Arc::default();
                    let stop = Arc::new(AtomicBool::new(false));
                    let thread = {
                        let (requests, stop) = (requests.clone(), stop.clone());
                        std::thread::spawn(move || {
                            while !stop.load(Ordering::SeqCst) {
                                match listener.accept() {
                                    Ok((stream, _)) => serve(stream, &requests, &responder),
                                    Err(_) => {
                                        std::thread::sleep(std::time::Duration::from_millis(2));
                                    }
                                }
                            }
                        })
                    };
                    Self {
                        url,
                        requests,
                        stop,
                        thread: Some(thread),
                    }
                }

                fn finish(mut self) -> Vec<Value> {
                    self.stop.store(true, Ordering::SeqCst);
                    self.thread.take().unwrap().join().unwrap();
                    self.requests.lock().unwrap().clone()
                }
            }

            fn serve(
                mut stream: std::net::TcpStream,
                requests: &Mutex<Vec<Value>>,
                responder: &Responder,
            ) {
                stream.set_nonblocking(false).unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0u8; 8192];
                let body_start = loop {
                    let read = stream.read(&mut chunk).unwrap_or(0);
                    if read == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..read]);
                    if let Some(at) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        break at + 4;
                    }
                };
                let head = String::from_utf8_lossy(&bytes[..body_start]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                while bytes.len() < body_start + length {
                    let read = stream.read(&mut chunk).unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..read]);
                }
                let body: Value =
                    serde_json::from_slice(&bytes[body_start..]).unwrap_or(Value::Null);
                let index = {
                    let mut requests = requests.lock().unwrap();
                    requests.push(body.clone());
                    requests.len()
                };
                let reply = responder(index, &body);
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }

            fn text_reply(content: &str) -> String {
                json!({"choices": [{"message": {"role": "assistant", "content": content}}]})
                    .to_string()
            }

            fn call_reply(name: &str, arguments: &Value) -> String {
                json!({"choices": [{"message": {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": [{
                        "id": "call-0",
                        "type": "function",
                        "function": {"name": name, "arguments": arguments.to_string()},
                    }],
                }}]})
                .to_string()
            }

            /// One reply with several calls, run in this order.
            fn calls_reply(calls: &[(&str, Value)]) -> String {
                let calls = calls
                    .iter()
                    .enumerate()
                    .map(|(index, (name, arguments))| {
                        json!({
                            "id": format!("call-{index}"),
                            "type": "function",
                            "function": {"name": name, "arguments": arguments.to_string()},
                        })
                    })
                    .collect::<Vec<_>>();
                json!({"choices": [{"message": {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": calls,
                }}]})
                .to_string()
            }

            fn submit(name: &str, mode: &str, revision: &str) -> String {
                call_reply(name, &json!({"mode": mode, "expected_revision": revision}))
            }

            fn messages(body: &Value) -> &[Value] {
                body["messages"].as_array().map_or(&[], Vec::as_slice)
            }

            fn has_answer(body: &Value) -> bool {
                messages(body).iter().any(|m| m["role"] == "tool")
            }

            fn last_answer(body: &Value) -> Value {
                let text = messages(body)
                    .iter()
                    .rev()
                    .find(|m| m["role"] == "tool")
                    .and_then(|m| m["content"].as_str())
                    .unwrap_or_else(|| panic!("no answer: {body}"));
                serde_json::from_str(text).unwrap_or_else(|_| panic!("not data: {text}"))
            }

            fn revision_below(answer: &Value) -> String {
                answer["stage"]["downstream"][0]["revision"]
                    .as_str()
                    .unwrap_or_else(|| panic!("no revision below: {answer}"))
                    .to_string()
            }

            fn step(instruction: &str, id: &str) -> PlanStep {
                PlanStep {
                    id: id.to_string(),
                    kind: "setup".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: instruction.to_string(),
                    expected_paths: Vec::new(),
                    verify: Vec::new(),
                }
            }

            /// The upper, and the lower on the revision the seal's answer named.
            fn seals_both() -> Responder {
                Arc::new(|index, body| match index {
                    1 => submit(UPPER_TOOL, "fast", "initial"),
                    2 => submit(LOWER_TOOL, "fast", &revision_below(&last_answer(body))),
                    _ => text_reply("done"),
                })
            }

            /// The upper, and then only the claim.
            fn stops_after_the_upper() -> Responder {
                Arc::new(|index, _| match index {
                    1 => submit(UPPER_TOOL, "fast", "initial"),
                    _ => text_reply("All stages are sealed and every check passed."),
                })
            }

            /// A step on the lower stage: its first call is stale and the answer says what to use.
            fn two_steps() -> Responder {
                Arc::new(|index, body| match index {
                    1 => submit(UPPER_TOOL, "fast", "initial"),
                    2 => submit(LOWER_TOOL, "fast", "initial"),
                    _ => {
                        let answer = last_answer(body);
                        match answer["revision"].as_str() {
                            Some(revision) if answer["status"] == "stale" => {
                                submit(LOWER_TOOL, "fast", revision)
                            }
                            _ => text_reply("done"),
                        }
                    }
                })
            }

            fn ultra_steps(submit_lower: bool) -> Responder {
                Arc::new(move |_, body| {
                    let text = body.to_string();
                    if !body["tools"].to_string().contains(UPPER_TOOL) {
                        let goal = if text.contains("Phase-second") {
                            "Submit the lower stage"
                        } else {
                            "Submit the upper stage"
                        };
                        return text_reply(
                            &json!({"goal": goal, "steps": [{
                                "id": "step-setup",
                                "kind": "setup",
                                "expected_result": "pass",
                                "instruction": goal,
                                "expected_paths": [],
                                "verify": [],
                            }]})
                            .to_string(),
                        );
                    }
                    if !text.contains("Submit the lower stage") {
                        return if has_answer(body) {
                            text_reply("done")
                        } else {
                            submit(UPPER_TOOL, "fast", "initial")
                        };
                    }
                    if !submit_lower {
                        let answered = messages(body)
                            .last()
                            .is_some_and(|message| message["role"] == "tool");
                        return if answered {
                            text_reply("done")
                        } else {
                            call_reply("Read", &json!({"path": FIXED}))
                        };
                    }
                    let answer = last_answer(body);
                    if answer["status"] == "stale" {
                        submit(LOWER_TOOL, "fast", answer["revision"].as_str().unwrap())
                    } else if answer["path"] == LOWER_OUTPUT {
                        text_reply("done")
                    } else {
                        submit(LOWER_TOOL, "fast", "initial")
                    }
                })
            }

            fn assert_stage_incomplete<T: std::fmt::Debug>(result: &Result<T, String>) {
                let error = result.as_ref().expect_err("no success with a stage open");
                assert!(error.contains("stage_incomplete"), "{error}");
            }

            /// `--prompt`: every stage sealed is a success. `enter_stage_run` and `scope_of`'s
            /// `All` are what ask, and a set nobody entered would not ask at all.
            #[test]
            fn a_prompt_that_seals_every_stage_succeeds() {
                let Some(world) = one_process("a_prompt_that_seals_every_stage_succeeds") else {
                    return;
                };
                let _live = world.enter();
                let (requests, result) = world.prompt(seals_both());
                result.unwrap();
                assert_eq!(requests.len(), 3);
                assert!(world.root.join(LOWER_OUTPUT).is_file());
            }

            /// The hold sends the claim back, and the funnel refuses what the hold could not mend.
            #[test]
            fn a_prompt_that_leaves_a_stage_open_is_held_and_then_refused() {
                let Some(world) =
                    one_process("a_prompt_that_leaves_a_stage_open_is_held_and_then_refused")
                else {
                    return;
                };
                let _live = world.enter();
                let (requests, result) = world.prompt(stops_after_the_upper());
                assert_stage_incomplete(&result);
                assert!(requests.len() >= 3, "{}", requests.len());
                assert!(
                    requests[2].to_string().contains(HOLD),
                    "the claim was answered: {}",
                    requests[2]
                );
                assert!(world.events().contains("structured_stage_incomplete"));
            }

            /// F5: a setup step can end at a tool result. A failed check must not end it unread.
            #[test]
            fn a_failed_check_in_a_setup_step_goes_back_to_its_model() {
                let Some(world) =
                    one_process("a_failed_check_in_a_setup_step_goes_back_to_its_model")
                else {
                    return;
                };
                let _live = world.enter();
                let marks = world.marks.join(format!("checks-{UPPER}"));
                std::fs::write(&marks, r#"[{"id":"shape","status":"failed"}]"#).unwrap();
                let responder: Responder = Arc::new(move |index, body| match index {
                    1 => submit(UPPER_TOOL, "fast", "initial"),
                    2 => {
                        assert_eq!(last_answer(body)["status"], "resubmit", "{body}");
                        std::fs::remove_file(&marks).unwrap();
                        submit(UPPER_TOOL, "fast", "initial")
                    }
                    _ => text_reply("done"),
                });
                let (requests, _) = world.plan(
                    vec![step("Submit the upper stage", "step-setup")],
                    responder,
                );
                assert!(requests.len() >= 2, "no second request");
                assert!(world.root.join(UPPER_OUTPUT).is_file());
            }

            /// `/plan-run`: the end of a plain step plan answers for what no step called.
            #[test]
            fn a_step_plan_that_leaves_a_stage_open_is_refused_at_its_end() {
                let Some(world) =
                    one_process("a_step_plan_that_leaves_a_stage_open_is_refused_at_its_end")
                else {
                    return;
                };
                let _live = world.enter();
                let responder: Responder = Arc::new(|index, _| match index {
                    1 => submit(UPPER_TOOL, "fast", "initial"),
                    _ => text_reply("done"),
                });
                let (_, result) = world.plan(
                    vec![step("Submit the upper stage", "step-setup")],
                    responder,
                );
                assert_stage_incomplete(&result);
                assert!(result.unwrap_err().contains("lower/main"));
            }

            /// A first step that leaves the lower open is not refused by itself: that is `scope_of`
            /// giving a step `Touched`. The plan's end then finds everything sealed.
            #[test]
            fn a_step_plan_that_seals_every_stage_succeeds() {
                let Some(world) = one_process("a_step_plan_that_seals_every_stage_succeeds") else {
                    return;
                };
                let _live = world.enter();
                let steps = vec![
                    step("Submit the upper stage", "step-upper"),
                    step("Submit the lower stage", "step-lower"),
                ];
                let (_, result) = world.plan(steps, two_steps());
                result.unwrap();
                assert!(world.root.join(LOWER_OUTPUT).is_file());
            }

            /// `remember_bound` and the final gate: a session that left nothing for the gate to
            /// find would fail this control.
            #[test]
            fn an_ultra_plan_that_seals_every_stage_succeeds() {
                let Some(world) = one_process("an_ultra_plan_that_seals_every_stage_succeeds")
                else {
                    return;
                };
                let _live = world.enter();
                let summary = world.ultra(ultra_steps(true)).unwrap();
                assert!(summary.contains("complete"), "{summary}");
                assert!(world.root.join(LOWER_OUTPUT).is_file());
            }

            /// T5 (acceptance 6): a seal stands in for nothing else. Every stage sealed, and the
            /// profile's own final check fails: no success, and no level is reported for one.
            #[test]
            fn sealing_every_stage_does_not_stand_in_for_final_acceptance() {
                let Some(world) =
                    one_process("sealing_every_stage_does_not_stand_in_for_final_acceptance")
                else {
                    return;
                };
                let _live = world.enter();
                world
                    .ultra_with_notes(false, ultra_steps(true))
                    .expect_err("the profile's own file is missing");
                assert!(
                    world.root.join(LOWER_OUTPUT).is_file(),
                    "both stages sealed"
                );
                let events = world.events();
                assert!(
                    !events.contains("\"event\":\"structured_stage_incomplete\""),
                    "the stages are not what refused: {events}"
                );
                let complete = events
                    .lines()
                    .filter(|line| line.contains("\"event\":\"ultra_plan_complete\""))
                    .collect::<Vec<_>>();
                assert!(
                    complete.iter().all(|line| line.contains("\"ok\":false")),
                    "{complete:?}"
                );
            }

            /// Each step passes, and only the final gate sees the stage nobody called.
            #[test]
            fn an_ultra_plan_that_leaves_a_stage_open_fails_at_final_acceptance() {
                let Some(world) =
                    one_process("an_ultra_plan_that_leaves_a_stage_open_fails_at_final_acceptance")
                else {
                    return;
                };
                let _live = world.enter();
                world
                    .ultra(ultra_steps(false))
                    .expect_err("the lower stage was never submitted");
                let events = world.events();
                let failed = events
                    .lines()
                    .find(|line| line.contains("\"event\":\"ultra_final_acceptance_failed\""))
                    .expect("the steps passed and acceptance is what refused");
                assert!(failed.contains("stage_incomplete"), "{failed}");
                assert!(failed.contains("lower/main"), "{failed}");
            }

            /// `config_resuming`: `/resume` in a new process goes on with the run. A start over
            /// would have taken `initial` as current and spent an attempt.
            #[test]
            fn resume_in_a_new_process_goes_on_with_the_run() {
                const NAME: &str = "resume_in_a_new_process_goes_on_with_the_run";
                let Some(base) = world_base() else {
                    let temp = prepared();
                    let base = std::fs::canonicalize(temp.path()).unwrap();
                    for role in ["first", "second"] {
                        let child = run_child(NAME, &base, &[(ROLE_ENV, role)]);
                        assert!(child.success, "{role}: {}", child.stdout);
                    }
                    return;
                };
                let world = World::at(Path::new(&base));
                let _live = world.enter();
                if std::env::var_os(ROLE_ENV).is_some_and(|role| role == "first") {
                    let (_, result) = world.prompt(stops_after_the_upper());
                    assert_stage_incomplete(&result);
                    assert_eq!(saved_attempts(&world), 1);
                    return;
                }
                let plans = world.root.join(".anvil/plans");
                std::fs::create_dir_all(&plans).unwrap();
                std::fs::write(
                    plans.join("recovery.yaml"),
                    format!(
                        "goal: \"finish\"\nprofile: \"{PROFILE}\"\nphases:\n  - id: \"finish\"\n    prompt: \"Submit the upper stage.\"\n  - id: \"after\"\n    prompt: \"Submit the lower stage.\"\n"
                    ),
                )
                .unwrap();
                let http = FakeHttp::start(Arc::new(|_, body| {
                    if !body["tools"].to_string().contains(UPPER_TOOL) {
                        return text_reply(
                            &json!({"goal": "finish", "steps": [{
                                "id": "step-setup",
                                "kind": "setup",
                                "expected_result": "pass",
                                "instruction": "Submit the upper stage",
                                "expected_paths": [],
                                "verify": [],
                            }]})
                            .to_string(),
                        );
                    }
                    if has_answer(body) {
                        return text_reply("stopping here");
                    }
                    submit(UPPER_TOOL, "fast", "initial")
                }));
                let config = world.config(&http.url);
                let mut planner = client_from_config(&config, true).unwrap();
                let mut execution = client_from_config(&config, false).unwrap();
                let result = crate::tui::slash::handle_command(
                    "/resume .anvil/plans/recovery.yaml",
                    &config,
                    planner.as_mut(),
                    execution.as_mut(),
                    &NOOP_UI,
                );
                let requests = http.finish();
                let first = requests
                    .iter()
                    .find(|body| has_answer(body))
                    .map(last_answer)
                    .unwrap_or_else(|| panic!("no answer: {result:?}"));
                assert_eq!(first["status"], "stale", "{first}");
                assert_eq!(saved_attempts(&world), 1, "no attempt came back");
                assert!(result.is_err(), "the obligations are still open");
            }

            fn saved_attempts(world: &World) -> u64 {
                let leaf = world.evidence.join("state/structured-stages");
                let directory = std::fs::read_dir(leaf)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                let saved: Value =
                    serde_json::from_slice(&std::fs::read(directory.join("ledger.json")).unwrap())
                        .unwrap();
                saved["body"]["attempts_used"].as_u64().unwrap()
            }

            /// What the loop's own count of missing-artifact stagnation says after `batches`
            /// replies with a file still missing. Read by the responder, before any later text.
            fn artifact_notices_after(world: &World, batches: Vec<Responder>) -> usize {
                const NOTICE: &str = "\"event\":\"artifact_stagnation_feedback\"";
                let seen = Arc::new(Mutex::new(None));
                let (kept, events) = (seen.clone(), world.evidence.join("events.jsonl"));
                let count = batches.len();
                let before = world.events().matches(NOTICE).count();
                let responder: Responder = Arc::new(move |index, body| {
                    if index > count {
                        let mut kept = kept.lock().unwrap();
                        if kept.is_none() {
                            let text = std::fs::read_to_string(&events).unwrap_or_default();
                            *kept = Some(text.matches(NOTICE).count() - before);
                        }
                        return text_reply("stopping");
                    }
                    batches[index - 1](index, body)
                });
                let http = FakeHttp::start(responder);
                let config = world.config(&http.url);
                let mut client = client_from_config(&config, false).unwrap();
                let _ = run_session_with_outcome_with_ui(
                    client.as_mut(),
                    &mut SessionSnapshot::new(),
                    "Submit the stage and write the notes.",
                    &["notes.txt".to_string()],
                    &config,
                    &NOOP_UI,
                );
                http.finish();
                seen.lock().unwrap().expect("the session went on")
            }

            /// N5: each seal is a move, so three of them are no stagnation. The control below is
            /// the same three batches with no seal in them.
            #[test]
            fn seals_are_progress_to_the_loop_and_stale_calls_are_not() {
                let Some(world) =
                    one_process("seals_are_progress_to_the_loop_and_stale_calls_are_not")
                else {
                    return;
                };
                let _live = world.enter();
                let stale: Responder =
                    Arc::new(|_, _| submit(UPPER_TOOL, "fast", "not-a-revision"));
                let stalled =
                    artifact_notices_after(&world, vec![stale.clone(), stale.clone(), stale]);
                assert!(stalled >= 1, "the control sees nothing: {}", world.events());

                let second = world.marks.join(format!("second-{UPPER}"));
                let seal: Responder = Arc::new(move |index, body| {
                    if index == 1 {
                        return submit(UPPER_TOOL, "fast", "initial");
                    }
                    let current = last_answer(body)["stage"]["revision"]
                        .as_str()
                        .unwrap()
                        .to_string();
                    // Other bytes each time, so each is a commit and a seal.
                    if index == 2 {
                        std::fs::write(&second, "on").unwrap();
                    } else {
                        std::fs::remove_file(&second).unwrap();
                    }
                    submit(UPPER_TOOL, &format!("version-{index}"), &current)
                });
                let sealed = artifact_notices_after(&world, vec![seal.clone(), seal.clone(), seal]);
                assert_eq!(
                    sealed,
                    0,
                    "a seal counted as stagnation: {}",
                    world.events()
                );
            }

            /// The record's notice shows once however often the record is asked about, and a
            /// stderr that is no terminal gets plain lines: no return, no escape.
            #[test]
            fn the_notice_shows_once_and_as_plain_lines_where_there_is_no_terminal() {
                const NAME: &str =
                    "the_notice_shows_once_and_as_plain_lines_where_there_is_no_terminal";
                let Some(base) = world_base() else {
                    let plain: &[(&str, &str)] = &[];
                    let no_interrupt: &[(&str, &str)] = &[("COMMANDAGENT_NO_INTERRUPT", "1")];
                    for (env, to_a_pipe) in [(plain, false), (plain, true), (no_interrupt, false)] {
                        let temp = prepared();
                        let base = std::fs::canonicalize(temp.path()).unwrap();
                        let child = run_child_writing(NAME, &base, env, to_a_pipe);
                        assert!(child.success, "{}", child.stdout);
                        let stderr = String::from_utf8(child.stderr).unwrap();
                        assert_eq!(stderr.matches("stage record refused: corrupt").count(), 1);
                        assert!(!stderr.contains('\r'), "{stderr:?}");
                        assert!(!stderr.contains('\x1b'), "{stderr:?}");
                        let lines = stderr
                            .lines()
                            .skip_while(|line| !line.starts_with("stage record refused:"))
                            .collect::<Vec<_>>();
                        assert!(lines[1].starts_with("  ledger: "), "{stderr:?}");
                        assert!(lines[2].starts_with("  to recover: "), "{stderr:?}");
                    }
                    return;
                };
                let world = World::at(Path::new(&base));
                let _live = world.enter();
                let (_, result) = world.prompt(seals_both());
                result.unwrap();
                let file = crate::planner::structured_stages::ledger::ledger_file(
                    &world.evidence.join("state"),
                    &world.root,
                    PROFILE,
                )
                .unwrap();
                std::fs::write(file, b"not a record").unwrap();
                let config = world.config("http://127.0.0.1:9");
                for _ in 0..2 {
                    let failure =
                        crate::planner::structured_stages::final_gate_failure(&config).unwrap();
                    assert!(failure.contains("ledger_refused"), "{failure}");
                }
            }

            /// Acceptance 1: a settings object and a document index go through the same machine,
            /// and what was adopted is what the receipt and the record name.
            #[test]
            fn a_settings_object_and_a_document_index_walk_the_same_machine() {
                let Some(world) =
                    one_process("a_settings_object_and_a_document_index_walk_the_same_machine")
                else {
                    return;
                };
                let _live = world.enter();
                let run = |profile: &str, upper: &'static str, lower: &'static str| {
                    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
                    let kept = seen.clone();
                    let http = FakeHttp::start(Arc::new(move |index, body| match index {
                        1 => submit(upper, "fast", "initial"),
                        2 => {
                            let answer = last_answer(body);
                            kept.lock().unwrap().push(answer.clone());
                            submit(lower, "fast", &revision_below(&answer))
                        }
                        3 => {
                            kept.lock().unwrap().push(last_answer(body));
                            text_reply("done")
                        }
                        _ => text_reply("done"),
                    }));
                    let mut config = world.config(&http.url);
                    config.profile = profile.to_string();
                    let mut client = client_from_config(&config, false).unwrap();
                    run_session_with_outcome_with_ui(
                        client.as_mut(),
                        &mut SessionSnapshot::new(),
                        "Submit both stages.",
                        &[],
                        &config,
                        &NOOP_UI,
                    )
                    .unwrap();
                    http.finish();
                    seen.lock().unwrap().clone()
                };
                for answers in [
                    run(PROFILE, UPPER_TOOL, LOWER_TOOL),
                    run(INDEX_PROFILE, "ProposeIndexUpper", "ProposeIndexLower"),
                ] {
                    assert_eq!(answers.len(), 2);
                    for answer in answers {
                        assert_eq!(answer["stage"]["status"], "sealed", "{answer}");
                        let path = answer["path"].as_str().unwrap();
                        let bytes = std::fs::read(world.root.join(path)).unwrap();
                        assert_eq!(answer["committed_sha256"], sha256(&bytes), "{answer}");
                    }
                }
                assert!(world.root.join("index/upper.json").is_file());
                assert!(world.root.join("index/lower.json").is_file());
                // A record for each profile, and each names what it adopted.
                let leaf = world.evidence.join("state/structured-stages");
                assert_eq!(std::fs::read_dir(leaf).unwrap().count(), 2);
            }

            /// A run resumed in a new process has its seals lowered with the bytes unchanged, and
            /// final acceptance does not take them as met. Whatever its repair then does, this is
            /// no success.
            #[test]
            fn seals_a_resume_lowered_are_not_met_at_final_acceptance() {
                const NAME: &str = "seals_a_resume_lowered_are_not_met_at_final_acceptance";
                let Some(base) = world_base() else {
                    let temp = prepared();
                    let base = std::fs::canonicalize(temp.path()).unwrap();
                    for role in ["first", "second"] {
                        let child = run_child(NAME, &base, &[(ROLE_ENV, role)]);
                        assert!(
                            child.success,
                            "{role}: {}",
                            String::from_utf8_lossy(&child.stderr)
                        );
                    }
                    return;
                };
                let world = World::at(Path::new(&base));
                let _live = world.enter();
                if std::env::var_os(ROLE_ENV).is_some_and(|role| role == "first") {
                    let (_, result) = world.prompt(seals_both());
                    result.unwrap();
                    return;
                }
                std::fs::write(world.root.join("notes.txt"), "the notes\n").unwrap();
                let http = FakeHttp::start(Arc::new(|_, body| {
                    if !body["tools"].to_string().contains(UPPER_TOOL) {
                        let goal = "Look at the seed";
                        return text_reply(
                            &json!({"goal": goal, "steps": [{
                                "id": "step-inspect",
                                "kind": "setup",
                                "expected_result": "pass",
                                "instruction": goal,
                                "expected_paths": [],
                                "verify": [],
                            }]})
                            .to_string(),
                        );
                    }
                    if !body
                        .to_string()
                        .contains("Repair the final acceptance failure")
                    {
                        let answered = messages(body)
                            .last()
                            .is_some_and(|message| message["role"] == "tool");
                        return if answered {
                            text_reply("done")
                        } else {
                            call_reply("Read", &json!({"path": FIXED}))
                        };
                    }
                    let Some(last) = messages(body).last().filter(|m| m["role"] == "tool") else {
                        return submit(UPPER_TOOL, "fast", "initial");
                    };
                    let answer: Value =
                        serde_json::from_str(last["content"].as_str().unwrap_or("{}"))
                            .unwrap_or_default();
                    match (answer["status"].as_str(), answer["path"].as_str()) {
                        (Some("stale"), _) if answer["revision"].is_string() => {
                            let tool = if body.to_string().contains("\"path\":\"out/upper.json\"") {
                                LOWER_TOOL
                            } else {
                                UPPER_TOOL
                            };
                            submit(tool, "fast", answer["revision"].as_str().unwrap())
                        }
                        (_, Some(UPPER_OUTPUT)) => submit(LOWER_TOOL, "fast", "initial"),
                        _ => text_reply("done"),
                    }
                }));
                let mut config = world.config(&http.url);
                config.resume = Some("a-saved-run".to_string());
                let mut planner = client_from_config(&config, true).unwrap();
                let mut execution = client_from_config(&config, false).unwrap();
                let plan = UltraPlan {
                    goal: "Look again".to_string(),
                    profile: PROFILE.to_string(),
                    style: "default".to_string(),
                    intent: "create".to_string(),
                    phases: vec![
                        UltraPhase {
                            id: "first".to_string(),
                            prompt: "Phase-first: look at the seed.".to_string(),
                        },
                        UltraPhase {
                            id: "second".to_string(),
                            prompt: "Phase-second: look at the seed again.".to_string(),
                        },
                    ],
                };
                let result = run_ultra_plan(planner.as_mut(), execution.as_mut(), &plan, &config);
                http.finish();
                result.expect_err("a seal a resume lowered is not a seal");
                let events = world.events();
                let unmet = events
                    .lines()
                    .find(|line| line.contains("\"event\":\"structured_stage_incomplete\""))
                    .expect("final acceptance asked");
                assert!(unmet.contains("\"phase\":\"unverified\""), "{unmet}");
                assert!(events.contains("\"event\":\"final_acceptance_repair_start\""));
            }

            /// M17: a pack's declaration counts too, for the run where no session ever bound.
            #[test]
            fn final_acceptance_finds_stages_a_selected_pack_declares() {
                use super::super::super::super::selection::{
                    self, PackSelection, ResolvedDeclaration,
                };
                let Some(world) =
                    one_process("final_acceptance_finds_stages_a_selected_pack_declares")
                else {
                    return;
                };
                let _live = world.enter();
                assert!(!selection::stages_declared_without_a_read("generic"));
                let chosen = PackSelection {
                    directory: world.base.join("pack"),
                    id: "a-pack".to_string(),
                    version: "1.0.0".to_string(),
                    hash: "sha256:none".to_string(),
                };
                // SAFETY: this process runs this one test, on this one thread.
                unsafe {
                    std::env::set_var("COMMANDAGENT_PACK_DIRECTORY", &chosen.directory);
                    std::env::set_var("COMMANDAGENT_PACK_ID", &chosen.id);
                    std::env::set_var("COMMANDAGENT_PACK_VERSION", &chosen.version);
                    std::env::set_var("COMMANDAGENT_PACK_HASH", &chosen.hash);
                }
                let declared = crate::planner::extension_profiles::find(PROFILE)
                    .and_then(|_| selection::manifest_declaration_for_test(PROFILE))
                    .unwrap();
                let pack_profile = crate::planner::pack::PackProfile::PythonCli;
                let _installed = selection::install_declaration(
                    chosen,
                    Some(ResolvedDeclaration {
                        pack_profile,
                        tools: Some(declared),
                    }),
                );
                assert!(selection::stages_declared_without_a_read("generic"));
            }

            /// B1: what a run after a pack's run finds. The profile declares nothing, so whatever
            /// stages a run has come from the pack it selected.
            const BARE_PROFILE: &str = "stage-session-bare";

            /// How the selection was installed: `--pack` decodes the pack first, a confirmed TUI
            /// dispatch (`with_confirmed_pack`) leaves that to admission.
            #[derive(Clone, Copy, PartialEq)]
            enum Installed {
                Decoded,
                LeftToAdmission,
            }

            impl World {
                fn with_bare_profile(self) -> Self {
                    let whole = manifest(BARE_PROFILE, UPPER, LOWER);
                    let bare = whole.split("[profile_tools]").next().unwrap();
                    let profile = self.base.join(format!("extension/profiles/{BARE_PROFILE}"));
                    std::fs::create_dir_all(&profile).unwrap();
                    std::fs::write(profile.join("manifest.toml"), bare).unwrap();
                    self
                }

                fn plan_bare(
                    &self,
                    steps: Vec<PlanStep>,
                    responder: Responder,
                ) -> Result<String, String> {
                    self.plan_as(BARE_PROFILE, steps, responder)
                }

                fn plan_as(
                    &self,
                    profile: &str,
                    steps: Vec<PlanStep>,
                    responder: Responder,
                ) -> Result<String, String> {
                    let http = FakeHttp::start(responder);
                    let mut config = self.config(&http.url);
                    config.profile = profile.to_string();
                    let mut client = client_from_config(&config, false).unwrap();
                    let plan = StepPlan {
                        goal: "Look at the seed.".to_string(),
                        steps,
                    };
                    let result = run_step_plan(client.as_mut(), &plan, &config)
                        .map_err(|error| format!("{error:#}"));
                    http.finish();
                    result
                }

                fn stage_pack(
                    &self,
                    id: &str,
                    tools: (&str, &str),
                    installed: Installed,
                ) -> crate::cli_pack::ResolvedPack {
                    let (upper, lower) = tools;
                    let declaration = format!(
                        "profile_tools:\n  schema_version: profile-tool.declaration/v2\n  builtin_tools: [Read]\n  tools:\n    - id: {upper}\n      effect: propose-artifacts\n    - id: {lower}\n      effect: propose-artifacts\n  stages:\n    stages:\n      - id: upper\n        targets:\n          - id: main\n            tool_id: {upper}\n      - id: lower\n        targets:\n          - id: main\n            tool_id: {lower}\n"
                    );
                    self.pack_declaring(id, &declaration, installed)
                }

                /// A pack with no stage, as a confirmed dispatch installs it. Its admission comes to
                /// nothing, or to a set of tools with no stage: `session_registry` has an arm for each.
                fn packs_with_no_stage(&self, id: &str) -> [crate::cli_pack::ResolvedPack; 2] {
                    [
                        self.pack_declaring(
                            &format!("{id}-checks"),
                            "checks:\n  - id: command_check\n    at:\n      kind: final_acceptance\n    params:\n      argv: [test, -f, fixed/seed.json]\n      cwd: workspace\n      expect:\n        exit_code: 0\n        stdout_regex: '^$'\n        max_bytes: 1024\n",
                            Installed::LeftToAdmission,
                        ),
                        self.pack_declaring(
                            &format!("{id}-tools"),
                            "profile_tools:\n  schema_version: profile-tool.declaration/v1\n  builtin_tools: [Read]\n",
                            Installed::LeftToAdmission,
                        ),
                    ]
                }

                fn pack_declaring(
                    &self,
                    id: &str,
                    declaration: &str,
                    installed: Installed,
                ) -> crate::cli_pack::ResolvedPack {
                    self.pack_for(BARE_PROFILE, id, declaration, installed)
                }

                fn pack_for(
                    &self,
                    profile: &str,
                    id: &str,
                    declaration: &str,
                    installed: Installed,
                ) -> crate::cli_pack::ResolvedPack {
                    let directory = self.base.join("packs").join(id).join("1.0.0");
                    std::fs::create_dir_all(&directory).unwrap();
                    let eval = format!(
                        "schema_version: commandagent.pack.eval/v0\npack:\n  id: {id}\n  version: 1.0.0\n  profile: {profile}\n  intent: create\n{declaration}"
                    );
                    std::fs::write(directory.join("eval.yaml"), &eval).unwrap();
                    let hash = crate::planner::pack::exact_byte_hash(None, Some(eval.as_bytes()));
                    std::fs::write(directory.join("pack.sha256"), format!("{hash}\n")).unwrap();
                    let declared = (installed == Installed::Decoded).then(|| {
                        let loaded =
                            crate::planner::pack::load_declared_directory(&directory).unwrap();
                        crate::tools::profile_tools::selection::ResolvedDeclaration {
                            pack_profile: loaded.pack.identity.profile,
                            tools: loaded.profile_tools,
                        }
                    });
                    crate::cli_pack::ResolvedPack {
                        id: id.to_string(),
                        version: "1.0.0".to_string(),
                        hash,
                        source: crate::cli_pack::SelectionSource::Repository,
                        directory,
                        declared,
                    }
                }

                fn stage_events(&self) -> usize {
                    self.events()
                        .matches("\"event\":\"structured_stage_")
                        .count()
                }
            }

            /// A step that reads a file and stops: it calls no stage.
            fn reads_and_stops() -> Responder {
                Arc::new(|_, body| {
                    let answered = messages(body)
                        .last()
                        .is_some_and(|message| message["role"] == "tool");
                    if answered {
                        text_reply("done")
                    } else {
                        call_reply("Read", &json!({"path": FIXED}))
                    }
                })
            }

            fn looks() -> Vec<PlanStep> {
                vec![step("Look at the seed", "step-look")]
            }

            /// The pack's run ends with its lower stage open.
            fn leave_the_lower_open(world: &World, pack: &crate::cli_pack::ResolvedPack) {
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(pack)).unwrap();
                let upper = vec![step("Submit the upper stage", "step-upper")];
                let first = world.plan_bare(upper, stops_after_the_upper());
                assert_stage_incomplete(&first);
            }

            /// The control: the same run with no pack before it, on the same fixture.
            #[test]
            fn b1_control_a_plan_with_no_stage_alone_succeeds() {
                let Some(world) = one_process("b1_control_a_plan_with_no_stage_alone_succeeds")
                else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let result = world.plan_bare(looks(), reads_and_stops());
                assert!(result.is_ok(), "{result:?}");
                assert_eq!(world.stage_events(), 0, "{}", world.events());
            }

            /// `--pack`: the guard goes, and with it the run's stages.
            #[test]
            fn b1_a_plan_with_no_stage_succeeds_after_a_pack_left_a_stage_open() {
                let Some(world) =
                    one_process("b1_a_plan_with_no_stage_succeeds_after_a_pack_left_a_stage_open")
                else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack = world.stage_pack("stage-pack-first", (UPPER, LOWER), Installed::Decoded);
                leave_the_lower_open(&world, &pack);
                let before = world.stage_events();
                let second = world.plan_bare(looks(), reads_and_stops());
                assert_eq!(
                    (second.is_ok(), world.stage_events() - before),
                    (true, 0),
                    "{second:?}"
                );
            }

            /// The path of `with_confirmed_pack`: the guard with nothing decoded, then no pack.
            #[test]
            fn b1_a_plan_with_no_stage_succeeds_after_a_confirmed_pack_left_a_stage_open() {
                const NAME: &str =
                    "b1_a_plan_with_no_stage_succeeds_after_a_confirmed_pack_left_a_stage_open";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack = world.stage_pack(
                    "stage-pack-confirmed",
                    (UPPER, LOWER),
                    Installed::LeftToAdmission,
                );
                leave_the_lower_open(&world, &pack);
                let before = world.stage_events();
                let second = world.plan_bare(looks(), reads_and_stops());
                assert_eq!(
                    (second.is_ok(), world.stage_events() - before),
                    (true, 0),
                    "{second:?}"
                );
            }

            /// A record the run with no stage has no business with: damaged, and it does not care.
            #[test]
            fn b1_a_plan_with_no_stage_reads_no_record_of_the_pack_before_it() {
                let Some(world) =
                    one_process("b1_a_plan_with_no_stage_reads_no_record_of_the_pack_before_it")
                else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack =
                    world.stage_pack("stage-pack-record", (UPPER, LOWER), Installed::Decoded);
                leave_the_lower_open(&world, &pack);
                let file = crate::planner::structured_stages::ledger::ledger_file(
                    &world.evidence.join("state"),
                    &world.root,
                    BARE_PROFILE,
                )
                .unwrap();
                assert!(file.is_file(), "the pack's run left a record");
                std::fs::write(&file, b"a record damaged on purpose").unwrap();
                let before = world.stage_events();
                let second = world.plan_bare(looks(), reads_and_stops());
                assert_eq!(
                    (second.is_ok(), world.stage_events() - before),
                    (true, 0),
                    "{second:?}"
                );
                assert_eq!(
                    std::fs::read(&file).unwrap(),
                    b"a record damaged on purpose"
                );
            }

            /// Pack A sealed everything. Pack B declares other stages and no session bound them.
            #[test]
            fn b1_a_second_pack_with_no_session_is_not_passed_by_the_first_packs_seals() {
                const NAME: &str =
                    "b1_a_second_pack_with_no_session_is_not_passed_by_the_first_packs_seals";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let first =
                    world.stage_pack("stage-pack-sealed", (UPPER, LOWER), Installed::Decoded);
                let second = world.stage_pack(
                    "stage-pack-unbound",
                    ("stage-index-upper", "stage-index-lower"),
                    Installed::Decoded,
                );
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&first)).unwrap();
                    let steps = vec![
                        step("Submit the upper stage", "step-upper"),
                        step("Submit the lower stage", "step-lower"),
                    ];
                    world.plan_bare(steps, two_steps()).unwrap();
                }
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&second)).unwrap();
                    let result = world.plan_bare(Vec::new(), Arc::new(|_, _| text_reply("nobody")));
                    let error = result.expect_err("no session bound the second pack's stages");
                    assert!(
                        error.contains("stage_incomplete: no_session_bound_the_declared_stages"),
                        "{error}"
                    );
                }
                // The second pack's failure takes nothing from the first.
                let _pack =
                    crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&first)).unwrap();
                let again = world.plan_bare(Vec::new(), Arc::new(|_, _| text_reply("nobody")));
                assert!(again.is_ok(), "{again:?}");
            }

            /// Pack A left a stage open, and pack B's session calls its own stage.
            #[test]
            fn b1_a_second_packs_session_is_measured_after_the_first_packs_record() {
                const NAME: &str =
                    "b1_a_second_packs_session_is_measured_after_the_first_packs_record";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let first =
                    world.stage_pack("stage-pack-earlier", (UPPER, LOWER), Installed::Decoded);
                let second = world.stage_pack(
                    "stage-pack-later",
                    ("stage-index-upper", "stage-index-lower"),
                    Installed::Decoded,
                );
                leave_the_lower_open(&world, &first);
                let _pack =
                    crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&second)).unwrap();
                let steps = vec![step("Submit the upper stage", "step-upper")];
                let result = world.plan_bare(
                    steps,
                    Arc::new(|index, _| match index {
                        1 => submit("ProposeIndexUpper", "fast", "initial"),
                        _ => text_reply("done"),
                    }),
                );
                let error = result.expect_err("the process holds the first pack's record");
                assert!(error.contains("stage_ledger_refused"), "{error}");
                assert!(error.contains("contract_changed"), "{error}");
            }

            /// The pack's session bound stages, and what is in memory says none are declared: only
            /// the bound run keeps this from a success, so "not declared in memory" cannot be off.
            #[test]
            fn b1_a_confirmed_packs_stages_are_not_known_without_a_read() {
                const NAME: &str = "b1_a_confirmed_packs_stages_are_not_known_without_a_read";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack = world.stage_pack(
                    "stage-pack-unknown",
                    (UPPER, LOWER),
                    Installed::LeftToAdmission,
                );
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                let upper = vec![step("Submit the upper stage", "step-upper")];
                assert_stage_incomplete(&world.plan_bare(upper, stops_after_the_upper()));
                assert!(
                    !crate::tools::profile_tools::selection::stages_declared_without_a_read(
                        BARE_PROFILE
                    ),
                    "the candidate would answer `None` here"
                );
                let mut config = world.config("http://127.0.0.1:9");
                config.profile = BARE_PROFILE.to_string();
                let failure = crate::planner::structured_stages::final_gate_failure(&config)
                    .expect("the open stage is what refuses");
                assert!(failure.contains("lower/main"), "{failure}");
            }

            /// A confirmed pack whose run has no session: nothing bound, nothing in memory.
            #[test]
            fn b1_a_confirmed_pack_with_no_session_is_no_success() {
                let Some(world) = one_process("b1_a_confirmed_pack_with_no_session_is_no_success")
                else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack = world.stage_pack(
                    "stage-pack-no-session",
                    (UPPER, LOWER),
                    Installed::LeftToAdmission,
                );
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                let result = world.plan_bare(Vec::new(), Arc::new(|_, _| text_reply("nobody")));
                let error = result.expect_err("the pack declares stages");
                assert!(error.contains("stage_incomplete"), "{error}");
            }

            /// The table beside `BOUND_RUNS`: a pack with the same tools and no stage, after a pack
            /// whose stage fed one of them.
            #[test]
            fn b1_a_pack_with_the_same_tools_and_no_stage_is_admitted_after_a_stage_pack() {
                const NAME: &str =
                    "b1_a_pack_with_the_same_tools_and_no_stage_is_admitted_after_a_stage_pack";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let first =
                    world.stage_pack("stage-pack-feeds", (UPPER, LOWER), Installed::Decoded);
                leave_the_lower_open(&world, &first);
                let plain = world.pack_declaring(
                    "stage-pack-plain",
                    &format!(
                        "profile_tools:\n  schema_version: profile-tool.declaration/v1\n  builtin_tools: [Read]\n  tools:\n    - id: {UPPER}\n      effect: propose-artifacts\n    - id: {LOWER}\n      effect: propose-artifacts\n"
                    ),
                    Installed::LeftToAdmission,
                );
                let _pack =
                    crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&plain)).unwrap();
                let result = world.plan_bare(looks(), reads_and_stops());
                assert!(result.is_ok(), "{result:?}");
            }

            /// A run with no stage neither makes the directory the stages keep their records in nor
            /// looks at what is there.
            #[test]
            fn b1_off_makes_no_stage_directory_and_does_not_read_one() {
                let Some(world) =
                    one_process("b1_off_makes_no_stage_directory_and_does_not_read_one")
                else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let directory = world.evidence.join("state/structured-stages");
                let first = world.plan_bare(looks(), reads_and_stops());
                assert!(first.is_ok(), "{first:?}");
                assert!(!directory.exists(), "an off run made the stages' directory");
                unreadable_in_the_way(&directory);
                let second = world.plan_bare(looks(), reads_and_stops());
                assert!(second.is_ok(), "{second:?}");
                assert_eq!(world.stage_events(), 0, "{}", world.events());
                assert!(directory.is_file(), "left as it was put there");
            }

            /// The same, with a pack's records already in the process and on the disk.
            #[test]
            fn b1_off_after_a_pack_does_not_read_an_unreadable_stage_directory() {
                const NAME: &str =
                    "b1_off_after_a_pack_does_not_read_an_unreadable_stage_directory";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack = world.stage_pack("stage-pack-moved", (UPPER, LOWER), Installed::Decoded);
                leave_the_lower_open(&world, &pack);
                let directory = world.evidence.join("state/structured-stages");
                std::fs::rename(&directory, world.evidence.join("state/moved-away")).unwrap();
                unreadable_in_the_way(&directory);
                let before = world.stage_events();
                let second = world.plan_bare(looks(), reads_and_stops());
                assert_eq!(
                    (second.is_ok(), world.stage_events() - before),
                    (true, 0),
                    "{second:?}"
                );
                assert!(directory.is_file(), "left as it was put there");
            }

            /// A file no one may read, where a directory would go.
            fn unreadable_in_the_way(path: &Path) {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, "not a directory\n").unwrap();
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
            }

            /// A confirmed pack with no stage: its session read it, so the gate does not.
            #[test]
            fn b1_a_confirmed_pack_with_no_stage_is_not_read_again_after_its_session() {
                const NAME: &str =
                    "b1_a_confirmed_pack_with_no_stage_is_not_read_again_after_its_session";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let mut config = world.config("http://127.0.0.1:9");
                config.profile = BARE_PROFILE.to_string();
                for pack in world.packs_with_no_stage("stage-pack-read-once") {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                    let result = world.plan_bare(looks(), reads_and_stops());
                    assert!(result.is_ok(), "{}: {result:?}", pack.id);
                    std::fs::remove_file(pack.directory.join("eval.yaml")).unwrap();
                    assert_eq!(
                        crate::planner::structured_stages::final_gate_failure(&config),
                        None,
                        "{}",
                        pack.id
                    );
                }
                assert_eq!(world.stage_events(), 0, "{}", world.events());
            }

            /// The control: with no session of it, the gate is what reads the pack.
            #[test]
            fn b1_a_confirmed_pack_with_no_stage_is_read_when_no_session_ran() {
                let Some(world) =
                    one_process("b1_a_confirmed_pack_with_no_stage_is_read_when_no_session_ran")
                else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let mut config = world.config("http://127.0.0.1:9");
                config.profile = BARE_PROFILE.to_string();
                for pack in world.packs_with_no_stage("stage-pack-read-by-gate") {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                    assert_eq!(
                        crate::planner::structured_stages::final_gate_failure(&config),
                        None,
                        "{}: it reads the pack, and there is no stage in it",
                        pack.id
                    );
                    std::fs::remove_file(pack.directory.join("eval.yaml")).unwrap();
                    assert_eq!(
                        crate::planner::structured_stages::final_gate_failure(&config).as_deref(),
                        Some("stage_incomplete: the_selected_pack_cannot_be_read"),
                        "{}: a pack it cannot read does not turn a declaration off",
                        pack.id
                    );
                }
            }

            /// Where a real entry runs no session: every step of a saved plan already passes. A
            /// pack's stages are not met by that, however it was installed.
            #[test]
            fn b1_a_plan_whose_steps_all_short_circuit_runs_no_session_and_is_no_success() {
                const NAME: &str =
                    "b1_a_plan_whose_steps_all_short_circuit_runs_no_session_and_is_no_success";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                std::fs::write(world.root.join("done.txt"), "already there\n").unwrap();
                let verified = || PlanStep {
                    id: "verify-existing".to_string(),
                    kind: "verify".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: "Verify done.txt exists".to_string(),
                    expected_paths: vec!["done.txt".to_string()],
                    verify: vec!["test -f done.txt".to_string()],
                };
                for (id, installed) in [
                    ("stage-pack-short-decoded", Installed::Decoded),
                    ("stage-pack-short-confirmed", Installed::LeftToAdmission),
                ] {
                    let pack = world.stage_pack(id, (UPPER, LOWER), installed);
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                    let before = world.events().matches("step_short_circuited").count();
                    let result = world.plan_bare(
                        vec![verified()],
                        Arc::new(|_, _| panic!("a step asked the provider")),
                    );
                    let error = result.expect_err("the pack declares stages");
                    assert!(
                        error.contains("stage_incomplete: no_session_bound_the_declared_stages"),
                        "{id}: {error}"
                    );
                    assert_eq!(
                        world.events().matches("step_short_circuited").count() - before,
                        1,
                        "{id}"
                    );
                }
                // And a pack with no stage is still a success, read only because no session did.
                for pack in world.packs_with_no_stage("stage-pack-short-plain") {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                    let plain = world.plan_bare(
                        vec![verified()],
                        Arc::new(|_, _| panic!("a step asked the provider")),
                    );
                    assert!(plain.is_ok(), "{}: {plain:?}", pack.id);
                }
            }

            /// A, a run with no stage, A again: the first A's open stage is still open, and what it
            /// sealed is still sealed.
            #[test]
            fn b1_a_pack_again_after_a_run_with_no_stage_still_has_its_stage_open() {
                const NAME: &str =
                    "b1_a_pack_again_after_a_run_with_no_stage_still_has_its_stage_open";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack =
                    world.stage_pack("stage-pack-return", (UPPER, LOWER), Installed::Decoded);
                leave_the_lower_open(&world, &pack);
                let between = world.plan_bare(looks(), reads_and_stops());
                assert!(between.is_ok(), "{between:?}");
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                let again = world.plan_bare(Vec::new(), Arc::new(|_, _| text_reply("nobody")));
                let error = again.expect_err("its lower stage is still open");
                assert!(error.contains("1 unmet: lower/main"), "{error}");
                let session = world.plan_bare(looks(), reads_and_stops());
                let error = session.expect_err("a later session of it asks the same");
                assert!(error.contains("1 unmet: lower/main"), "{error}");
            }

            /// What a changed output took stays taken across a run with no stage and a later
            /// session, with the bytes put back: no seal comes back by itself.
            #[test]
            fn b1_a_seal_a_changed_output_took_stays_taken_in_a_later_session() {
                const NAME: &str = "b1_a_seal_a_changed_output_took_stays_taken_in_a_later_session";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack = world.stage_pack("stage-pack-lost", (UPPER, LOWER), Installed::Decoded);
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                    let steps = vec![
                        step("Submit the upper stage", "step-upper"),
                        step("Submit the lower stage", "step-lower"),
                    ];
                    world.plan_bare(steps, two_steps()).unwrap();
                }
                let output = world.root.join(LOWER_OUTPUT);
                let sealed = std::fs::read(&output).unwrap();
                std::fs::write(&output, "changed after the seal\n").unwrap();
                let between = world.plan_bare(looks(), reads_and_stops());
                assert!(between.is_ok(), "the run with no stage: {between:?}");
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                let session = world.plan_bare(looks(), reads_and_stops());
                let error = session.expect_err("the seal no longer stands for these bytes");
                assert!(error.contains("lower/main"), "{error}");
                let events = world.events();
                let unmet = events
                    .lines()
                    .rfind(|line| line.contains("\"event\":\"structured_stage_incomplete\""))
                    .expect("the gate asked");
                assert!(unmet.contains("\"phase\":\"unverified\""), "{unmet}");
                std::fs::write(&output, sealed).unwrap();
                let restored = world.plan_bare(looks(), reads_and_stops());
                let error = restored.expect_err("the same bytes bring no seal back");
                assert!(error.contains("lower/main"), "{error}");
            }

            /// A tool whose upstream inputs differ in another stage set: the admission lets it in on
            /// an adoption of its own, and its first stage call stops at the record. No process
            /// starts, nothing is sealed, and the first set is left as it was.
            #[test]
            fn b1_another_upstream_set_stops_at_contract_changed_and_starts_no_process() {
                const NAME: &str =
                    "b1_another_upstream_set_stops_at_contract_changed_and_starts_no_process";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let starts = |tool: &str| {
                    std::fs::read_to_string(world.marks.join(format!("started-{tool}")))
                        .map_or(0, |marks| marks.lines().count())
                };
                let first =
                    world.stage_pack("stage-pack-chained", (UPPER, LOWER), Installed::Decoded);
                // The lower tool alone: what fed it as a stage's output is a fixed input here.
                let second = world.pack_declaring(
                    "stage-pack-lower-alone",
                    &format!(
                        "profile_tools:\n  schema_version: profile-tool.declaration/v2\n  builtin_tools: [Read]\n  tools:\n    - id: {LOWER}\n      effect: propose-artifacts\n  stages:\n    stages:\n      - id: lower\n        targets:\n          - id: main\n            tool_id: {LOWER}\n"
                    ),
                    Installed::Decoded,
                );
                leave_the_lower_open(&world, &first);
                assert_eq!(starts(UPPER), 1);
                let file = crate::planner::structured_stages::ledger::ledger_file(
                    &world.evidence.join("state"),
                    &world.root,
                    BARE_PROFILE,
                )
                .unwrap();
                let record = std::fs::read(&file).unwrap();
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&second)).unwrap();
                    let steps = vec![step("Submit the lower stage", "step-lower")];
                    let called = Arc::new(AtomicBool::new(false));
                    let saw = called.clone();
                    let result = world.plan_bare(
                        steps,
                        Arc::new(move |index, _| {
                            if index == 1 {
                                saw.store(true, Ordering::SeqCst);
                                submit(LOWER_TOOL, "fast", "initial")
                            } else {
                                text_reply("done")
                            }
                        }),
                    );
                    assert!(called.load(Ordering::SeqCst), "the session did start");
                    let error = result.expect_err("the record is of another declaration");
                    assert!(error.contains("stage_ledger_refused"), "{error}");
                    assert!(error.contains("contract_changed"), "{error}");
                }
                assert_eq!(starts(LOWER), 0, "no process for a call the record refused");
                assert_eq!(starts(UPPER), 1);
                assert!(!world.root.join(LOWER_OUTPUT).exists(), "nothing committed");
                assert_eq!(std::fs::read(&file).unwrap(), record, "nothing recorded");
                let _pack =
                    crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&first)).unwrap();
                let again = world.plan_bare(Vec::new(), Arc::new(|_, _| text_reply("nobody")));
                let error = again.expect_err("the first set's lower stage is still open");
                assert!(error.contains("1 unmet: lower/main"), "{error}");
            }

            /// A pack whose value nothing decoded after another pack: neither reads the other's.
            #[test]
            fn b1_a_second_confirmed_pack_with_no_session_is_read_and_not_passed() {
                const NAME: &str =
                    "b1_a_second_confirmed_pack_with_no_session_is_read_and_not_passed";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let first = world.stage_pack(
                    "stage-pack-sealed-first",
                    (UPPER, LOWER),
                    Installed::Decoded,
                );
                let second = world.stage_pack(
                    "stage-pack-confirmed-next",
                    ("stage-index-upper", "stage-index-lower"),
                    Installed::LeftToAdmission,
                );
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&first)).unwrap();
                    let steps = vec![
                        step("Submit the upper stage", "step-upper"),
                        step("Submit the lower stage", "step-lower"),
                    ];
                    world.plan_bare(steps, two_steps()).unwrap();
                }
                let _pack =
                    crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&second)).unwrap();
                let result = world.plan_bare(Vec::new(), Arc::new(|_, _| text_reply("nobody")));
                let error = result.expect_err("no session bound the second pack's stages");
                assert!(
                    error.contains("stage_incomplete: no_session_bound_the_declared_stages"),
                    "{error}"
                );
            }

            /// The lower stage, on the revision the stale answer to the first call names.
            fn seals_the_lower() -> Responder {
                Arc::new(|index, body| match index {
                    1 => submit(LOWER_TOOL, "fast", "initial"),
                    _ => {
                        let answer = last_answer(body);
                        match answer["revision"].as_str() {
                            Some(revision) if answer["status"] == "stale" => {
                                submit(LOWER_TOOL, "fast", revision)
                            }
                            _ => text_reply("done"),
                        }
                    }
                })
            }

            /// A run with no declaration is never asked. A declared set is asked before anything
            /// bound it, after a session that called nothing, after a seal that is not the whole
            /// set, and passes once every obligation is met.
            #[test]
            fn final_acceptance_asks_for_every_obligation_and_only_a_bound_set_is_asked() {
                const NAME: &str =
                    "final_acceptance_asks_for_every_obligation_and_only_a_bound_set_is_asked";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let mut bare = world.config("http://127.0.0.1:9");
                bare.profile = BARE_PROFILE.to_string();
                assert_eq!(
                    crate::planner::structured_stages::final_gate_failure(&bare),
                    None
                );
                let config = world.config("http://127.0.0.1:9");
                let gate = || crate::planner::structured_stages::final_gate_failure(&config);
                let failure = gate().unwrap();
                assert!(
                    failure.contains("stage_incomplete: no_session_bound_the_declared_stages"),
                    "{failure}"
                );

                let (_, looked) = world.plan(looks(), reads_and_stops());
                assert_stage_incomplete(&looked);
                let failure = gate().unwrap();
                assert!(
                    failure.contains("stage_incomplete: obligations_unmet"),
                    "{failure}"
                );

                let upper = vec![step("Submit the upper stage", "step-upper")];
                let (_, sealed) = world.plan(upper, stops_after_the_upper());
                assert_stage_incomplete(&sealed);
                assert!(gate().is_some(), "a step's seal is not the whole set");

                let lower = vec![step("Submit the lower stage", "step-lower")];
                let (_, result) = world.plan(lower, seals_the_lower());
                result.unwrap();
                assert_eq!(gate(), None);
                assert_eq!(
                    crate::planner::structured_stages::final_gate_failure(&bare),
                    None
                );
            }

            /// The same pack again, with no session: what today's table gives it.
            #[test]
            fn b1_the_same_pack_again_finds_its_own_stages() {
                let Some(world) = one_process("b1_the_same_pack_again_finds_its_own_stages") else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack = world.stage_pack("stage-pack-again", (UPPER, LOWER), Installed::Decoded);
                leave_the_lower_open(&world, &pack);
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                    let again = world.plan_bare(Vec::new(), Arc::new(|_, _| text_reply("nobody")));
                    let error = again.expect_err("its lower stage is still open");
                    assert!(error.contains("lower/main"), "{error}");
                    // The upper is sealed already: only the lower is left.
                    let steps = vec![step("Submit the lower stage", "step-lower")];
                    world.plan_bare(steps, seals_the_lower()).unwrap();
                }
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                let sealed = world.plan_bare(Vec::new(), Arc::new(|_, _| text_reply("nobody")));
                assert!(sealed.is_ok(), "{sealed:?}");
                std::fs::write(world.root.join(LOWER_OUTPUT), "changed after the seal\n").unwrap();
                let moved = world.plan_bare(Vec::new(), Arc::new(|_, _| text_reply("nobody")));
                let error = moved.expect_err("the seal no longer stands for these bytes");
                assert!(error.contains("lower/main"), "{error}");
            }

            /// What one process does when a part of the table's key changes under it, as a REPL or
            /// a server can: a Config built for another extension root replaces the profiles.
            const NO_SESSION: &str = "stage_incomplete: no_session_bound_the_declared_stages";
            const CANNOT_READ: &str = "stage_incomplete: the_selected_pack_cannot_be_read";

            impl World {
                /// `profile` as another extension root declares it, registered as a Config does.
                fn declare_under(&self, root: &str, profile: &str, manifest: &str) {
                    let directory = self.base.join(root).join("profiles").join(profile);
                    std::fs::create_dir_all(&directory).unwrap();
                    std::fs::write(directory.join("manifest.toml"), manifest).unwrap();
                    self.register_again(root);
                }

                fn register_again(&self, root: &str) {
                    crate::planner::extension_profiles::register(&self.base.join(root)).unwrap();
                }

                fn gate_at(&self, workspace: &Path, profile: &str) -> Option<String> {
                    let mut config = self.config("http://127.0.0.1:9");
                    config.workspace_root = workspace.to_path_buf();
                    config.profile = profile.to_string();
                    crate::planner::structured_stages::final_gate_failure(&config)
                }

                fn gate(&self, profile: &str) -> Option<String> {
                    self.gate_at(&self.root, profile)
                }

                fn checks_pack(
                    &self,
                    id: &str,
                    installed: Installed,
                ) -> crate::cli_pack::ResolvedPack {
                    self.pack_declaring(id, CHECKS_ONLY, installed)
                }

                fn record_of(&self, profile: &str) -> PathBuf {
                    crate::planner::structured_stages::ledger::ledger_file(
                        &self.evidence.join("state"),
                        &self.root,
                        profile,
                    )
                    .unwrap()
                }

                fn starts(&self, tool: &str) -> usize {
                    std::fs::read_to_string(self.marks.join(format!("started-{tool}")))
                        .map_or(0, |marks| marks.lines().count())
                }
            }

            const CHECKS_ONLY: &str = "checks:\n  - id: command_check\n    at:\n      kind: final_acceptance\n    params:\n      argv: [test, -f, fixed/seed.json]\n      cwd: workspace\n      expect:\n        exit_code: 0\n        stdout_regex: '^$'\n        max_bytes: 1024\n";

            /// The first manifest declaring its lower tool alone: another contract, the same tools.
            fn lower_alone(profile: &str) -> String {
                let whole = manifest(profile, UPPER, LOWER);
                let head = whole.split("[profile_tools]").next().unwrap();
                format!(
                    "{head}[profile_tools]\nschema_version = \"profile-tool.declaration/v2\"\nbuiltin_tools = [\"Read\"]\ntools = [\n  {{ id = \"{LOWER}\", effect = \"propose-artifacts\" }},\n]\n[profile_tools.stages]\nstages = [\n  {{ id = \"lower\", targets = [{{ id = \"main\", tool_id = \"{LOWER}\" }}] }},\n]\n"
                )
            }

            fn with_no_declaration(profile: &str) -> String {
                let whole = manifest(profile, UPPER, LOWER);
                whole.split("[profile_tools]").next().unwrap().to_string()
            }

            fn nobody() -> Responder {
                Arc::new(|_, _| text_reply("nobody"))
            }

            fn both_stages() -> Vec<PlanStep> {
                vec![
                    step("Submit the upper stage", "step-upper"),
                    step("Submit the lower stage", "step-lower"),
                ]
            }

            fn the_upper() -> Vec<PlanStep> {
                vec![step("Submit the upper stage", "step-upper")]
            }

            fn the_lower() -> Vec<PlanStep> {
                vec![step("Submit the lower stage", "step-lower")]
            }

            /// Declaration: a record that says "no stage" answers for the declaration it was read
            /// under, and a later one with stages and no session is no success.
            #[test]
            fn transition_a_no_stage_record_does_not_pass_a_declaration_with_stages() {
                const NAME: &str =
                    "transition_a_no_stage_record_does_not_pass_a_declaration_with_stages";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let packs = world.packs_with_no_stage("stage-pack-before-the-swap");
                for pack in &packs {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(pack)).unwrap();
                    let result = world.plan_bare(looks(), reads_and_stops());
                    assert!(result.is_ok(), "{}: {result:?}", pack.id);
                    assert_eq!(world.gate(BARE_PROFILE), None, "{}", pack.id);
                }
                world.declare_under(
                    "extension-two",
                    BARE_PROFILE,
                    &manifest(BARE_PROFILE, UPPER, LOWER),
                );
                for pack in &packs {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(pack)).unwrap();
                    let result = world.plan_bare(Vec::new(), nobody());
                    let error = result.expect_err(&pack.id);
                    assert!(error.contains(NO_SESSION), "{}: {error}", pack.id);
                }
                assert!(!world.record_of(BARE_PROFILE).exists(), "nothing recorded");
                // The declaration it was read under still has its answer.
                world.register_again("extension");
                let _pack =
                    crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&packs[0])).unwrap();
                assert_eq!(world.gate(BARE_PROFILE), None);
            }

            /// Declaration, with a session: it binds the new stages, and they can be met.
            #[test]
            fn transition_a_session_binds_a_new_declaration_after_a_no_stage_record() {
                const NAME: &str =
                    "transition_a_session_binds_a_new_declaration_after_a_no_stage_record";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack = world.checks_pack("stage-pack-then-bound", Installed::LeftToAdmission);
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                world.plan_bare(looks(), reads_and_stops()).unwrap();
                world.declare_under(
                    "extension-two",
                    BARE_PROFILE,
                    &manifest(BARE_PROFILE, UPPER, LOWER),
                );
                world.plan_bare(both_stages(), two_steps()).unwrap();
                assert_eq!(world.gate(BARE_PROFILE), None);
                std::fs::write(world.root.join(LOWER_OUTPUT), "changed after the seal\n").unwrap();
                let moved = world.gate(BARE_PROFILE).unwrap();
                assert!(moved.contains("lower/main"), "{moved}");
            }

            /// Declaration: the old run's seals, all there, say nothing of a later contract.
            #[test]
            fn transition_old_seals_do_not_pass_another_declaration_with_no_session() {
                const NAME: &str =
                    "transition_old_seals_do_not_pass_another_declaration_with_no_session";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                let (_, sealed) = world.plan(both_stages(), two_steps());
                sealed.unwrap();
                assert_eq!(world.gate(PROFILE), None);
                let file = world.record_of(PROFILE);
                let record = std::fs::read(&file).unwrap();
                world.declare_under("extension-two", PROFILE, &lower_alone(PROFILE));
                let (_, result) = world.plan(Vec::new(), nobody());
                let error = result.expect_err("nothing bound this declaration");
                assert!(error.contains(NO_SESSION), "{error}");
                assert_eq!(std::fs::read(&file).unwrap(), record, "the record stays");
                // The first declaration finds its seals, for its own bytes.
                world.register_again("extension");
                assert_eq!(world.gate(PROFILE), None);
                assert_eq!(std::fs::read(&file).unwrap(), record, "the record stays");
            }

            /// The upper left open under the first declaration, then a session of another one that
            /// calls a stage: the one record per workspace and profile stops it.
            fn another_declarations_session_is_stopped(world: &World) {
                let (_, first) = world.plan(the_upper(), stops_after_the_upper());
                assert_stage_incomplete(&first);
                assert_eq!(world.starts(UPPER), 1);
                let file = world.record_of(PROFILE);
                let record = std::fs::read(&file).unwrap();
                world.declare_under("extension-two", PROFILE, &lower_alone(PROFILE));
                let called = Arc::new(AtomicBool::new(false));
                let saw = called.clone();
                let (_, result) = world.plan(
                    the_lower(),
                    Arc::new(move |index, _| {
                        if index == 1 {
                            saw.store(true, Ordering::SeqCst);
                            submit(LOWER_TOOL, "fast", "initial")
                        } else {
                            text_reply("done")
                        }
                    }),
                );
                assert!(called.load(Ordering::SeqCst), "the session did start");
                let error = result.expect_err("the record is of another declaration");
                assert!(error.contains("stage_ledger_refused"), "{error}");
                assert!(error.contains("contract_changed"), "{error}");
                assert_eq!(world.starts(LOWER), 0, "no process for a refused call");
                assert_eq!(world.starts(UPPER), 1);
                assert!(!world.root.join(LOWER_OUTPUT).exists(), "nothing committed");
                assert_eq!(std::fs::read(&file).unwrap(), record, "nothing recorded");
            }

            /// Declaration, with a session. And the gate of that declaration, after it.
            #[test]
            fn transition_another_declarations_session_stops_at_contract_changed() {
                const NAME: &str =
                    "transition_another_declarations_session_stops_at_contract_changed";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                another_declarations_session_is_stopped(&world);
                let refused = world.gate(PROFILE).unwrap();
                assert!(refused.contains("ledger_refused"), "{refused}");
            }

            /// Declaration, going back: what the refused session left is under its own declaration,
            /// so the first one's gate still names the stage it left open.
            #[test]
            fn transition_going_back_after_another_declarations_session_finds_the_first_record() {
                const NAME: &str = "transition_going_back_after_another_declarations_session_finds_the_first_record";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                another_declarations_session_is_stopped(&world);
                world.register_again("extension");
                let open = world.gate(PROFILE).unwrap();
                assert!(open.contains("1 unmet: lower/main"), "{open}");
            }

            /// Declaration: stages taken out of it, or the profile gone from the root. What is left
            /// is the selected pack, and the old run's open stage is not this run's.
            #[test]
            fn transition_a_declaration_with_no_stage_answers_for_itself() {
                const NAME: &str = "transition_a_declaration_with_no_stage_answers_for_itself";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                let pack = world.pack_for(
                    PROFILE,
                    "stage-pack-under-stages",
                    CHECKS_ONLY,
                    Installed::LeftToAdmission,
                );
                let upper_open = || {
                    let (_, first) = world.plan(the_upper(), stops_after_the_upper());
                    assert_stage_incomplete(&first);
                };
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                    upper_open();
                }
                world.declare_under("extension-two", PROFILE, &with_no_declaration(PROFILE));
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                    assert_eq!(world.gate(PROFILE), None, "with the pack selected");
                }
                assert_eq!(world.gate(PROFILE), None, "with no pack");
                let (_, looked) = world.plan(looks(), reads_and_stops());
                assert!(looked.is_ok(), "{looked:?}");
                // A root that does not declare it: `find` says nothing.
                world.declare_under(
                    "extension-three",
                    INDEX_PROFILE,
                    &manifest(INDEX_PROFILE, "stage-index-upper", "stage-index-lower"),
                );
                assert!(crate::planner::extension_profiles::find(PROFILE).is_none());
                assert_eq!(world.gate(PROFILE), None, "with no declaration");
                {
                    // A pack for a profile no root declares does not parse.
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                    assert_eq!(world.gate(PROFILE).as_deref(), Some(CANNOT_READ));
                }
                // The first declaration, and its open stage, as they were.
                world.register_again("extension");
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                let open = world.gate(PROFILE).unwrap();
                assert!(open.contains("1 unmet: lower/main"), "{open}");
            }

            /// Root: the same bytes under another root are the same declaration, for a set of stages.
            #[test]
            fn transition_the_same_manifest_under_another_root_keeps_its_stages() {
                const NAME: &str =
                    "transition_the_same_manifest_under_another_root_keeps_its_stages";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                let (_, first) = world.plan(the_upper(), stops_after_the_upper());
                assert_stage_incomplete(&first);
                world.declare_under("extension-two", PROFILE, &manifest(PROFILE, UPPER, LOWER));
                let open = world.gate(PROFILE).unwrap();
                assert!(open.contains("1 unmet: lower/main"), "{open}");
                let (_, sealed) = world.plan(the_lower(), seals_the_lower());
                sealed.unwrap();
                assert_eq!(world.gate(PROFILE), None);
            }

            /// Root: another byte is another declaration, whatever the byte says. With no session
            /// that is no success. A session binds it, and the seals are all still there.
            #[test]
            fn transition_a_manifest_changed_only_in_wording_is_bound_by_a_session() {
                const NAME: &str =
                    "transition_a_manifest_changed_only_in_wording_is_bound_by_a_session";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                let (_, sealed) = world.plan(both_stages(), two_steps());
                sealed.unwrap();
                let reworded = manifest(PROFILE, UPPER, LOWER)
                    .replace("Stage session", "Stage session, reworded");
                world.declare_under("extension-two", PROFILE, &reworded);
                assert_eq!(world.gate(PROFILE).as_deref(), Some(NO_SESSION));
                let (_, looked) = world.plan(looks(), reads_and_stops());
                assert!(looked.is_ok(), "{looked:?}");
                assert_eq!(world.gate(PROFILE), None);
            }

            /// Root: and for what a session found in a pack, which is not read again.
            #[test]
            fn transition_the_same_manifest_under_another_root_keeps_a_no_stage_record() {
                const NAME: &str =
                    "transition_the_same_manifest_under_another_root_keeps_a_no_stage_record";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack = world.checks_pack("stage-pack-two-roots", Installed::LeftToAdmission);
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                world.plan_bare(looks(), reads_and_stops()).unwrap();
                world.declare_under(
                    "extension-two",
                    BARE_PROFILE,
                    &with_no_declaration(BARE_PROFILE),
                );
                std::fs::remove_file(pack.directory.join("eval.yaml")).unwrap();
                assert_eq!(world.gate(BARE_PROFILE), None);
            }

            /// Workspace: canonical, so another name for it is the same one and another is not.
            #[test]
            fn transition_another_workspace_has_no_record_and_a_link_to_this_one_does() {
                const NAME: &str =
                    "transition_another_workspace_has_no_record_and_a_link_to_this_one_does";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let other = world.base.join("workspace-two");
                std::fs::create_dir_all(&other).unwrap();
                let link = world.base.join("workspace-link");
                std::os::unix::fs::symlink(&world.root, &link).unwrap();
                let (_, sealed) = world.plan(both_stages(), two_steps());
                sealed.unwrap();
                assert_eq!(world.gate_at(&link, PROFILE), None);
                assert_eq!(
                    world.gate_at(&other, PROFILE).as_deref(),
                    Some(NO_SESSION),
                    "the seals are of the other workspace"
                );
                let pack = world.checks_pack("stage-pack-two-places", Installed::LeftToAdmission);
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                world.plan_bare(looks(), reads_and_stops()).unwrap();
                assert_eq!(
                    world.gate_at(&other, BARE_PROFILE),
                    None,
                    "it reads the pack"
                );
                std::fs::remove_file(pack.directory.join("eval.yaml")).unwrap();
                assert_eq!(world.gate_at(&world.root, BARE_PROFILE), None);
                assert_eq!(world.gate_at(&link, BARE_PROFILE), None);
                assert_eq!(
                    world.gate_at(&other, BARE_PROFILE).as_deref(),
                    Some(CANNOT_READ)
                );
                // A path that cannot be resolved finds no record, so the gate reads the pack.
                let gone = world.base.join("workspace-gone");
                assert_eq!(
                    world.gate_at(&gone, BARE_PROFILE).as_deref(),
                    Some(CANNOT_READ)
                );
            }

            /// Profile name: neither kind of record answers for another name.
            #[test]
            fn transition_another_profile_name_has_no_record() {
                const NAME: &str = "transition_another_profile_name_has_no_record";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let (_, sealed) = world.plan(both_stages(), two_steps());
                sealed.unwrap();
                assert_eq!(world.gate(PROFILE), None);
                assert_eq!(world.gate(INDEX_PROFILE).as_deref(), Some(NO_SESSION));
                assert_eq!(world.gate(PLAIN_PROFILE), None);
                assert_eq!(world.gate(BARE_PROFILE), None);
                let pack = world.checks_pack("stage-pack-two-names", Installed::LeftToAdmission);
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                world.plan_bare(looks(), reads_and_stops()).unwrap();
                assert_eq!(world.gate(INDEX_PROFILE).as_deref(), Some(NO_SESSION));
                std::fs::remove_file(pack.directory.join("eval.yaml")).unwrap();
                assert_eq!(world.gate(BARE_PROFILE), None);
                assert_eq!(world.gate(PLAIN_PROFILE).as_deref(), Some(CANNOT_READ));
            }

            /// Pack: its directory, id, and version the same, its bytes another. Neither kind of
            /// record answers for those.
            #[test]
            fn transition_a_pack_rewritten_in_place_has_no_record() {
                const NAME: &str = "transition_a_pack_rewritten_in_place_has_no_record";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let tools = (UPPER, LOWER);
                let checks = world.checks_pack("stage-pack-rewritten", Installed::LeftToAdmission);
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&checks)).unwrap();
                    world.plan_bare(looks(), reads_and_stops()).unwrap();
                }
                let stages =
                    world.stage_pack("stage-pack-rewritten", tools, Installed::LeftToAdmission);
                assert_eq!(stages.directory, checks.directory);
                assert_ne!(stages.hash, checks.hash);
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&stages)).unwrap();
                    assert_eq!(world.gate(BARE_PROFILE).as_deref(), Some(NO_SESSION));
                }
                let first = world.stage_pack("stage-pack-stages-first", tools, Installed::Decoded);
                leave_the_lower_open(&world, &first);
                let second = world.pack_declaring(
                    "stage-pack-stages-first",
                    &format!(
                        "profile_tools:\n  schema_version: profile-tool.declaration/v2\n  builtin_tools: [Read]\n  tools:\n    - id: {LOWER}\n      effect: propose-artifacts\n  stages:\n    stages:\n      - id: lower\n        targets:\n          - id: main\n            tool_id: {LOWER}\n"
                    ),
                    Installed::LeftToAdmission,
                );
                assert_eq!(second.directory, first.directory);
                let _pack =
                    crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&second)).unwrap();
                assert_eq!(world.gate(BARE_PROFILE).as_deref(), Some(NO_SESSION));
            }

            /// Decode: a decoded pack with no stage leaves no record. Nothing vouches for the same
            /// pack later with no decoded value, so the gate reads it, and it cannot.
            #[test]
            fn transition_a_decoded_pack_leaves_no_record_for_itself_undecoded() {
                const NAME: &str =
                    "transition_a_decoded_pack_leaves_no_record_for_itself_undecoded";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let decoded = world.checks_pack("stage-pack-decoded-first", Installed::Decoded);
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&decoded)).unwrap();
                    world.plan_bare(looks(), reads_and_stops()).unwrap();
                    assert_eq!(world.gate(BARE_PROFILE), None);
                }
                let left =
                    world.checks_pack("stage-pack-decoded-first", Installed::LeftToAdmission);
                assert_eq!(left.hash, decoded.hash);
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&left)).unwrap();
                std::fs::remove_file(left.directory.join("eval.yaml")).unwrap();
                assert_eq!(world.gate(BARE_PROFILE).as_deref(), Some(CANNOT_READ));
            }

            /// Decode: whether a value was decoded is no part of which pack it is.
            #[test]
            fn transition_a_decoded_value_coming_or_going_keeps_the_record() {
                const NAME: &str = "transition_a_decoded_value_coming_or_going_keeps_the_record";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let tools = (UPPER, LOWER);
                let decoded = world.stage_pack("stage-pack-both-ways", tools, Installed::Decoded);
                leave_the_lower_open(&world, &decoded);
                let left =
                    world.stage_pack("stage-pack-both-ways", tools, Installed::LeftToAdmission);
                assert_eq!(left.hash, decoded.hash);
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&left)).unwrap();
                    let open = world.gate(BARE_PROFILE).unwrap();
                    assert!(open.contains("1 unmet: lower/main"), "{open}");
                    world.plan_bare(the_lower(), seals_the_lower()).unwrap();
                }
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&decoded)).unwrap();
                    assert_eq!(world.gate(BARE_PROFILE), None);
                }
                let left = world.checks_pack("stage-pack-then-known", Installed::LeftToAdmission);
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&left)).unwrap();
                    world.plan_bare(looks(), reads_and_stops()).unwrap();
                }
                let known = world.checks_pack("stage-pack-then-known", Installed::Decoded);
                let _pack =
                    crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&known)).unwrap();
                std::fs::remove_file(known.directory.join("eval.yaml")).unwrap();
                assert_eq!(world.gate(BARE_PROFILE), None, "its value says no stage");
            }

            fn both_stages_declared() -> String {
                format!(
                    "profile_tools:\n  schema_version: profile-tool.declaration/v2\n  builtin_tools: [Read]\n  tools:\n    - id: {UPPER}\n      effect: propose-artifacts\n    - id: {LOWER}\n      effect: propose-artifacts\n  stages:\n    stages:\n      - id: upper\n        targets:\n          - id: main\n            tool_id: {UPPER}\n      - id: lower\n        targets:\n          - id: main\n            tool_id: {LOWER}\n"
                )
            }

            /// Profile name, with nothing else to tell them apart: two builtin profiles have no
            /// declaration's hash, and the pack is the same. One's record, open or sealed, says
            /// nothing of the other. The value decoded for the pack answers without a read.
            #[test]
            fn transition_another_builtin_profile_with_the_same_pack_has_no_record() {
                const NAME: &str =
                    "transition_another_builtin_profile_with_the_same_pack_has_no_record";
                const BOUND: &str = "python-cli";
                const OTHER: &str = "data";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                assert!(crate::planner::extension_profiles::find(BOUND).is_none());
                assert!(crate::planner::extension_profiles::find(OTHER).is_none());
                let pack = world.pack_for(
                    BOUND,
                    "stage-pack-of-two-builtins",
                    &both_stages_declared(),
                    Installed::Decoded,
                );
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                // Each builtin's own final contract has its say. The gate is what is asked.
                let upper = world.plan_as(BOUND, the_upper(), stops_after_the_upper());
                assert!(upper.is_err(), "{upper:?}");
                let open = world.gate(BOUND).unwrap();
                assert!(open.contains("1 unmet: lower/main"), "{open}");
                assert_eq!(world.gate(OTHER).as_deref(), Some(NO_SESSION));
                let lower = world.plan_as(BOUND, the_lower(), seals_the_lower());
                assert!(lower.is_err(), "{lower:?}");
                assert_eq!(
                    world.gate(BOUND),
                    None,
                    "every stage of the record is sealed"
                );
                assert_eq!(world.gate(OTHER).as_deref(), Some(NO_SESSION));
            }

            /// Whether a record's seals are for the contract declared now, asked at the gate.
            const DECLARED_UNREADABLE: &str =
                "stage_incomplete: the_declared_stages_cannot_be_read";

            impl World {
                fn descriptor_file(&self, tool: &str) -> PathBuf {
                    self.base
                        .join(format!("registry/profile-tools/{tool}.json"))
                }

                fn edit_descriptor(&self, tool: &str, edit: impl FnOnce(&mut Value)) {
                    let file = self.descriptor_file(tool);
                    let mut descriptor: Value =
                        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
                    edit(&mut descriptor);
                    std::fs::write(&file, descriptor.to_string()).unwrap();
                }

                fn seal_both(&self) {
                    let (_, sealed) = self.plan(both_stages(), two_steps());
                    sealed.unwrap();
                    assert_eq!(self.gate(PROFILE), None);
                }

                /// This process's registry, with its descriptors as they are now, under `root`.
                fn registry_at(&self, root: &str) -> PathBuf {
                    let root = self.base.join(root);
                    let tools = root.join("profile-tools");
                    std::fs::create_dir_all(&tools).unwrap();
                    for tool in [UPPER, LOWER] {
                        std::fs::copy(
                            self.descriptor_file(tool),
                            tools.join(format!("{tool}.json")),
                        )
                        .unwrap();
                    }
                    root
                }
            }

            fn reading(inputs: &Value) -> impl FnOnce(&mut Value) + '_ {
                move |descriptor| {
                    descriptor["schema"]["inputs"] = inputs.clone();
                    descriptor["schema_sha256"] = json!(sha256(
                        serde_json::to_string(&descriptor["schema"])
                            .unwrap()
                            .as_bytes()
                    ));
                }
            }

            fn use_registry(root: &Path) {
                // SAFETY: this process runs this one test, on this one thread.
                unsafe {
                    std::env::set_var(REGISTRY_ENV, root);
                }
            }

            /// Another registry root, whose lower tool no longer reads the upper's output.
            #[test]
            fn identity_a_registry_root_dropping_an_upstream_is_no_success_with_no_session() {
                const NAME: &str =
                    "identity_a_registry_root_dropping_an_upstream_is_no_success_with_no_session";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world.seal_both();
                let record = std::fs::read(world.record_of(PROFILE)).unwrap();
                let original = std::fs::read(world.descriptor_file(LOWER)).unwrap();
                world.edit_descriptor(LOWER, reading(&json!([FIXED])));
                let root = world.registry_at("registry-two");
                std::fs::write(world.descriptor_file(LOWER), original).unwrap();
                use_registry(&root);
                let (_, result) = world.plan(Vec::new(), nobody());
                let error = result.expect_err("nothing bound this contract");
                assert!(error.contains(NO_SESSION), "{error}");
                assert_eq!(std::fs::read(world.record_of(PROFILE)).unwrap(), record);
            }

            /// The same root, the operator's file edited in place, as in a long-lived process.
            #[test]
            fn identity_a_descriptor_rewritten_in_place_is_no_success_with_no_session() {
                const NAME: &str =
                    "identity_a_descriptor_rewritten_in_place_is_no_success_with_no_session";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world.seal_both();
                let record = std::fs::read(world.record_of(PROFILE)).unwrap();
                world.edit_descriptor(LOWER, reading(&json!([FIXED])));
                let (_, result) = world.plan(Vec::new(), nobody());
                let error = result.expect_err("nothing bound this contract");
                assert!(error.contains(NO_SESSION), "{error}");
                assert_eq!(std::fs::read(world.record_of(PROFILE)).unwrap(), record);
            }

            /// The other way: a tool that read no stage's output now reads one.
            #[test]
            fn identity_an_upstream_added_to_a_descriptor_is_no_success_with_no_session() {
                const NAME: &str =
                    "identity_an_upstream_added_to_a_descriptor_is_no_success_with_no_session";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                world.edit_descriptor(LOWER, reading(&json!([FIXED])));
                let _live = world.enter();
                world.seal_both();
                world.edit_descriptor(LOWER, reading(&json!([UPPER_OUTPUT, FIXED])));
                let (_, result) = world.plan(Vec::new(), nobody());
                let error = result.expect_err("nothing bound this contract");
                assert!(error.contains(NO_SESSION), "{error}");
            }

            /// The same upstream set: the entry refuses it as before, and the gate does on its own,
            /// for a change that comes after the entry's check.
            #[test]
            fn identity_changed_required_checks_are_refused_at_the_entry_and_by_the_gate() {
                const NAME: &str =
                    "identity_changed_required_checks_are_refused_at_the_entry_and_by_the_gate";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world.seal_both();
                world.edit_descriptor(LOWER, |descriptor| {
                    descriptor["stage"]["required_checks"] = json!(["shape", "another-check"]);
                });
                assert_eq!(world.gate(PROFILE).as_deref(), Some(NO_SESSION));
                let (_, result) = world.plan(Vec::new(), nobody());
                let error = result.expect_err("the entry compares the descriptor");
                assert!(error.contains("schema_mismatch"), "{error}");
            }

            #[test]
            fn identity_a_stage_descriptor_gone_is_no_success() {
                const NAME: &str = "identity_a_stage_descriptor_gone_is_no_success";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world.seal_both();
                std::fs::remove_file(world.descriptor_file(LOWER)).unwrap();
                assert_eq!(world.gate(PROFILE).as_deref(), Some(DECLARED_UNREADABLE));
            }

            /// A pack left to admission is read for what it declares now, and it cannot be.
            #[test]
            fn identity_a_stage_packs_bytes_gone_is_no_success() {
                const NAME: &str = "identity_a_stage_packs_bytes_gone_is_no_success";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let world = world.with_bare_profile();
                let _live = world.enter();
                let pack = world.stage_pack(
                    "stage-pack-then-gone",
                    (UPPER, LOWER),
                    Installed::LeftToAdmission,
                );
                let _pack = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                world.plan_bare(both_stages(), two_steps()).unwrap();
                assert_eq!(world.gate(BARE_PROFILE), None);
                std::fs::remove_file(pack.directory.join("eval.yaml")).unwrap();
                assert_eq!(world.gate(BARE_PROFILE).as_deref(), Some(CANNOT_READ));
            }

            /// After a session too, and for any byte of the descriptor. The same bytes back are the
            /// same contract.
            #[test]
            fn identity_a_descriptor_changed_after_the_session_is_no_success_until_restored() {
                const NAME: &str =
                    "identity_a_descriptor_changed_after_the_session_is_no_success_until_restored";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world.seal_both();
                let file = world.descriptor_file(LOWER);
                let original = std::fs::read(&file).unwrap();
                world.edit_descriptor(LOWER, |descriptor| {
                    descriptor["description"] = json!("Reworded, and nothing else.");
                });
                assert_eq!(world.gate(PROFILE).as_deref(), Some(NO_SESSION));
                std::fs::write(&file, original).unwrap();
                assert_eq!(world.gate(PROFILE), None);
            }

            /// The control: the same contract passes, also from another root with the same bytes.
            #[test]
            fn identity_the_same_contract_passes_after_its_session() {
                const NAME: &str = "identity_the_same_contract_passes_after_its_session";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world.seal_both();
                let (_, again) = world.plan(Vec::new(), nobody());
                assert!(again.is_ok(), "{again:?}");
                use_registry(&world.registry_at("registry-same-bytes"));
                assert_eq!(world.gate(PROFILE), None);
                let (_, moved) = world.plan(Vec::new(), nobody());
                assert!(moved.is_ok(), "{moved:?}");
            }

            /// The control: a session of the changed contract gets as far as the record.
            #[test]
            fn identity_a_session_of_the_changed_contract_stops_at_the_record() {
                const NAME: &str = "identity_a_session_of_the_changed_contract_stops_at_the_record";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world.seal_both();
                world.edit_descriptor(LOWER, reading(&json!([FIXED])));
                let (_, prompted) = world.prompt(nobody());
                let error = prompted.expect_err("the record is of another contract");
                assert!(error.contains("ledger_refused"), "{error}");
                let gate = world.gate(PROFILE).unwrap();
                assert!(gate.contains("ledger_refused"), "{gate}");
            }

            /// The gate's own answer on a contract changed under a record.
            #[test]
            fn identity_a_changed_contract_is_no_success_at_the_gate() {
                const NAME: &str = "identity_a_changed_contract_is_no_success_at_the_gate";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world.seal_both();
                world.edit_descriptor(LOWER, reading(&json!([FIXED])));
                assert_eq!(world.gate(PROFILE).as_deref(), Some(NO_SESSION));
            }

            /// The control: whatever the gate answers, it admits nothing, starts nothing, and
            /// writes nothing. This asks nothing of the answer itself: on a contract that matches,
            /// on one that does not, and on a pin the process no longer holds.
            #[test]
            fn identity_the_gate_adopts_and_starts_nothing() {
                const NAME: &str = "identity_the_gate_adopts_and_starts_nothing";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world.seal_both();
                let adoption_count =
                    super::super::super::super::schema::binding::adoption_count_for_test;
                let adoptions = adoption_count();
                let starts = (world.starts(UPPER), world.starts(LOWER));
                let record = std::fs::read(world.record_of(PROFILE)).unwrap();
                let unchanged = |scene: &str| {
                    assert_eq!(adoption_count(), adoptions, "{scene}");
                    assert_eq!(
                        (world.starts(UPPER), world.starts(LOWER)),
                        starts,
                        "{scene}"
                    );
                    assert_eq!(
                        std::fs::read(world.record_of(PROFILE)).unwrap(),
                        record,
                        "{scene}"
                    );
                };
                let _ = world.gate(PROFILE);
                unchanged("the same contract");
                let file = world.descriptor_file(LOWER);
                let original = std::fs::read(&file).unwrap();
                world.edit_descriptor(LOWER, reading(&json!([FIXED])));
                let _ = world.gate(PROFILE);
                unchanged("another contract");
                std::fs::write(&file, original).unwrap();
                std::fs::write(world.tool_script(LOWER), "#!/bin/sh\nexit 3\n").unwrap();
                let _ = world.gate(PROFILE);
                unchanged("a process directory off its pin");
            }

            impl World {
                fn tool_script(&self, tool: &str) -> PathBuf {
                    self.base.join(format!("tools/{tool}/tool.sh"))
                }

                fn descriptor_sha(&self, tool: &str) -> String {
                    super::super::super::super::descriptor::load(tool, &self.root)
                        .unwrap()
                        .sha256
                }

                /// The lower tool pins a file of its own, in the descriptor's `dependencies`.
                fn with_a_pinned_library(&self) -> PathBuf {
                    let library = self.base.join("lower-tool-library.txt");
                    std::fs::write(&library, "the library as first pinned\n").unwrap();
                    self.pin_library(&library);
                    library
                }

                fn pin_library(&self, library: &Path) {
                    let pin = sha256(&std::fs::read(library).unwrap());
                    self.edit_descriptor(LOWER, |descriptor| {
                        descriptor["dependencies"] = json!([{"path": library, "sha256": pin}]);
                    });
                }

                /// The lower tool runs an executable of its own, whose bytes a test can change. A
                /// script: a system may kill a copy of its own shell.
                fn with_an_executable_of_its_own(&self) -> PathBuf {
                    use std::os::unix::fs::PermissionsExt as _;
                    let directory = self.base.join("lower-tool-bin");
                    std::fs::create_dir_all(&directory).unwrap();
                    let executable = directory.join("run-the-script");
                    std::fs::write(&executable, "#!/bin/sh\nexec /bin/sh \"$@\"\n").unwrap();
                    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
                        .unwrap();
                    let pin = sha256(&std::fs::read(&executable).unwrap());
                    self.edit_descriptor(LOWER, |descriptor| {
                        descriptor["process"]["executable"] =
                            json!({"path": executable, "sha256": pin});
                    });
                    executable
                }
            }

            /// A pinned file's bytes alone. The descriptor's bytes do not change, and its digest
            /// cannot be compared here at all: `descriptor::load` refuses first. The same bytes back
            /// pass again.
            #[test]
            fn identity_a_dependency_changed_under_its_pin_is_unreadable_until_restored() {
                const NAME: &str =
                    "identity_a_dependency_changed_under_its_pin_is_unreadable_until_restored";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let library = world.with_a_pinned_library();
                let _live = world.enter();
                world.seal_both();
                let original = std::fs::read(&library).unwrap();
                std::fs::write(&library, "the library, changed\n").unwrap();
                assert_eq!(world.gate(PROFILE).as_deref(), Some(DECLARED_UNREADABLE));
                std::fs::write(&library, original).unwrap();
                assert_eq!(world.gate(PROFILE), None);
            }

            /// A pinned file and its pin together: the descriptor's digest is what changed.
            #[test]
            fn identity_a_dependency_repinned_is_no_success_until_restored() {
                const NAME: &str = "identity_a_dependency_repinned_is_no_success_until_restored";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let library = world.with_a_pinned_library();
                let _live = world.enter();
                world.seal_both();
                let before = world.descriptor_sha(LOWER);
                let original = std::fs::read(&library).unwrap();
                std::fs::write(&library, "the library, changed and pinned\n").unwrap();
                world.pin_library(&library);
                assert_ne!(world.descriptor_sha(LOWER), before);
                assert_eq!(world.gate(PROFILE).as_deref(), Some(NO_SESSION));
                std::fs::write(&library, original).unwrap();
                world.pin_library(&library);
                assert_eq!(world.descriptor_sha(LOWER), before);
                assert_eq!(world.gate(PROFILE), None);
            }

            /// The script the tool runs, in a directory the process pins. The descriptor's digest
            /// stays the same, so only the pin says the bytes moved.
            #[test]
            fn identity_a_process_directory_changed_under_its_pin_is_unreadable_until_restored() {
                const NAME: &str = "identity_a_process_directory_changed_under_its_pin_is_unreadable_until_restored";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let _live = world.enter();
                world.seal_both();
                let before = world.descriptor_sha(LOWER);
                let script = world.tool_script(LOWER);
                let original = std::fs::read(&script).unwrap();
                std::fs::write(&script, "#!/bin/sh\nexit 3\n").unwrap();
                assert_eq!(world.descriptor_sha(LOWER), before);
                assert_eq!(world.gate(PROFILE).as_deref(), Some(DECLARED_UNREADABLE));
                std::fs::write(&script, original).unwrap();
                assert_eq!(world.gate(PROFILE), None);
            }

            /// The executable's bytes alone, the same way.
            #[test]
            fn identity_an_executable_changed_under_its_pin_is_unreadable_until_restored() {
                const NAME: &str =
                    "identity_an_executable_changed_under_its_pin_is_unreadable_until_restored";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                let executable = world.with_an_executable_of_its_own();
                let _live = world.enter();
                world.seal_both();
                let before = world.descriptor_sha(LOWER);
                let original = std::fs::read(&executable).unwrap();
                let mut changed = original.clone();
                changed.extend_from_slice(b"one more byte");
                std::fs::write(&executable, changed).unwrap();
                assert_eq!(world.descriptor_sha(LOWER), before);
                assert_eq!(world.gate(PROFILE).as_deref(), Some(DECLARED_UNREADABLE));
                std::fs::write(&executable, original).unwrap();
                assert_eq!(world.gate(PROFILE), None);
            }

            /// `find` says nothing for a builtin profile, whichever root is registered, and no root
            /// can declare that name. A pack's record for it stays found.
            #[test]
            fn transition_a_builtin_profile_finds_its_pack_record_under_any_root() {
                const NAME: &str =
                    "transition_a_builtin_profile_finds_its_pack_record_under_any_root";
                let Some(world) = one_process(NAME) else {
                    return;
                };
                const BUILTIN: &str = "python-cli";
                let _live = world.enter();
                assert!(crate::planner::extension_profiles::find(BUILTIN).is_none());
                let pack = world.pack_for(
                    BUILTIN,
                    "stage-pack-for-a-builtin",
                    &both_stages_declared(),
                    Installed::Decoded,
                );
                {
                    let _pack =
                        crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                    assert_eq!(world.gate(BUILTIN).as_deref(), Some(NO_SESSION));
                    // This profile's own final contract has its say first. The gate is what is asked.
                    let first = world.plan_as(BUILTIN, the_upper(), stops_after_the_upper());
                    assert!(first.is_err(), "{first:?}");
                    let open = world.gate(BUILTIN).unwrap();
                    assert!(open.contains("1 unmet: lower/main"), "{open}");
                    world.declare_under("extension-two", PROFILE, &lower_alone(PROFILE));
                    let open = world.gate(BUILTIN).unwrap();
                    assert!(open.contains("1 unmet: lower/main"), "{open}");
                }
                assert_eq!(world.gate(BUILTIN), None, "with no pack it has no stage");
                let checks = world.pack_for(
                    BUILTIN,
                    "stage-pack-builtin-checks",
                    CHECKS_ONLY,
                    Installed::LeftToAdmission,
                );
                let _pack =
                    crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&checks)).unwrap();
                let looked = world.plan_as(BUILTIN, looks(), reads_and_stops());
                assert!(looked.is_err(), "{looked:?}");
                std::fs::remove_file(checks.directory.join("eval.yaml")).unwrap();
                assert_eq!(world.gate(BUILTIN), None, "its session read it");
            }
        }
    }

    #[test]
    fn a_failing_exit_with_a_valid_result_is_refused() {
        let bench = bench(&format!("{}exit 7\n", good_script()));
        let error = bench.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::ProcessFailed, "{error:#}");
        assert!(error.to_string().contains("exit_nonzero"), "{error:#}");
        assert_eq!(bench.saved(), None);
    }

    #[test]
    fn the_result_limit_holds_at_its_edge_with_stderr_counted() {
        // The text has a fixed length once its digests are filled in, so one run measures it.
        let script = format!(
            "{} | tee \"$MARKS/stdout\"\nprintf 1234 >&2\n",
            good_script().trim_end()
        );
        let measured = bench(&script);
        measured.call().unwrap();
        let stdout = std::fs::read(measured.marks.join("stdout")).unwrap().len();
        for (limit, ok) in [(stdout + 4, true), (stdout + 3, false)] {
            let bench = bench_with(
                &script,
                Options {
                    max_result_bytes: limit,
                    ..Options::default()
                },
            );
            assert_eq!(bench.call().is_ok(), ok, "limit {limit}");
            assert_eq!(bench.saved().is_some(), ok, "limit {limit}");
        }
    }

    #[test]
    fn the_wall_clock_limit_stops_the_process_and_saves_nothing() {
        let bench = bench_with(
            &format!("{PRELUDE}exec sleep 30\n"),
            Options {
                timeout_ms: 200,
                ..Options::default()
            },
        );
        let started = std::time::Instant::now();
        let error = bench.call().unwrap_err();
        assert!(error.to_string().contains("timed_out"), "{error:#}");
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        assert_eq!(bench.saved(), None);
    }

    #[test]
    fn a_plan_context_is_refused_before_the_process() {
        let bench = bench(&good_script());
        let mut context = bench.context();
        context.mode = ExecutionMode::Plan;
        let error = bench
            .tool
            .execute(&json!({"mode": "fast"}), &context, &|| false)
            .unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectReadOnly);
        assert_eq!(bench.starts(), 0);
        assert_eq!(bench.saved(), None);
    }

    #[test]
    fn an_explicit_read_only_policy_stops_it_whatever_yes_says() {
        let bench = bench(&good_script());
        let _policy = allow_policy::install(false, &[AllowTarget::Read]);
        let error = bench.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectReadOnly);
        assert_eq!(bench.starts(), 0);
        assert_eq!(bench.saved(), None);
    }

    #[test]
    fn an_explicit_write_policy_is_the_authority_it_takes() {
        let bench = bench(&good_script());
        let _policy = allow_policy::install(false, &[AllowTarget::Write]);
        let mut context = bench.context();
        context.auto_approve = false;
        bench
            .tool
            .execute(&json!({"mode": "fast"}), &context, &|| false)
            .unwrap();
        assert_eq!(bench.saved().as_deref(), Some(CANDIDATE));
    }

    #[test]
    fn no_approval_in_a_legacy_run_is_refused_before_the_process() {
        let bench = bench(&good_script());
        let mut context = bench.context();
        context.auto_approve = false;
        let error = bench
            .tool
            .execute(&json!({"mode": "fast"}), &context, &|| false)
            .unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectReadOnly);
        assert_eq!(bench.starts(), 0);
    }

    /// The policy turns read-only while the process runs: only the check right before the
    /// commit can see this.
    #[test]
    fn a_policy_that_turned_read_only_during_the_process_stops_the_commit() {
        let bench = bench(&good_script());
        let held = RefCell::new(None);
        let error = bench
            .tool
            .execute(&json!({"mode": "fast"}), &bench.context(), &|| {
                held.borrow_mut()
                    .get_or_insert_with(|| allow_policy::install(false, &[AllowTarget::Read]));
                false
            })
            .unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectReadOnly, "{error:#}");
        assert_eq!(bench.starts(), 1);
        assert_eq!(bench.saved(), None);
        assert!(
            std::fs::read_dir(bench.workspace.join("out"))
                .map_or(true, |mut dir| dir.next().is_none())
        );
    }

    #[test]
    fn a_protected_destination_or_input_is_refused_with_the_values_in_force() {
        for protected in [OUTPUT, "out", INPUT] {
            let bench = bench(&good_script());
            let mut context = bench.context();
            context.protected_paths = vec![protected.to_string()];
            let error = bench
                .tool
                .execute(&json!({"mode": "fast"}), &context, &|| false)
                .unwrap_err();
            assert_eq!(kind(&error), RefusalKind::EffectPathRefused, "{protected}");
            assert_eq!(bench.starts(), 0, "{protected}");
        }
    }

    #[test]
    fn another_workspace_in_the_context_is_refused() {
        let bench = bench(&good_script());
        let mut context = bench.context();
        context.root = bench.marks.clone();
        let error = bench
            .tool
            .execute(&json!({"mode": "fast"}), &context, &|| false)
            .unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectPathRefused);
        assert_eq!(bench.starts(), 0);
    }

    #[test]
    fn a_runtime_changed_during_the_process_stops_the_commit() {
        let bench = bench(&format!("{}echo changed > \"$PWD/added\"\n", good_script()));
        let error = bench.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::InvalidPin, "{error:#}");
        assert_eq!(bench.saved(), None);
    }

    #[test]
    fn a_runtime_file_rewritten_during_the_process_stops_the_commit() {
        let bench = bench(&format!("{}: > \"$PWD/tool.sh\"\n", good_script()));
        let error = bench.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::InvalidPin, "{error:#}");
        assert_eq!(bench.saved(), None);
        assert_eq!(bench.starts(), 1);
    }

    /// Right before the exec, too: the admission's answer about who can write is not kept.
    #[test]
    fn a_runtime_someone_else_can_write_after_admission_never_starts() {
        use std::os::unix::fs::PermissionsExt as _;
        let bench = bench(&good_script());
        let script = bench.tool_dir.join("tool.sh");
        for mode in [0o664, 0o646] {
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(mode)).unwrap();
            let error = bench.call().unwrap_err();
            assert_eq!(kind(&error), RefusalKind::EffectUnavailable, "{mode:o}");
            assert_eq!(bench.starts(), 0, "{mode:o}");
            assert_eq!(bench.saved(), None, "{mode:o}");
        }
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::set_permissions(&bench.tool_dir, std::fs::Permissions::from_mode(0o775)).unwrap();
        let error = bench.call().unwrap_err();
        assert_eq!(
            kind(&error),
            RefusalKind::EffectUnavailable,
            "the directory"
        );
        assert_eq!(bench.starts(), 0);
    }

    /// The directory that holds the executable, too, right before the exec.
    #[test]
    fn an_executable_directory_someone_else_can_write_after_admission_never_starts() {
        use std::os::unix::fs::PermissionsExt as _;
        let bench = bench_with(
            &good_script(),
            Options {
                own_executable: true,
                ..Options::default()
            },
        );
        let directory = bench.workspace.parent().unwrap().join("executable");
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o775)).unwrap();
        let error = bench.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectUnavailable, "{error:#}");
        assert_eq!(bench.starts(), 0);
        assert_eq!(bench.saved(), None);
    }

    /// `{"mode":"fast"}` is 15 bytes: a limit of 15 takes it, and a limit of 14 starts nothing.
    #[test]
    fn the_input_limit_refuses_one_byte_over_it_and_takes_what_is_at_it() {
        let at = |max_input_bytes| {
            bench_with(
                &good_script(),
                Options {
                    max_input_bytes,
                    ..Options::default()
                },
            )
        };
        let over = at(14);
        let error = over.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectLimitExceeded, "{error:#}");
        assert_eq!(over.starts(), 0);
        assert_eq!(over.saved(), None);
        let at_limit = at(15);
        at_limit.call().unwrap();
        assert_eq!(at_limit.starts(), 1);
        assert_eq!(at_limit.saved().as_deref(), Some(CANDIDATE));
    }

    /// Whatever step fails, the call ends as unavailable, with no start and no save, and the
    /// record says nothing ran.
    #[test]
    fn a_launch_stopped_over_its_descriptors_is_unavailable_and_ran_nothing() {
        use super::super::isolation::test_hook::{DescriptorFault, descriptor_fault};
        let bench = bench(&good_script());
        {
            let _fault = descriptor_fault(DescriptorFault::MarkRefused);
            let error = bench.call().unwrap_err();
            assert_eq!(kind(&error), RefusalKind::EffectUnavailable, "{error:#}");
        }
        assert_eq!(bench.starts(), 0);
        assert_eq!(bench.saved(), None);
        let receipts = bench.tool.receipts.lock().unwrap();
        assert!(receipts.iter().all(|receipt| !receipt.executed));
    }

    /// A refusal's words are fixed: a classifier that reads words must not find a recoverable
    /// kind in what a path or a policy happens to say, or the model is asked to try again.
    #[test]
    fn no_refusal_takes_its_class_from_a_path_or_a_policy() {
        use crate::tools::registry::{recoverable_tool_error, tool_error_kind};
        use std::os::unix::fs::symlink;
        let bench = bench(&good_script());
        let named = "is a directory";
        std::fs::create_dir_all(bench.workspace.join(named)).unwrap();
        symlink(&bench.marks, bench.workspace.join(named).join("frozen")).unwrap();
        let mut context = bench.context();
        context.protected_paths = vec![format!("{named}/frozen")];
        let error = bench
            .tool
            .execute(&json!({"mode": "fast"}), &context, &|| false)
            .unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectPathRefused, "{error:#}");
        assert!(!error.to_string().contains(named), "{error:#}");
        assert_eq!(tool_error_kind(&error), "tool_execution_error");
        assert!(!recoverable_tool_error(&error));
        let refused = super::authorize(&bench.workspace, false, false).unwrap_err();
        let error = anyhow::Error::from(refused);
        assert_eq!(tool_error_kind(&error), "tool_execution_error", "{error:#}");
        assert!(!recoverable_tool_error(&error), "{error:#}");
    }

    #[test]
    fn a_hard_linked_input_is_refused_before_the_process() {
        let bench = bench(&good_script());
        std::fs::hard_link(
            bench.workspace.join(INPUT),
            bench.marks.join("another-name"),
        )
        .unwrap();
        let error = bench.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectPathRefused, "{error:#}");
        assert_eq!(bench.starts(), 0);
        let base = super::config_for_test(&bench.workspace);
        assert_eq!(
            bench.tool.preflight(&base).unwrap_err().kind,
            RefusalKind::EffectPathRefused
        );
    }

    const DUMMY: &str = "fake-unit-diagnostic-974-not-a-real-key";

    /// One failing call, with its event file, and a dummy secret in the scope.
    fn failing_call(script: &str, timeout_ms: u64) -> (Bench, Value, String) {
        let bench = bench_with(
            script,
            Options {
                timeout_ms,
                ..Options::default()
            },
        );
        let events = bench.marks.join("events.jsonl");
        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register_credential("DUMMY_API_KEY", DUMMY).unwrap();
        crate::sensitive_data::install_scope(catalog, Some(&bench.workspace), Some(&events));
        let mut context = bench.context();
        context.eval_events_path = Some(events.clone());
        let error = bench
            .tool
            .execute(&json!({"mode": "fast"}), &context, &|| false)
            .unwrap_err();
        let text = std::fs::read_to_string(&events).unwrap();
        let line = text
            .lines()
            .find(|line| line.contains("profile_tool_effect_rejected"))
            .unwrap_or_else(|| panic!("no rejection event: {text}"));
        (
            bench,
            serde_json::from_str(line).unwrap(),
            format!("{error:#}"),
        )
    }

    fn noisy_result(request_id: &str) -> String {
        let diagnostics = r#"[{"code":"note","message":"said a-plain-word and is a directory"}]"#;
        result_script(
            request_id,
            "$(field runtime_sha256)",
            "$(field input_revision)",
            "$(field schema_sha256)",
            CHECKS_PASSED,
            &good_output(),
        )
        .replace(
            r#""diagnostics":[]"#,
            &format!(r#""diagnostics":{diagnostics}"#),
        )
    }

    /// What the process said stays as data on the event, for each way of failing, and never in
    /// the message, which later steps classify by its words. A secret in it refuses it whole:
    /// see `a_secret_in_any_output_saves_nothing_and_leaves_no_words`.
    #[test]
    fn a_failed_process_leaves_its_words_on_the_event_and_not_in_the_message() {
        let stderr = "printf 'stderr says a-plain-word' >&2\n";
        let cases = [
            (
                "a nonzero exit",
                format!("{}{stderr}exit 3\n", noisy_result("$(field request_id)")),
                20_000,
                "process_failed",
            ),
            (
                "a clock",
                format!(
                    "{}{stderr}exec sleep 30\n",
                    noisy_result("$(field request_id)")
                ),
                500,
                "process_failed",
            ),
            (
                "a binding",
                format!("{}{stderr}", noisy_result("another-request")),
                20_000,
                "result_rejected",
            ),
        ];
        for (label, script, timeout_ms, expected) in cases {
            let (bench, event, message) = failing_call(&script, timeout_ms);
            assert_eq!(event["kind"], expected, "{label}");
            assert_eq!(event["diagnostics"][0]["code"], "note", "{label}: {event}");
            let said = event["diagnostics"][0]["message"].as_str().unwrap();
            assert!(said.contains("a-plain-word"), "{label}: {said}");
            let tail = event["stderr_tail"].as_str().unwrap();
            assert!(tail.contains("stderr says"), "{label}: {tail}");
            assert!(!message.contains("said"), "{label}: {message}");
            assert!(!message.contains("stderr says"), "{label}: {message}");
            assert_eq!(bench.saved(), None, "{label}");
        }
    }

    /// The forms a tool can print the dummy in, each one a line of a script.
    fn printed_forms() -> Vec<(String, String)> {
        let hex = DUMMY
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut forms = vec![
            ("as it is".to_string(), DUMMY.to_string()),
            ("hex".to_string(), hex.clone()),
            ("upper hex".to_string(), hex.to_ascii_uppercase()),
        ];
        for prefix in ["", "a", "ab"] {
            let encoded = encode_base64(format!("{prefix}{DUMMY}z").as_bytes());
            let url = encoded.replace('+', "-").replace('/', "_");
            let unpadded = encoded.trim_end_matches('=').to_string();
            forms.push((format!("base64 after {prefix:?}"), encoded));
            forms.push((format!("url-safe after {prefix:?}"), url));
            forms.push((format!("unpadded after {prefix:?}"), unpadded));
        }
        forms
    }

    /// Nothing of the secret anywhere a later step can read, and nothing else of the output.
    fn assert_refused_with_no_words(label: &str, bench: &Bench, event: &Value, message: &str) {
        assert_eq!(event["kind"], "result_rejected", "{label}: {event}");
        assert_eq!(event["diagnostics"], json!([]), "{label}: {event}");
        assert_eq!(event["stderr_tail"], "", "{label}: {event}");
        assert!(
            message.contains("secret_value_in_output"),
            "{label}: {message}"
        );
        assert!(!message.contains(DUMMY), "{label}: {message}");
        assert!(!event.to_string().contains(DUMMY), "{label}: {event}");
        assert_eq!(bench.saved(), None, "{label}");
        assert_eq!(bench.starts(), 1, "{label}");
        assert!(bench.tool.take_committed_paths().is_empty(), "{label}");
    }

    /// Each form on stdout and on stderr of a result that is otherwise good.
    #[test]
    fn a_secret_in_any_listed_form_on_either_stream_saves_nothing() {
        for (label, form) in printed_forms() {
            for (stream, line) in [
                ("stdout", format!("printf '{form}'\n")),
                ("stderr", format!("printf '{form}' >&2\n")),
            ] {
                let script = format!("{}{line}", good_script());
                let (bench, event, message) = failing_call(&script, 20_000);
                assert_refused_with_no_words(
                    &format!("{label} on {stream}"),
                    &bench,
                    &event,
                    &message,
                );
                assert!(!event.to_string().contains(&form), "{label}: {event}");
            }
        }
    }

    /// Whatever else was wrong with the run: this is judged before any of that is reported.
    #[test]
    fn a_secret_in_any_output_saves_nothing_and_leaves_no_words() {
        let diagnostics = format!(r#"[{{"code":"note","message":"said {DUMMY}"}}]"#);
        let in_diagnostics = |request_id: &str| {
            result_script(
                request_id,
                "$(field runtime_sha256)",
                "$(field input_revision)",
                "$(field schema_sha256)",
                CHECKS_PASSED,
                &good_output(),
            )
            .replace(
                r#""diagnostics":[]"#,
                &format!(r#""diagnostics":{diagnostics}"#),
            )
        };
        let spelled = DUMMY
            .chars()
            .map(|character| format!("\\\\u{:04x}", character as u32))
            .collect::<String>();
        let stderr = format!("printf '{DUMMY}' >&2\n");
        let cases = [
            (
                "diagnostics only",
                in_diagnostics("$(field request_id)"),
                20_000,
            ),
            (
                "an escape only a decoder sees",
                in_diagnostics("$(field request_id)").replace(DUMMY, &spelled),
                20_000,
            ),
            ("stderr only", format!("{}{stderr}", good_script()), 20_000),
            (
                "a nonzero exit",
                format!("{PRELUDE}{stderr}exit 3\n"),
                20_000,
            ),
            ("a clock", format!("{PRELUDE}{stderr}exec sleep 30\n"), 500),
            ("a binding", in_diagnostics("another-request"), 20_000),
        ];
        for (label, script, timeout_ms) in cases {
            let (bench, event, message) = failing_call(&script, timeout_ms);
            assert_refused_with_no_words(label, &bench, &event, &message);
        }
    }

    /// Bytes that are not text around the value, in the candidate, with a good result.
    #[test]
    fn a_secret_in_a_binary_candidate_saves_nothing() {
        let mut candidate = vec![0xff, 0x00, 0xfe];
        candidate.extend_from_slice(DUMMY.as_bytes());
        candidate.push(0xff);
        let outputs = format!(
            r#"[{{"id":"artifact","sha256":"{}","bytes_base64":"{}"}}]"#,
            sha256(&candidate),
            encode_base64(&candidate)
        );
        let script = result_script(
            "$(field request_id)",
            "$(field runtime_sha256)",
            "$(field input_revision)",
            "$(field schema_sha256)",
            CHECKS_PASSED,
            &outputs,
        );
        let (bench, event, message) = failing_call(&script, 20_000);
        assert_refused_with_no_words("a binary candidate", &bench, &event, &message);
    }

    const NOISE: &str = r#"[{"code":"note","message":"said a-plain-word"}]"#;

    /// An envelope carrying these outputs, with ordinary diagnostics, and ordinary stderr.
    fn noisy_envelope(request_id: &str, checks: &str, outputs: &str) -> String {
        let result = result_script(
            request_id,
            "$(field runtime_sha256)",
            "$(field input_revision)",
            "$(field schema_sha256)",
            checks,
            outputs,
        )
        .replace(r#""diagnostics":[]"#, &format!(r#""diagnostics":{NOISE}"#));
        format!("{result}printf 'stderr says a-plain-word' >&2\n")
    }

    /// Only decoding the candidate shows these: the wire is one more base64 around them. However
    /// the run ended, the verdict comes first and nothing else of the output stays.
    #[test]
    fn a_secret_only_the_decoded_candidate_shows_leaves_no_words_however_the_run_ended() {
        let hex = DUMMY
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let judged = SecretForms::new([DUMMY]);
        for (form, candidate) in [("hex", hex), ("base64", encode_base64(DUMMY.as_bytes()))] {
            let wire = encode_base64(candidate.as_bytes());
            assert!(judged.found_in(candidate.as_bytes()), "{form}");
            assert!(
                !judged.found_in(wire.as_bytes()),
                "{form}: the wire shows it"
            );
            let outputs = format!(
                r#"[{{"id":"artifact","sha256":"{}","bytes_base64":"{wire}"}}]"#,
                sha256(candidate.as_bytes())
            );
            let good = "$(field request_id)";
            let failed = r#"[{"id":"a","status":"failed"}]"#;
            for (label, script) in [
                ("a good run", noisy_envelope(good, CHECKS_PASSED, &outputs)),
                (
                    "a nonzero exit",
                    format!("{}exit 3\n", noisy_envelope(good, CHECKS_PASSED, &outputs)),
                ),
                (
                    "a binding",
                    noisy_envelope("another-request", CHECKS_PASSED, &outputs),
                ),
                ("a failed check", noisy_envelope(good, failed, &outputs)),
            ] {
                let label = format!("{form} and {label}");
                let (bench, event, message) = failing_call(&script, 20_000);
                assert_refused_with_no_words(&label, &bench, &event, &message);
                for shown in [&candidate, &wire] {
                    assert!(!message.contains(shown), "{label}: {message}");
                    assert!(!event.to_string().contains(shown), "{label}: {event}");
                }
            }
        }
    }

    /// A candidate that does not decode matches nothing, and the refusal it always had stands,
    /// with what the process said as data.
    #[test]
    fn a_candidate_that_does_not_decode_keeps_its_own_refusal_and_its_words() {
        let outputs = format!(
            r#"[{{"id":"artifact","sha256":"{}","bytes_base64":"not base64!"}}]"#,
            "0".repeat(64)
        );
        let script = noisy_envelope("$(field request_id)", CHECKS_PASSED, &outputs);
        let (bench, event, message) = failing_call(&script, 20_000);
        assert_eq!(event["kind"], "result_rejected", "{event}");
        assert!(message.contains("output_not_base64"), "{message}");
        assert!(!message.contains("said"), "{message}");
        assert_eq!(event["diagnostics"][0]["code"], "note", "{event}");
        let tail = event["stderr_tail"].as_str().unwrap();
        assert!(tail.contains("stderr says"), "{tail}");
        assert_eq!(bench.saved(), None);
    }

    /// A value the run has no way to judge is not an empty set: nothing starts.
    #[test]
    fn secrets_that_cannot_be_read_stop_the_call_before_the_process() {
        // Too short to be held as a secret, which is why it cannot be read, and no word that a
        // backtrace, a path or a symbol could show.
        const UNREADABLE: &str = "Zq7_x3";
        assert!(UNREADABLE.len() < crate::sensitive_data::MIN_GENERIC_SECRET_LEN);
        let bench = bench(&good_script());
        std::fs::write(
            bench.workspace.join(".env"),
            format!("DUMMY_API_KEY={UNREADABLE}\n"),
        )
        .unwrap();
        let error = bench.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectEnvRefused, "{error:#}");
        assert_eq!(bench.starts(), 0);
        assert_eq!(bench.saved(), None);
        assert!(
            !format!("{error:#} {error:?}").contains(UNREADABLE),
            "{error:#}"
        );
    }

    /// A workspace's own value counts, and nothing leaks from a scope of another workspace.
    #[test]
    fn a_value_of_another_workspace_is_not_this_runs_secret() {
        let other = tempfile::tempdir().unwrap();
        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register_credential("DUMMY_API_KEY", DUMMY).unwrap();
        crate::sensitive_data::install_scope(catalog, Some(other.path()), None);
        let bench = bench(&format!("{}printf '{DUMMY}' >&2\n", good_script()));
        bench.call().unwrap();
        assert_eq!(bench.saved().as_deref(), Some(CANDIDATE));
    }

    #[test]
    fn the_stderr_on_the_event_is_bounded() {
        let script = format!("{PRELUDE}head -c 5000 /dev/zero | tr '\\0' 'x' >&2\nexit 3\n");
        let (_bench, event, _) = failing_call(&script, 20_000);
        let tail = event["stderr_tail"].as_str().unwrap();
        assert_eq!(tail.len(), 2048);
    }

    /// The model gets the receipt. What the process said stays with the host.
    #[test]
    fn a_success_keeps_what_the_process_said_from_the_tool_result() {
        let bench = bench(&noisy_result("$(field request_id)"));
        let text = bench.call().unwrap();
        let receipt: Value = serde_json::from_str(&text).unwrap();
        assert!(receipt.get("diagnostics").is_none(), "{text}");
        assert!(!text.contains("said"), "{text}");
        assert_eq!(receipt["committed"], true);
        let commits = bench.tool.commits.lock().unwrap();
        assert_eq!(commits[0].diagnostics[0].code, "note");
    }

    #[test]
    fn a_shared_kind_keeps_where_it_came_from_and_its_word() {
        for kind in [
            RefusalKind::Unavailable,
            RefusalKind::InvalidPin,
            RefusalKind::InvalidDescriptor,
            RefusalKind::ConflictingDeclaration,
            RefusalKind::WorkspaceDescriptor,
            RefusalKind::UnknownToolId,
            RefusalKind::RegistryUnavailable,
            RefusalKind::NameCollision,
        ] {
            let marked = super::with_effect_origin(ProfileToolRefusal::new(kind, "why"));
            assert_eq!(marked.kind.as_str(), kind.as_str());
            assert!(marked.kind.is_effect_tool_reason(), "{kind:?}");
            assert!(!kind.is_effect_tool_reason(), "{kind:?}");
        }
    }

    #[test]
    fn a_runtime_changed_before_the_call_never_starts() {
        let bench = bench(&good_script());
        std::fs::write(bench.tool_dir.join("added"), "x").unwrap();
        let error = bench.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::InvalidPin, "{error:#}");
        assert_eq!(bench.starts(), 0);
    }

    #[test]
    fn an_input_changed_during_the_process_stops_the_commit() {
        let bench = bench(&format!(
            "cd \"$MARKS/../workspace\" || exit 9\necho changed >> {INPUT}\ncd \"$OLDPWD\"\n{}",
            good_script()
        ));
        let error = bench.call().unwrap_err();
        assert_eq!(
            kind(&error),
            RefusalKind::InputRevisionMismatch,
            "{error:#}"
        );
        assert_eq!(bench.saved(), None);
    }

    #[test]
    fn an_argument_the_schema_refuses_goes_back_as_data_and_starts_nothing() {
        let bench = bench(&good_script());
        let error = bench
            .tool
            .execute(&json!({"mode": 1}), &bench.context(), &|| false)
            .unwrap_err();
        assert!(
            error.to_string().starts_with("extension_tool_rejected"),
            "{error:#}"
        );
        assert!(crate::tools::registry::recoverable_tool_error(&error));
        assert_eq!(bench.starts(), 0);
    }

    #[test]
    fn both_registry_functions_reach_the_same_guard_and_the_allowlist_holds() {
        let bench = bench(&good_script());
        let registry = bench.registry();
        assert!(
            registry
                .specs()
                .iter()
                .any(|spec| spec.function.name == "ProposeUnit")
        );
        for removed in ["Write", "Edit", "Bash"] {
            let error = registry
                .execute(removed, &json!({"path": OUTPUT}), &bench.context())
                .unwrap_err();
            assert!(error.to_string().contains("unknown tool"), "{removed}");
        }
        let mut context = bench.context();
        context.mode = ExecutionMode::Plan;
        let arguments = json!({"mode": "fast"});
        assert_eq!(
            kind(
                &registry
                    .execute("ProposeUnit", &arguments, &context)
                    .unwrap_err()
            ),
            RefusalKind::EffectReadOnly
        );
        assert_eq!(
            kind(
                &registry
                    .execute_with_cancel("ProposeUnit", &arguments, &context, || false, || false)
                    .unwrap_err()
            ),
            RefusalKind::EffectReadOnly
        );
        assert_eq!(bench.starts(), 0);
        registry
            .execute("ProposeUnit", &arguments, &bench.context())
            .unwrap();
        assert_eq!(bench.saved().as_deref(), Some(CANDIDATE));
        assert_eq!(registry.take_committed_paths(), [OUTPUT.to_string()]);
    }

    #[test]
    fn a_builtin_name_is_refused_even_after_it_was_removed() {
        let bench = bench(&good_script());
        let mut spec_tool = bench.registry().specs().to_vec();
        spec_tool.retain(|spec| spec.function.name == "ProposeUnit");
        assert_eq!(spec_tool.len(), 1);
        for builtin in ["Write", "Read", "Bash"] {
            let renamed = bench_named(builtin);
            let error = ToolRegistry::with_profile_tools_and_proposals(
                [],
                [renamed.tool.clone()],
                &["Read".to_string()],
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("duplicate tool name"),
                "{builtin}"
            );
        }
    }

    fn bench_named(name: &str) -> Bench {
        let mut bench = bench(&good_script());
        let tool = Arc::get_mut(&mut bench.tool).unwrap();
        tool.spec.function.name = name.to_string();
        bench
    }

    #[test]
    fn a_cancel_and_a_force_cancel_reach_the_process_through_the_registry() {
        for force in [false, true] {
            let bench = bench(&format!("{PRELUDE}exec sleep 30\n"));
            let registry = bench.registry();
            let marks = bench.marks.clone();
            let started_file = move || marks.join("started").exists();
            let begun = std::time::Instant::now();
            let fired = |wanted: bool| wanted && started_file();
            let error = registry
                .execute_with_cancel(
                    "ProposeUnit",
                    &json!({"mode": "fast"}),
                    &bench.context(),
                    || fired(!force),
                    || fired(force),
                )
                .unwrap_err();
            assert!(error.to_string().contains("cancelled"), "{error:#}");
            assert!(begun.elapsed() < std::time::Duration::from_secs(10));
            assert_eq!(bench.saved(), None);
        }
    }

    #[test]
    fn a_cancel_before_the_call_starts_nothing() {
        let bench = bench(&good_script());
        let error = bench
            .tool
            .execute(&json!({"mode": "fast"}), &bench.context(), &|| true)
            .unwrap_err();
        assert_eq!(kind(&error), RefusalKind::ProcessFailed);
        assert_eq!(bench.starts(), 0);
    }

    /// The cancel turns true the moment the destination holds the new bytes: the commit
    /// stands, and the receipt says so and says the cancel came after.
    #[test]
    fn a_cancel_after_the_commit_keeps_the_receipt() {
        let bench = bench(&good_script());
        let target = bench.workspace.join(OUTPUT);
        let text = bench
            .tool
            .execute(&json!({"mode": "fast"}), &bench.context(), &|| {
                target.exists()
            })
            .unwrap();
        let receipt: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(receipt["committed"], true);
        assert_eq!(receipt["cancelled_after_commit"], true);
        assert_eq!(bench.saved().as_deref(), Some(CANDIDATE));
        assert_eq!(bench.tool.take_committed_paths(), [OUTPUT.to_string()]);
    }

    #[test]
    fn usage_stays_unknown() {
        let bench = bench(&good_script());
        bench.call().unwrap();
        let receipts = bench.tool.receipts.lock().unwrap();
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].executed && receipts[0].ok);
        assert_eq!(receipts[0].usage, None);
    }

    #[test]
    fn the_start_refuses_an_action_that_plans_no_authority_a_protected_path_and_a_scrubbed_value() {
        let bench = bench_with(
            &good_script(),
            Options {
                env: vec![("TOOL_TOKEN", "fake-unit-credential-974-not-a-real-key")],
                ..Options::default()
            },
        );
        let base = super::config_for_test(&bench.workspace);
        let kind_of = |config: &crate::config::Config| {
            bench.tool.preflight(config).map_err(|refused| refused.kind)
        };

        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog
            .register_credential("DUMMY_API_KEY", "fake-unit-credential-974-not-a-real-key")
            .unwrap();
        crate::sensitive_data::install_scope(catalog, Some(&bench.workspace), None);
        assert_eq!(kind_of(&base), Err(RefusalKind::EffectEnvRefused));

        let mut planning = base.clone();
        planning.action = crate::config::Action::PlanSteps("goal".to_string());
        assert_eq!(kind_of(&planning), Err(RefusalKind::EffectReadOnly));
        let mut planning = base.clone();
        planning.action = crate::config::Action::UltraPlan("goal".to_string());
        assert_eq!(kind_of(&planning), Err(RefusalKind::EffectReadOnly));

        let mut unapproved = base.clone();
        unapproved.yes = false;
        assert_eq!(kind_of(&unapproved), Err(RefusalKind::EffectReadOnly));

        {
            let _policy = allow_policy::install(false, &[AllowTarget::Read]);
            assert_eq!(kind_of(&base), Err(RefusalKind::EffectReadOnly));
        }

        let contract = bench.marks.join("contract.json");
        std::fs::write(&contract, json!({"protected_paths": ["out"]}).to_string()).unwrap();
        let mut protected = base.clone();
        protected.completion_contract_path = Some(contract);
        assert_eq!(kind_of(&protected), Err(RefusalKind::EffectPathRefused));
        assert_eq!(bench.starts(), 0);
    }

    #[test]
    fn the_start_accepts_a_clean_context_and_refuses_what_is_at_the_destination() {
        let bench = bench(&good_script());
        let base = super::config_for_test(&bench.workspace);
        assert_eq!(bench.tool.preflight(&base), Ok(()));
        std::fs::create_dir_all(bench.workspace.join("out")).unwrap();
        std::os::unix::fs::symlink(&bench.marks, bench.workspace.join(OUTPUT)).unwrap();
        assert_eq!(
            bench.tool.preflight(&base).unwrap_err().kind,
            RefusalKind::EffectPathRefused
        );
    }

    #[test]
    fn a_session_that_can_only_read_is_refused() {
        let bench = bench(&good_script());
        assert!(bench.tool.preflight_session(SessionAccess::Normal).is_ok());
        for access in [
            SessionAccess::InspectStep,
            SessionAccess::ScopedRecoveryReads,
        ] {
            assert_eq!(
                bench.tool.preflight_session(access).unwrap_err().kind,
                RefusalKind::EffectReadOnly
            );
        }
    }

    #[test]
    fn a_descriptor_output_or_env_outside_the_rules_is_refused_by_its_own_check() {
        let bench = bench(&good_script());
        let declared = bench.tool.declaration.clone();
        let check = |change: &dyn Fn(&mut ProcessDeclaration)| {
            let mut declaration = declared.clone();
            change(&mut declaration);
            declaration.validate("t").unwrap_err().kind
        };
        assert_eq!(
            check(&|d| d.outputs.push(OutputDeclaration {
                id: "second".to_string(),
                path: "second.json".to_string(),
            })),
            RefusalKind::EffectOutputRefused
        );
        assert_eq!(
            check(&|d| d.outputs.clear()),
            RefusalKind::EffectOutputRefused
        );
        assert_eq!(
            check(&|d| d.limits.timeout_ms = 30_001),
            RefusalKind::EffectLimitExceeded
        );
        assert_eq!(
            check(&|d| d.limits.timeout_ms = 0),
            RefusalKind::EffectLimitExceeded
        );
        assert_eq!(
            check(&|d| d.limits.max_result_bytes = 64 * 1024 + 1),
            RefusalKind::EffectLimitExceeded
        );
        assert_eq!(
            check(&|d| {
                d.env
                    .insert("NODE_OPTIONS".to_string(), "--inspect".to_string());
            }),
            RefusalKind::EffectEnvRefused
        );
        assert_eq!(
            check(&|d| {
                d.env.clear();
                for index in 0..17 {
                    d.env.insert(format!("KEY_{index}"), "v".to_string());
                }
            }),
            RefusalKind::EffectLimitExceeded
        );
        assert_eq!(
            check(&|d| {
                d.env.insert("LONG".to_string(), "v".repeat(1025));
            }),
            RefusalKind::EffectLimitExceeded
        );
        assert_eq!(
            check(&|d| d.cwd = PathBuf::from("/")),
            RefusalKind::InvalidDescriptor
        );
        let mut at_edge = declared.clone();
        at_edge.limits.timeout_ms = 30_000;
        at_edge.limits.max_result_bytes = 64 * 1024;
        at_edge.env.clear();
        for index in 0..16 {
            at_edge.env.insert(format!("KEY_{index}"), "v".repeat(1024));
        }
        assert!(at_edge.validate("t").is_ok());
    }

    #[test]
    fn a_path_outside_the_rules_is_refused_by_the_shared_judgement() {
        let bench = bench(&good_script());
        let policy = WorkspacePolicy::for_task_request();
        for path in [
            "../outside.json",
            "/etc/passwd",
            ".env",
            ".git/config",
            ".anvil/state.json",
            "secrets/.ssh/id_rsa",
        ] {
            assert_eq!(
                super::judge_path(&bench.workspace, path, &[], policy, "t", "output")
                    .unwrap_err()
                    .kind,
                RefusalKind::EffectPathRefused,
                "{path}"
            );
        }
        assert!(
            super::judge_path(&bench.workspace, "out/a.json", &[], policy, "t", "output").is_ok()
        );
        assert!(
            super::judge_path(
                Path::new(&bench.workspace),
                "out/a.json",
                &["out".to_string()],
                policy,
                "t",
                "output"
            )
            .is_err()
        );
    }
}
