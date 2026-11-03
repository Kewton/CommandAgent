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
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};

use super::admission::{ToolExecutionReceipt, ToolUsage};
use super::commit::{self, CommitPlan, CommitReceipt};
use super::descriptor::{self, Descriptor, ProcessPins, RegistryBinding};
use super::isolation::{self, StrictLaunch};
use super::process::{self, RunLimits};
use super::protocol::{self, Binding, Diagnostic, ProcessDeclaration};
use super::schema::SchemaInput;
use super::schema::invocation::SchemaGuard;
use super::secret_output::SecretForms;
use super::selection::DeclaredTool;
use super::{ProfileToolRefusal, RefusalKind};
use crate::config::{Action, Config, OpenAiCompatibleConfig};
use crate::eval_events;
use crate::mode::ExecutionMode;
use crate::tools::registry::{FunctionSpec, ToolContext, ToolSpec};
use crate::tools::workspace_policy::WorkspacePolicy;

pub const EFFECT_PROPOSE_ARTIFACTS: &str = "propose-artifacts";
pub const RUNTIME_PINNED_PROCESS: &str = "pinned_process";
const TRANSPORT: &str = "profile_tool";
const STAGE_PROCESS: &str = "before_process";
const STAGE_COMMIT: &str = "before_commit";

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
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.spec.function.name
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
        self.guard
            .adopted_inputs()
            .iter()
            .map(|input| input.path.clone())
            .collect()
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
                self.record(true, true);
                Ok(text)
            }
            Err(error) => {
                self.record(notes.executed, false);
                emit_rejection(context, self, &error, &notes);
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
        let binding = Binding {
            request_id: uuid::Uuid::now_v7().to_string(),
            tool_id: self.tool_id.clone(),
            tool_version: self.tool_version.clone(),
            descriptor_sha256: self.descriptor_sha256.clone(),
            runtime_sha256: self.runtime_sha256.clone(),
            input_revision: self.guard.adopted_revision().to_string(),
            schema_sha256: self.guard.adopted_schema_sha256().to_string(),
        };
        let inputs: &[SchemaInput] = self.guard.adopted_inputs();
        let output = self.declaration.single_output();
        let request = protocol::request_bytes(&binding, arguments, inputs, output);
        notes.executed = true;
        let observation = self
            .launch(&request, is_cancelled)
            .inspect_err(|error| notes.executed = error.kind != RefusalKind::EffectUnavailable)?;
        // Before anything keeps, cuts, or reports a byte of it: with a secret in it, nothing of
        // what the process said goes further, however the process ended.
        if secrets.found_in(&observation.stdout)
            || secrets.found_in(&observation.stderr)
            || protocol::decode_document(&observation.stdout)
                .is_some_and(|document| secrets.found_in_document(&document))
        {
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
        if secrets.found_in(&accepted.bytes) {
            return Err(secret_in_output(&self.tool_id));
        }
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

    /// A value the run has no way to judge is not an empty set: nothing starts.
    #[test]
    fn secrets_that_cannot_be_read_stop_the_call_before_the_process() {
        let bench = bench(&good_script());
        std::fs::write(bench.workspace.join(".env"), "DUMMY_API_KEY=short\n").unwrap();
        let error = bench.call().unwrap_err();
        assert_eq!(kind(&error), RefusalKind::EffectEnvRefused, "{error:#}");
        assert_eq!(bench.starts(), 0);
        assert_eq!(bench.saved(), None);
        assert!(
            !format!("{error:#} {error:?}").contains("short"),
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
