//! Pre-provider admission of the selected tool set, and the immutable bindings
//! later stages receive.
//!
//! G1 runs only `read` tools with the existing closed argument types, through an
//! in-process executor a Rust caller registered. No configuration file can
//! register one, so a descriptor naming a runtime nobody registered is refused
//! as unavailable. Nothing here lifts acceptance or assurance: a receipt records
//! that a tool ran, never that an artifact or stage passed.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::descriptor::{self, Descriptor, EFFECT_READ, LoadedDescriptor, RUNTIME_IN_PROCESS};
use super::selection::{self, DeclaredTool, PackSource, SelectedTools};
use super::{ProfileToolRefusal, RefusalKind};
use crate::tools::extension::{ExtensionExecutor, ExtensionNetworkAccess, ExtensionTool};
use crate::tools::registry::ToolRegistry;

/// The one required feature G1 can check: the OS write sandbox is in force for
/// this run. Every other feature is unavailable; hash pins and a cleared
/// environment are not isolation and never stand in for it.
const FEATURE_OS_SANDBOX: &str = "os_sandbox";

/// Usage a tool reported. A field it did not report stays `None`: unknown, not
/// free.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_usd_micros: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadToolOutput {
    pub text: String,
    /// `None` when the executor knows nothing about usage.
    pub usage: Option<ToolUsage>,
}

/// A read-only in-process executor. It receives only arguments that already
/// passed the closed type checks and the workspace path rules.
pub trait ProfileToolReadExecutor: fmt::Debug + Send + Sync {
    fn read(&self, workspace_root: &Path, arguments: &Value) -> anyhow::Result<ReadToolOutput>;
}

type ExecutorTable = BTreeMap<String, Arc<dyn ProfileToolReadExecutor>>;

static READ_EXECUTORS: Mutex<ExecutorTable> = Mutex::new(BTreeMap::new());

/// Make an executor available under a runtime ID. This is the only way to add
/// one; the `commandagent` binary registers none in G1.
pub fn register_read_executor(runtime_id: &str, executor: Arc<dyn ProfileToolReadExecutor>) {
    READ_EXECUTORS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(runtime_id.to_string(), executor);
}

/// What G2 and G3 receive for one admitted tool. The fields are private and only
/// readable, so nothing after admission can change a pin, an effect, or a limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedToolBinding {
    run_id: Option<String>,
    profile: String,
    tool_id: String,
    name: String,
    version: String,
    schema_sha256: String,
    descriptor_sha256: String,
    effect: String,
    max_input_bytes: usize,
    max_output_bytes: usize,
}

impl AdmittedToolBinding {
    pub fn run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
    }
    pub fn profile(&self) -> &str {
        &self.profile
    }
    pub fn tool_id(&self) -> &str {
        &self.tool_id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn schema_sha256(&self) -> &str {
        &self.schema_sha256
    }
    pub fn descriptor_sha256(&self) -> &str {
        &self.descriptor_sha256
    }
    pub fn effect(&self) -> &str {
        &self.effect
    }
    pub fn max_input_bytes(&self) -> usize {
        self.max_input_bytes
    }
    pub fn max_output_bytes(&self) -> usize {
        self.max_output_bytes
    }
}

/// One execution attempt. `usage` stays `None` when it is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionReceipt {
    pub tool_id: String,
    pub executed: bool,
    pub ok: bool,
    pub usage: Option<ToolUsage>,
}

type Receipts = Arc<Mutex<Vec<ToolExecutionReceipt>>>;

/// An admitted set: the bindings, the tools to publish, and the builtins kept.
#[derive(Debug, Clone)]
pub struct AdmittedProfileTools {
    bindings: Vec<AdmittedToolBinding>,
    builtin_tools: Vec<String>,
    tools: Vec<ExtensionTool>,
    receipts: Receipts,
}

impl AdmittedProfileTools {
    pub fn bindings(&self) -> &[AdmittedToolBinding] {
        &self.bindings
    }

    pub fn receipts(&self) -> Vec<ToolExecutionReceipt> {
        self.receipts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The registry a session publishes from and executes through. A tool name
    /// that collides with a builtin, removed or not, or with another admitted
    /// tool is refused here.
    pub fn registry(&self) -> Result<ToolRegistry, ProfileToolRefusal> {
        ToolRegistry::with_profile_tools(self.tools.iter().cloned(), &self.builtin_tools)
            .map_err(|error| ProfileToolRefusal::new(RefusalKind::NameCollision, error.to_string()))
    }
}

/// Admit what this run's profile and pack declare. `Ok(None)` means off.
pub fn admit_selected(
    workspace: &Path,
    profile: &str,
    run_id: Option<String>,
) -> Result<Option<AdmittedProfileTools>, ProfileToolRefusal> {
    admit_with(
        workspace,
        profile,
        run_id,
        PackSource::Environment,
        SandboxView::InForce,
    )
}

/// Whether a required OS sandbox holds for this admission.
#[derive(Debug, Clone, Copy)]
pub(super) enum SandboxView {
    /// The run's installed state.
    InForce,
    /// Before the CLI installs the sandbox: whether it is going to be applied.
    /// The session admits again against the installed state.
    WillApply(bool),
}

impl SandboxView {
    fn applied(self) -> bool {
        match self {
            Self::InForce => crate::tools::os_sandbox::is_applied(),
            Self::WillApply(requested) => {
                requested
                    && matches!(
                        crate::tools::os_sandbox::probe(),
                        crate::tools::os_sandbox::SandboxAvailability::Available
                    )
            }
        }
    }
}

pub(super) fn admit_with(
    workspace: &Path,
    profile: &str,
    run_id: Option<String>,
    pack: PackSource,
    sandbox: SandboxView,
) -> Result<Option<AdmittedProfileTools>, ProfileToolRefusal> {
    let Some(selected) = selection::selected(profile, pack)? else {
        return Ok(None);
    };
    let workspace = std::fs::canonicalize(workspace).map_err(|error| {
        ProfileToolRefusal::new(
            RefusalKind::RegistryUnavailable,
            format!("workspace cannot be resolved: {error}"),
        )
    })?;
    let admitted = admit(selected, &workspace, profile, run_id, sandbox)?;
    admitted.registry()?;
    Ok(Some(admitted))
}

fn admit(
    selected: SelectedTools,
    workspace: &Path,
    profile: &str,
    run_id: Option<String>,
    sandbox: SandboxView,
) -> Result<AdmittedProfileTools, ProfileToolRefusal> {
    let receipts: Receipts = Arc::new(Mutex::new(Vec::new()));
    let mut bindings = Vec::with_capacity(selected.tools.len());
    let mut tools = Vec::with_capacity(selected.tools.len());
    for (tool_id, declared) in &selected.tools {
        let loaded = descriptor::load(tool_id, workspace)?;
        let executor = admit_one(&loaded.descriptor, declared, sandbox)?;
        let binding = binding(&loaded, declared, profile, run_id.clone())?;
        let arguments = descriptor::closed_arguments(&loaded.descriptor)?;
        let pinned = PinnedReadExecutor {
            binding: binding.clone(),
            descriptor: loaded.descriptor.clone(),
            descriptor_path: loaded.path.clone(),
            workspace: workspace.to_path_buf(),
            inner: executor,
            receipts: receipts.clone(),
        };
        let tool = ExtensionTool::read_only(
            loaded.descriptor.name.clone(),
            loaded.descriptor.description.clone(),
            arguments,
            ExtensionNetworkAccess::None,
            Arc::new(pinned),
        )
        .map_err(|error| {
            ProfileToolRefusal::new(RefusalKind::InvalidDescriptor, error.to_string())
        })?;
        bindings.push(binding);
        tools.push(tool);
    }
    Ok(AdmittedProfileTools {
        bindings,
        builtin_tools: selected.builtin_tools.into_iter().collect(),
        tools,
        receipts,
    })
}

/// The effect, required features, and runtime. A requested effect or feature
/// that is not implemented is unavailable; none is reported as a success.
fn admit_one(
    descriptor: &Descriptor,
    declared: &DeclaredTool,
    sandbox: SandboxView,
) -> Result<Arc<dyn ProfileToolReadExecutor>, ProfileToolRefusal> {
    let tool = &descriptor.tool_id;
    let unavailable = |reason: String| ProfileToolRefusal::new(RefusalKind::Unavailable, reason);
    if declared.effect != descriptor.effect {
        return Err(ProfileToolRefusal::new(
            RefusalKind::ConflictingDeclaration,
            format!(
                "tool `{tool}` is declared with effect `{}` but registered with `{}`",
                declared.effect, descriptor.effect
            ),
        ));
    }
    if descriptor.effect != EFFECT_READ {
        return Err(unavailable(format!(
            "tool `{tool}` effect `{}` is not implemented yet",
            descriptor.effect
        )));
    }
    for feature in descriptor.requires.iter().chain(&declared.requires) {
        if feature != FEATURE_OS_SANDBOX {
            return Err(unavailable(format!(
                "tool `{tool}` requires `{feature}`, which is not implemented"
            )));
        }
        if !sandbox.applied() {
            return Err(unavailable(format!(
                "tool `{tool}` requires `{feature}`, which is not in force for this run"
            )));
        }
    }
    if descriptor.runtime.kind != RUNTIME_IN_PROCESS {
        return Err(unavailable(format!(
            "tool `{tool}` runtime kind `{}` is not implemented",
            descriptor.runtime.kind
        )));
    }
    READ_EXECUTORS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&descriptor.runtime.id)
        .cloned()
        .ok_or_else(|| {
            unavailable(format!(
                "tool `{tool}` runtime `{}` is not available in this process",
                descriptor.runtime.id
            ))
        })
}

/// The declaration can narrow a limit, never widen one.
fn binding(
    loaded: &LoadedDescriptor,
    declared: &DeclaredTool,
    profile: &str,
    run_id: Option<String>,
) -> Result<AdmittedToolBinding, ProfileToolRefusal> {
    let descriptor = &loaded.descriptor;
    let narrowed = |declared: Option<usize>, registered: usize, label: &str| match declared {
        Some(value) if value == 0 || value > registered => Err(ProfileToolRefusal::new(
            RefusalKind::ConflictingDeclaration,
            format!(
                "tool `{}` {label} must stay within 1..{registered}",
                descriptor.tool_id
            ),
        )),
        Some(value) => Ok(value),
        None => Ok(registered),
    };
    Ok(AdmittedToolBinding {
        run_id,
        profile: profile.to_string(),
        tool_id: descriptor.tool_id.clone(),
        name: descriptor.name.clone(),
        version: descriptor.version.clone(),
        schema_sha256: descriptor.schema_sha256.clone(),
        descriptor_sha256: loaded.sha256.clone(),
        effect: descriptor.effect.clone(),
        max_input_bytes: narrowed(
            declared.max_input_bytes,
            descriptor.limits.max_input_bytes,
            "max_input_bytes",
        )?,
        max_output_bytes: narrowed(
            declared.max_output_bytes,
            descriptor.limits.max_output_bytes,
            "max_output_bytes",
        )?,
    })
}

/// Checks the pins and limits around each call of the operator's executor.
struct PinnedReadExecutor {
    binding: AdmittedToolBinding,
    descriptor: Descriptor,
    descriptor_path: PathBuf,
    workspace: PathBuf,
    inner: Arc<dyn ProfileToolReadExecutor>,
    receipts: Receipts,
}

impl fmt::Debug for PinnedReadExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PinnedReadExecutor")
            .field("binding", &self.binding)
            .finish_non_exhaustive()
    }
}

impl PinnedReadExecutor {
    fn record(&self, executed: bool, ok: bool, usage: Option<ToolUsage>) {
        self.receipts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(ToolExecutionReceipt {
                tool_id: self.binding.tool_id.clone(),
                executed,
                ok,
                usage,
            });
    }

    fn check_before_execution(&self, arguments: &Value) -> anyhow::Result<()> {
        descriptor::verify_descriptor_pin(&self.descriptor_path, &self.binding.descriptor_sha256)?;
        descriptor::verify_dependencies(&self.descriptor, &self.workspace)?;
        let input_bytes = serde_json::to_vec(arguments)?.len();
        if input_bytes > self.binding.max_input_bytes {
            anyhow::bail!(
                "profile tool input of {input_bytes} bytes exceeds the {} byte limit",
                self.binding.max_input_bytes
            );
        }
        Ok(())
    }
}

impl ExtensionExecutor for PinnedReadExecutor {
    fn transport(&self) -> &'static str {
        "profile_tool"
    }

    fn execute(&self, workspace_root: &Path, arguments: &Value) -> anyhow::Result<String> {
        if let Err(error) = self.check_before_execution(arguments) {
            self.record(false, false, None);
            return Err(error);
        }
        let output = match self.inner.read(workspace_root, arguments) {
            Ok(output) => output,
            Err(error) => {
                self.record(true, false, None);
                return Err(error);
            }
        };
        if output.text.len() > self.binding.max_output_bytes {
            self.record(true, false, output.usage);
            anyhow::bail!(
                "profile tool output of {} bytes exceeds the {} byte limit",
                output.text.len(),
                self.binding.max_output_bytes
            );
        }
        self.record(true, true, output.usage);
        Ok(output.text)
    }
}
