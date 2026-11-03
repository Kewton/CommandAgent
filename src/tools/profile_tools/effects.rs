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
    CommitObservation, ExhaustedLimit, InvalidatedTarget, LocalDecision, ObservedBytes,
    TerminalFailure, judge_checks,
};
use crate::planner::structured_stages::{StageCallFacts, StageMember, StageRun, ledger_refused};
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
        let ticket = match decision {
            BeginDecision::Proceed(ticket) => ticket,
            other => return self.answer_without_process(other, &ledger, target, notes),
        };
        let outcome =
            self.stage_attempt(call, &ledger, (&contract, target), &ticket, &asked, notes);
        if let Err(error) = &outcome {
            // Past the intent the permit has answered already, and this is refused as stale.
            if let Ok(mut held) = locked(&ledger) {
                let failure = terminal_failure(error.kind, notes.secret_in_output);
                let _ = held.record_failure(&ticket, AttemptFailure::Terminal(failure));
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
        if let Err(kind) = judge_checks(&target.required_checks, &result.checks) {
            locked(ledger)?
                .record_failure(ticket, AttemptFailure::Check(kind))
                .map_err(ledger_refused)?;
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

fn now_ms() -> u64 {
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
        RefusalKind::CommitFailed => TerminalFailure::CommitBeforeIntent,
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

/// What an operator needs to act on a refusal that carries more than its message may. The path
/// and the input's name are theirs, and go nowhere but here and the event.
fn stage_refusal_notice(data: &Value) -> String {
    let field = |name: &str| data.get(name).and_then(Value::as_str);
    let mut lines = vec![format!(
        "stage record refused: {}",
        field("code").unwrap_or("unknown")
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
    eval_events::scrub_sensitive_text(&lines.join("\n"))
}

fn emit_stage_refusal(context: &ToolContext, tool: &ProposalTool, data: &Value) {
    eprintln!("{}", stage_refusal_notice(data));
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
        use super::super::super::schema::invocation::ReceiveState;
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
        use crate::planner::structured_stages::{StageMember, StageRun};
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
                } = self;
                // Everything of the old one goes first: its ledger's lock, and its guard, whose
                // end would otherwise come after the new one's start.
                drop((_guard, workspace, marks, upper, lower, run));
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

            let bench = bench.restarted(LedgerEntry::Fresh);
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
                notice.contains("only `--fresh-session` goes on"),
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
