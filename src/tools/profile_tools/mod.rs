//! Operator-pinned tools a selected profile or pack may reference (Issue #972).
//!
//! Trust is split three ways:
//!
//! - A profile manifest or an eval pack carries only a versioned opt-in
//!   declaration ([`selection::ProfileToolDeclaration`]): opaque tool IDs, the
//!   builtin tools it wants to keep, and limits it may only narrow. It cannot
//!   name an executable, argv, module URL, expected hash, or permission.
//! - The operator owns the registry ([`descriptor`]): `profile-tools/<id>.json`
//!   below the operator root, outside the workspace, set before the run.
//! - The host admits the selected declarations before any provider call
//!   ([`admission`]), publishes only the admitted set, and enforces the same set
//!   again when a tool call arrives. Pins are compared at admission and again
//!   right before an execution.
//!
//! No declaration means off: the session keeps the default registry and nothing
//! here reads a file, starts a process, or emits an event.
//!
//! A descriptor that declares a `schema` takes nested arguments through
//! [`schema`] (G2). The flat `object` and `array` parameter kinds stay refused as
//! unavailable. Nothing here reports them, or an unknown cost, as a success.
//!
//! A descriptor with effect `propose-artifacts` and a pinned process (G3) proposes bytes
//! through [`effects`]: [`isolation`] starts it, [`process`] bounds it, [`protocol`] judges
//! its result, and only [`commit`] changes the workspace, once.

pub mod admission;
pub mod commit;
pub mod descriptor;
pub mod effects;
pub(crate) mod isolation;
pub(crate) mod pre_run;
pub(crate) mod process;
pub mod protocol;
pub mod schema;
mod secret_output;
pub mod selection;

use std::fmt;
use std::path::Path;

pub use admission::{
    AdmittedProfileTools, AdmittedToolBinding, ProfileToolReadExecutor, ReadToolOutput,
    ToolExecutionReceipt, ToolUsage, admit_selected, register_read_executor,
};
pub use commit::CommitReceipt;
pub use descriptor::directory_pin;
pub use schema::{
    ProfileToolSchemaCompiler, SchemaCompileInput, SchemaInput, register_schema_compiler,
};
pub use selection::{DeclaredTool, ProfileToolDeclaration};

/// Why a selected tool set was refused. Every refusal happens before a provider
/// call; the kind is part of the message so a caller and a test can tell them
/// apart without new event vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalKind {
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
    /// The strict isolation a proposal tool needs is missing. Reported as `unavailable`;
    /// a separate variant so a REPL's free text can stop on it without moving where a
    /// G1 tool's `unavailable` is reported.
    EffectUnavailable,
    /// A proposal tool's runtime differs from its pin at admission. Reported as
    /// `invalid_pin`, kept apart for the same reason.
    EffectInvalidPin,
    /// The same, for the other kinds a proposal tool's admission shares with G1. Each
    /// reports the shared word, so only the place it stops a REPL's free text differs.
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
    /// A declared stage set cannot be admitted, here or for this session.
    StageContractRefused,
    /// A stage tool's record of attempts cannot be vouched for.
    StageLedgerRefused,
    /// A limit, a changed fixed input, or an unfinished commit: no later call can get past it.
    StageRunEnded,
    /// A session wanted to end with a stage obligation it answers for unmet.
    StageIncomplete,
}

impl RefusalKind {
    /// Only a descriptor with a G2 `schema` can produce these.
    pub(crate) const fn is_schema_tool_reason(self) -> bool {
        matches!(
            self,
            Self::SchemaUnsupported
                | Self::SchemaLimitExceeded
                | Self::SchemaScrubbed
                | Self::SchemaMismatch
                | Self::InputRevisionUnavailable
                | Self::InputRevisionMismatch
        )
    }

    /// Only a `propose-artifacts` tool's admission can produce these.
    pub(crate) const fn is_effect_tool_reason(self) -> bool {
        matches!(
            self,
            Self::EffectUnavailable
                | Self::EffectInvalidPin
                | Self::EffectInvalidDescriptor
                | Self::EffectConflictingDeclaration
                | Self::EffectWorkspaceDescriptor
                | Self::EffectUnknownToolId
                | Self::EffectRegistryUnavailable
                | Self::EffectNameCollision
                | Self::EffectReadOnly
                | Self::EffectLimitExceeded
                | Self::EffectOutputRefused
                | Self::EffectPathRefused
                | Self::EffectEnvRefused
                | Self::StageContractRefused
                | Self::StageLedgerRefused
        )
    }

    /// A shared kind that came from a proposal tool's admission keeps that origin, so it
    /// is not mistaken for a G1 refusal, whose place stays where it was.
    pub(crate) const fn with_effect_origin(self) -> Self {
        match self {
            Self::Unavailable => Self::EffectUnavailable,
            Self::InvalidPin => Self::EffectInvalidPin,
            Self::InvalidDescriptor => Self::EffectInvalidDescriptor,
            Self::ConflictingDeclaration => Self::EffectConflictingDeclaration,
            Self::WorkspaceDescriptor => Self::EffectWorkspaceDescriptor,
            Self::UnknownToolId => Self::EffectUnknownToolId,
            Self::RegistryUnavailable => Self::EffectRegistryUnavailable,
            Self::NameCollision => Self::EffectNameCollision,
            other => other,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownToolId => "unknown_tool_id",
            Self::NameCollision => "name_collision",
            Self::ConflictingDeclaration => "conflicting_declaration",
            Self::InvalidPin => "invalid_pin",
            Self::WorkspaceDescriptor => "workspace_descriptor",
            Self::InvalidDescriptor => "invalid_descriptor",
            Self::RegistryUnavailable => "registry_unavailable",
            Self::Unavailable => "unavailable",
            Self::SelectedPackInvalid => "selected_pack_invalid",
            Self::SchemaUnsupported => "schema_unsupported",
            Self::SchemaLimitExceeded => "schema_limit_exceeded",
            Self::SchemaScrubbed => "schema_scrubbed",
            Self::SchemaMismatch => "schema_mismatch",
            Self::InputRevisionUnavailable => "input_revision_unavailable",
            Self::InputRevisionMismatch => "input_revision_mismatch",
            Self::SchemaViolation => "schema_violation",
            Self::EffectUnavailable => "unavailable",
            Self::EffectInvalidPin => "invalid_pin",
            Self::EffectInvalidDescriptor => "invalid_descriptor",
            Self::EffectConflictingDeclaration => "conflicting_declaration",
            Self::EffectWorkspaceDescriptor => "workspace_descriptor",
            Self::EffectUnknownToolId => "unknown_tool_id",
            Self::EffectRegistryUnavailable => "registry_unavailable",
            Self::EffectNameCollision => "name_collision",
            Self::EffectReadOnly => "effect_read_only",
            Self::EffectLimitExceeded => "effect_limit_exceeded",
            Self::EffectOutputRefused => "effect_output_refused",
            Self::EffectPathRefused => "effect_path_refused",
            Self::EffectEnvRefused => "effect_env_refused",
            Self::ProcessFailed => "process_failed",
            Self::ResultRejected => "result_rejected",
            Self::CommitFailed => "commit_failed",
            Self::StageContractRefused => "stage_contract_refused",
            Self::StageLedgerRefused => "stage_ledger_refused",
            Self::StageRunEnded => "stage_run_ended",
            Self::StageIncomplete => "stage_incomplete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileToolRefusal {
    pub kind: RefusalKind,
    pub reason: String,
    /// What an operator needs and a message must not carry. It reaches the event and the
    /// display. `reason` stays words a later step may read.
    pub(crate) data: Option<serde_json::Value>,
}

impl ProfileToolRefusal {
    pub(crate) fn new(kind: RefusalKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            reason: reason.into(),
            data: None,
        }
    }
}

impl fmt::Display for ProfileToolRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "profile_tool_refused: {}: {}",
            self.kind.as_str(),
            self.reason
        )
    }
}

impl std::error::Error for ProfileToolRefusal {}

/// The pre-provider check for an entry that holds the client its sessions will use.
/// It keeps nothing but a schema tool's adoption: every session admits its own set.
/// Off returns without reading anything.
pub(crate) fn refuse_for_client(
    config: &crate::config::Config,
    client: &dyn crate::providers::ChatClient,
) -> anyhow::Result<()> {
    if let Some(admitted) = admit_selected(&config.workspace_root, &config.profile, None)? {
        admitted.preflight_schema_tools(config, client.profile_tool_schema_wire())?;
    }
    Ok(())
}

/// The same check for an entry that holds no session client: a planner call, or a
/// workflow. The executor role of the configuration says what its sessions will use.
pub(crate) fn refuse_before_provider(
    workspace: &Path,
    config: &crate::config::Config,
) -> anyhow::Result<()> {
    if let Some(admitted) = admit_selected(workspace, &config.profile, None)? {
        admitted.preflight_schema_tools(config, configured_wire(config))?;
    }
    Ok(())
}

fn configured_wire(
    config: &crate::config::Config,
) -> crate::providers::profile_tool_schema::ProfileToolSchemaWire {
    crate::providers::profile_tool_schema::wire_for_role(
        config,
        crate::config::ProviderRole::Executor,
    )
}
