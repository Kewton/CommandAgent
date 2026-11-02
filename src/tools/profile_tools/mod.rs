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
//! unavailable, and so does `propose-artifacts` (G3). Nothing here reports them,
//! or an unknown cost, as a success.

pub mod admission;
pub mod descriptor;
pub(crate) mod pre_run;
pub mod schema;
pub mod selection;

use std::fmt;
use std::path::Path;

pub use admission::{
    AdmittedProfileTools, AdmittedToolBinding, ProfileToolReadExecutor, ReadToolOutput,
    ToolExecutionReceipt, ToolUsage, admit_selected, register_read_executor,
};
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileToolRefusal {
    pub kind: RefusalKind,
    pub reason: String,
}

impl ProfileToolRefusal {
    pub(crate) fn new(kind: RefusalKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            reason: reason.into(),
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
