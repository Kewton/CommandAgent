//! Pre-provider admission of the selected tool set, and the immutable bindings
//! later stages receive.
//!
//! G1 runs only `read` tools with the existing closed argument types, through an
//! in-process executor a Rust caller registered. No configuration file can
//! register one, so a descriptor naming a runtime nobody registered is refused
//! as unavailable. Nothing here lifts acceptance or assurance: a receipt records
//! that a tool ran, never that an artifact or stage passed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::commit::CommitReceipt;
use super::descriptor::{
    self, Descriptor, DirectoryIdentity, EFFECT_READ, LoadedDescriptor, RUNTIME_IN_PROCESS,
    RegistryBinding,
};
use super::effects::{self, ProposalParts, ProposalTool, SessionAccess};
use super::schema::binding::{self as schema_binding, ToolPins};
use super::schema::invocation::{self, ReceiveState, SchemaGuard};
use super::selection::{self, DeclaredTool, PackSource, SelectedTools};
use super::{ProfileToolRefusal, RefusalKind};
use crate::planner::structured_stages::contract::{
    self as stage_contract, HostToolFacts, StageContractDraft, StageLimits, StageRunFacts,
};
use crate::planner::structured_stages::ledger::{LedgerEntry, process_ledgers};
use crate::planner::structured_stages::{
    DeclaredStagesUnread, StageIdentity, StageRun, contract_refused,
};
use crate::tools::extension::{ExtensionExecutor, ExtensionNetworkAccess, ExtensionTool};
use crate::tools::registry::ToolRegistry;

/// The OS sandbox wraps the commands a run spawns. An in-process executor spawns
/// none and runs with the host's authority, so no sandbox state satisfies this
/// feature in G1. Hash pins and a cleared environment are not isolation either.
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
    input_revision: Option<String>,
    adopted_schema_sha256: Option<String>,
    runtime_sha256: Option<String>,
}

impl AdmittedToolBinding {
    /// The digest of a G3 process declaration. `None` for any other tool.
    pub fn runtime_sha256(&self) -> Option<&str> {
        self.runtime_sha256.as_deref()
    }
    /// The host-adopted digest of the declared inputs. `None` without a G2 schema.
    pub fn input_revision(&self) -> Option<&str> {
        self.input_revision.as_deref()
    }
    /// The digest of the adopted, compiled schema. `None` without a G2 schema.
    pub fn adopted_schema_sha256(&self) -> Option<&str> {
        self.adopted_schema_sha256.as_deref()
    }
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
    schema_tools: Arc<ReceiveState>,
    proposals: Vec<Arc<ProposalTool>>,
    commits: Arc<Mutex<Vec<CommitReceipt>>>,
    /// `None` without a stage declaration, and then nothing below touches a ledger.
    stages: Option<Arc<StageRun>>,
    /// The function names of the tools joined to `stages`, which the history keeps under the
    /// stage budget while a session of this set runs (Issue #1004). Empty without a stage.
    stage_names: Arc<BTreeSet<String>>,
}

#[cfg(test)]
impl AdmittedProfileTools {
    pub(super) fn without_stages_for_test() -> Self {
        Self {
            bindings: Vec::new(),
            builtin_tools: Vec::new(),
            tools: Vec::new(),
            receipts: Arc::default(),
            schema_tools: Arc::new(ReceiveState::default()),
            proposals: Vec::new(),
            commits: Arc::default(),
            stages: None,
            stage_names: Arc::default(),
        }
    }

    /// One stage over a tool no process backs: enough for what is judged before any call.
    pub(super) fn with_stage_set_for_test(workspace: &Path) -> Self {
        use crate::planner::structured_stages::contract::{
            NarrowedStageLimits, StageDeclaration, StageSetDeclaration, TargetDeclaration,
        };
        let draft = StageContractDraft::new(
            &StageSetDeclaration {
                stages: vec![StageDeclaration {
                    id: "only".to_string(),
                    targets: vec![TargetDeclaration {
                        id: "main".to_string(),
                        tool_id: "a-tool".to_string(),
                    }],
                    depends_on: Vec::new(),
                    required_checks: Vec::new(),
                }],
                limits: NarrowedStageLimits::default(),
            },
            &[HostToolFacts {
                tool_id: "a-tool".to_string(),
                output_id: "artifact".to_string(),
                output_path: "out/a.json".to_string(),
                schema_inputs: Vec::new(),
                has_revision_argument: false,
                has_compiler: false,
                required_checks: vec!["shape".to_string()],
                limits: StageLimits {
                    attempts_per_target: 2,
                    attempts_per_run: 2,
                    deadline_seconds: 60,
                },
            }],
        )
        .unwrap();
        Self {
            stages: Some(Arc::new(StageRun::new(
                draft,
                BTreeMap::new(),
                workspace.to_path_buf(),
                "a-profile".to_string(),
                Arc::default(),
            ))),
            ..Self::without_stages_for_test()
        }
    }
}

impl AdmittedProfileTools {
    pub(crate) fn stage_run(&self) -> Option<Arc<StageRun>> {
        self.stages.clone()
    }

    /// The same question as a session's, for an entry about to spend a provider call. Off for a
    /// set with no stage.
    pub(crate) fn preflight_stage_run(
        &self,
        config: &crate::config::Config,
    ) -> Result<(), ProfileToolRefusal> {
        match &self.stages {
            Some(stages) => stages
                .judge_run(StageRunFacts::of_session(config), &config.state_dir)
                .map(|_| ()),
            None => Ok(()),
        }
    }

    /// What a declared stage set needs from the session it runs in. Every session asks again:
    /// the run's OS boundary and whether this is a workflow child are read now, not kept.
    pub(crate) fn bind_stage_session(
        &self,
        config: &crate::config::Config,
        entry: LedgerEntry,
    ) -> Result<(), ProfileToolRefusal> {
        match &self.stages {
            Some(stages) => {
                stages.bind_session(StageRunFacts::of_session(config), &config.state_dir, entry)
            }
            None => Ok(()),
        }
    }

    /// Off for every set without a schema tool or a proposal tool. Every entry reaches
    /// this before its first provider call.
    pub(crate) fn preflight_schema_tools(
        &self,
        config: &crate::config::Config,
        wire: crate::providers::profile_tool_schema::ProfileToolSchemaWire,
    ) -> Result<(), ProfileToolRefusal> {
        self.schema_tools.preflight(config, wire)?;
        self.proposals
            .iter()
            .try_for_each(|tool| tool.preflight(config))
    }

    /// What only a session knows: an inspect step, or a recovery's scoped reads.
    pub(crate) fn preflight_session(
        &self,
        access: &dyn Fn(&str) -> SessionAccess,
    ) -> Result<(), ProfileToolRefusal> {
        self.proposals
            .iter()
            .try_for_each(|tool| tool.preflight_session(access(tool.name())))
    }

    /// Every commit this set made, each decided by the commit leaf.
    pub fn commit_receipts(&self) -> Vec<CommitReceipt> {
        self.commits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Installs this set on the session's thread. `None` for a set without a schema
    /// tool, which leaves every parser on its existing path. A stage tool is a proposal
    /// tool, and admission refuses one without a schema, so a set with stages always gets a
    /// scope, and its stage tools' names go on the thread with it.
    pub(crate) fn enter_session(&self) -> Option<invocation::SessionScope> {
        (!self.schema_tools.is_empty()).then(|| {
            invocation::enter_session(self.schema_tools.clone())
                .with_stage_names(self.stage_names.clone())
        })
    }

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
        ToolRegistry::with_profile_tools_and_proposals(
            self.tools.iter().cloned(),
            self.proposals.iter().cloned(),
            &self.builtin_tools,
        )
        .map_err(|error| {
            let kind = if self.proposals.is_empty() {
                RefusalKind::NameCollision
            } else {
                RefusalKind::NameCollision.with_effect_origin()
            };
            ProfileToolRefusal::new(kind, error.to_string())
        })
    }
}

/// Admit what this run's profile and pack declare. `Ok(None)` means off.
pub fn admit_selected(
    workspace: &Path,
    profile: &str,
    run_id: Option<String>,
) -> Result<Option<AdmittedProfileTools>, ProfileToolRefusal> {
    admit_with(workspace, profile, run_id, PackSource::Environment)
}

pub(super) fn admit_with(
    workspace: &Path,
    profile: &str,
    run_id: Option<String>,
    pack: PackSource,
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
    let admitted = admit(selected, &workspace, profile, run_id)?;
    admitted.registry()?;
    Ok(Some(admitted))
}

fn admit(
    selected: SelectedTools,
    workspace: &Path,
    profile: &str,
    run_id: Option<String>,
) -> Result<AdmittedProfileTools, ProfileToolRefusal> {
    let receipts: Receipts = Arc::new(Mutex::new(Vec::new()));
    let mut bindings = Vec::with_capacity(selected.tools.len());
    let mut tools = Vec::with_capacity(selected.tools.len());
    let mut schema_tools = ReceiveState::default();
    let mut proposals: Vec<Arc<ProposalTool>> = Vec::new();
    let commits: Arc<Mutex<Vec<CommitReceipt>>> = Arc::default();
    let draft = stage_draft(&selected, workspace)?;
    for (tool_id, declared) in &selected.tools {
        let loaded = descriptor::load(tool_id, workspace).map_err(|error| {
            if declared.effect == effects::EFFECT_PROPOSE_ARTIFACTS {
                effects::with_effect_origin(error)
            } else {
                error
            }
        })?;
        if loaded.descriptor.effect == effects::EFFECT_PROPOSE_ARTIFACTS
            && loaded.descriptor.runtime.kind == effects::RUNTIME_PINNED_PROCESS
        {
            let upstream = draft
                .as_ref()
                .map(|draft| draft.upstream_inputs(tool_id))
                .unwrap_or_default();
            let tool = admit_proposal(
                &loaded,
                declared,
                (profile, run_id.clone()),
                (workspace, &upstream),
                &mut schema_tools,
                (receipts.clone(), commits.clone()),
                &mut bindings,
            )
            .map_err(effects::with_effect_origin)?;
            proposals.push(Arc::new(tool));
            continue;
        }
        let executor = admit_one(&loaded.descriptor, declared)?;
        let mut binding = binding(&loaded, declared, profile, run_id.clone())?;
        let arguments = descriptor::closed_arguments(&loaded.descriptor)?;
        let schema_guard = admit_schema(
            &loaded,
            &mut binding,
            (workspace, &BTreeSet::new()),
            profile,
            &mut schema_tools,
        )?;
        let pinned = PinnedReadExecutor {
            binding: binding.clone(),
            descriptor: loaded.descriptor.clone(),
            registry: loaded.registry.clone(),
            dependency_directories: loaded.dependency_directories.clone(),
            workspace: workspace.to_path_buf(),
            inner: executor,
            receipts: receipts.clone(),
            schema_guard: schema_guard.clone(),
        };
        let tool = ExtensionTool::read_only(
            loaded.descriptor.name.clone(),
            loaded.descriptor.description.clone(),
            arguments,
            ExtensionNetworkAccess::None,
            Arc::new(pinned),
        )
        .map(|tool| tool.with_schema_guard(schema_guard))
        .map_err(|error| {
            ProfileToolRefusal::new(RefusalKind::InvalidDescriptor, error.to_string())
        })?;
        bindings.push(binding);
        tools.push(tool);
    }
    let (stages, stage_names) = match draft {
        Some(draft) => {
            let (run, names) = stage_run(draft, &proposals, workspace, profile)?;
            (Some(run), names)
        }
        None => (None, BTreeSet::new()),
    };
    Ok(AdmittedProfileTools {
        bindings,
        builtin_tools: selected.builtin_tools.into_iter().collect(),
        tools,
        receipts,
        schema_tools: Arc::new(schema_tools),
        proposals,
        commits,
        stages,
        stage_names: Arc::new(stage_names),
    })
}

fn stage_refused(tool: &str, reason: &str) -> ProfileToolRefusal {
    ProfileToolRefusal::new(
        RefusalKind::StageContractRefused,
        format!("tool `{tool}` {reason}"),
    )
}

/// The one place an operator's descriptor becomes what a stage set is judged on. A commit
/// names its output as declared and the state machine compares normalized paths, so a spelling
/// that normalizes to something else would make every commit of the tool fail to bind.
fn host_tool_facts(descriptor: &Descriptor) -> Result<HostToolFacts, ProfileToolRefusal> {
    let tool = descriptor.tool_id.as_str();
    let floor = descriptor
        .stage
        .as_ref()
        .ok_or_else(|| stage_refused(tool, "has no `stage` floor from its operator"))?;
    let schema = descriptor
        .schema
        .as_ref()
        .ok_or_else(|| stage_refused(tool, "has no schema"))?;
    let process = descriptor
        .process
        .as_ref()
        .filter(|process| process.outputs.len() == 1)
        .ok_or_else(|| stage_refused(tool, "has no single output"))?;
    let shape = schema_binding::declared_shape(schema, tool)?;
    let output = process.single_output();
    let one_spelling = |path: &str| {
        stage_contract::normalize_path(path).is_ok_and(|normalized| normalized == path)
    };
    if !one_spelling(&output.path) || !shape.inputs.iter().all(|path| one_spelling(path)) {
        return Err(stage_refused(
            tool,
            "declares a path that is not in its one normal spelling",
        ));
    }
    Ok(HostToolFacts {
        tool_id: descriptor.tool_id.clone(),
        output_id: output.id.clone(),
        output_path: output.path.clone(),
        schema_inputs: shape.inputs,
        has_revision_argument: shape.has_revision_argument,
        has_compiler: shape.has_compiler,
        required_checks: floor.required_checks.clone(),
        limits: StageLimits {
            attempts_per_target: floor.attempts_per_target,
            attempts_per_run: floor.attempts_per_run,
            deadline_seconds: floor.deadline_seconds,
        },
    })
}

/// A declared stage set, judged before any input is adopted: an input that a stage's output
/// feeds is not there yet on a first run, and G2 has to know not to wait for it. Off reads nothing.
fn stage_draft(
    selected: &SelectedTools,
    workspace: &Path,
) -> Result<Option<StageContractDraft>, ProfileToolRefusal> {
    Ok(stage_draft_and_descriptors(selected, workspace)?.map(|(draft, _)| draft))
}

fn stage_draft_and_descriptors(
    selected: &SelectedTools,
    workspace: &Path,
) -> Result<Option<(StageContractDraft, Vec<LoadedDescriptor>)>, ProfileToolRefusal> {
    let Some(declaration) = &selected.stages else {
        return Ok(None);
    };
    let staged = declaration
        .stages
        .iter()
        .flat_map(|stage| stage.targets.iter())
        .map(|target| target.tool_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut facts = Vec::with_capacity(staged.len());
    let mut descriptors = Vec::with_capacity(staged.len());
    for (tool_id, declared) in &selected.tools {
        if staged.contains(tool_id.as_str()) && declared.effect == effects::EFFECT_PROPOSE_ARTIFACTS
        {
            let loaded =
                descriptor::load(tool_id, workspace).map_err(effects::with_effect_origin)?;
            facts.push(host_tool_facts(&loaded.descriptor)?);
            descriptors.push(loaded);
        }
    }
    StageContractDraft::new(declaration, &facts)
        .map(|draft| Some((draft, descriptors)))
        .map_err(contract_refused)
}

/// What the stages declared now are, for a record's seals to be held to. It only reads: no input,
/// no adoption, no probe, no stage run, no ledger, and `Ok(None)` is a declaration with no stage.
/// A descriptor's digest covers what it says and not what it points at, so the bytes of a process
/// are held to their pins here too.
pub(crate) fn declared_stage_identity(
    workspace: &Path,
    profile: &str,
) -> Result<Option<StageIdentity>, DeclaredStagesUnread> {
    let selected = selection::selected(profile, PackSource::Environment)
        .map_err(|_| DeclaredStagesUnread::SelectedPack)?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let unread = |_| DeclaredStagesUnread::Stages;
    let Some((draft, loaded)) =
        stage_draft_and_descriptors(&selected, workspace).map_err(unread)?
    else {
        return Ok(None);
    };
    let mut descriptors = BTreeMap::new();
    for tool in loaded {
        let process = tool
            .descriptor
            .process
            .as_ref()
            .ok_or(DeclaredStagesUnread::Stages)?;
        descriptor::verify_process_pins(process, &tool.descriptor.tool_id, workspace)
            .map_err(unread)?;
        descriptors.insert(tool.descriptor.tool_id, tool.sha256);
    }
    Ok(Some(StageIdentity {
        contract_sha256: draft.contract_sha256(),
        descriptors,
    }))
}

/// Joins the admitted tools to their stage set. Only now does the process's ledger table come
/// into it, and only because a declaration is there. With the run go the function names the
/// model calls those tools by, which is what the history sees.
fn stage_run(
    draft: StageContractDraft,
    proposals: &[Arc<ProposalTool>],
    workspace: &Path,
    profile: &str,
) -> Result<(Arc<StageRun>, BTreeSet<String>), ProfileToolRefusal> {
    let mut members = BTreeMap::new();
    let mut staged = Vec::new();
    for tool_id in draft.tool_ids() {
        let tool = proposals
            .iter()
            .find(|tool| tool.tool_id() == tool_id)
            .ok_or_else(|| stage_refused(tool_id, "is not an admitted proposal tool"))?;
        if let Some(target) = draft.target_for_tool(tool_id) {
            tool.matches_target(target)?;
        }
        members.insert(tool_id.to_string(), tool.stage_member()?);
        staged.push(tool.clone());
    }
    let run = Arc::new(StageRun::new(
        draft,
        members,
        workspace.to_path_buf(),
        profile.to_string(),
        process_ledgers(),
    ));
    let names = staged.iter().map(|tool| tool.name().to_string()).collect();
    for tool in staged {
        tool.attach_stage(run.clone());
    }
    Ok((run, names))
}

/// G3: the checks that need no run context, then the same schema adoption as any tool. A
/// proposal tool gets its own registry entry, so nothing here passes through the Read path.
fn admit_proposal(
    loaded: &LoadedDescriptor,
    declared: &DeclaredTool,
    (profile, run_id): (&str, Option<String>),
    (workspace, upstream): (&Path, &BTreeSet<String>),
    schema_tools: &mut ReceiveState,
    (receipts, commits): (Receipts, Arc<Mutex<Vec<CommitReceipt>>>),
    bindings: &mut Vec<AdmittedToolBinding>,
) -> Result<ProposalTool, ProfileToolRefusal> {
    let (declaration, pins) = effects::admit_process(&loaded.descriptor, declared, workspace)?;
    let mut admitted = binding(loaded, declared, profile, run_id)?;
    let guard = admit_schema(
        loaded,
        &mut admitted,
        (workspace, upstream),
        profile,
        schema_tools,
    )?
    .ok_or_else(|| {
        ProfileToolRefusal::new(
            RefusalKind::InvalidDescriptor,
            format!("tool `{}` needs a schema", admitted.tool_id),
        )
    })?;
    let tool = ProposalTool::new(ProposalParts {
        name: admitted.name.clone(),
        description: loaded.descriptor.description.clone(),
        tool_id: admitted.tool_id.clone(),
        tool_version: admitted.version.clone(),
        descriptor_sha256: admitted.descriptor_sha256.clone(),
        declaration: declaration.clone(),
        registry: loaded.registry.clone(),
        pins,
        workspace: workspace.to_path_buf(),
        guard,
        max_input_bytes: admitted.max_input_bytes,
        max_output_bytes: admitted.max_output_bytes,
        receipts,
        commits,
    })?;
    admitted.runtime_sha256 = Some(tool.runtime_sha256().to_string());
    bindings.push(admitted);
    Ok(tool)
}

/// Adopts a G2 tool's inputs and schema. Runs after `admit_one`, so a tool whose
/// effect, feature, or runtime is unavailable stays refused whatever its schema says.
fn admit_schema(
    loaded: &LoadedDescriptor,
    binding: &mut AdmittedToolBinding,
    (workspace, upstream): (&Path, &BTreeSet<String>),
    profile: &str,
    schema_tools: &mut ReceiveState,
) -> Result<Option<Arc<SchemaGuard>>, ProfileToolRefusal> {
    let Some(schema) = &loaded.descriptor.schema else {
        return Ok(None);
    };
    let pins = ToolPins {
        tool_id: &binding.tool_id,
        version: &binding.version,
        descriptor_sha256: &binding.descriptor_sha256,
        max_input_bytes: binding.max_input_bytes,
    };
    let adoption =
        schema_binding::adopt_with_upstream(schema, &pins, workspace, profile, upstream)?;
    binding.input_revision = Some(adoption.adopted().revision.clone());
    binding.adopted_schema_sha256 = Some(adoption.schema().sha256().to_string());
    Ok(Some(schema_tools.add(
        &binding.name,
        &loaded.descriptor.description,
        adoption,
        loaded.registry.clone(),
        &binding.descriptor_sha256,
    )))
}

/// The effect, required features, and runtime. A requested effect or feature
/// that is not implemented is unavailable; none is reported as a success.
fn admit_one(
    descriptor: &Descriptor,
    declared: &DeclaredTool,
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
    if let Some(feature) = descriptor.requires.iter().chain(&declared.requires).next() {
        return Err(unavailable(if feature == FEATURE_OS_SANDBOX {
            format!(
                "tool `{tool}` requires `{feature}`, which cannot isolate an in_process executor: the OS sandbox wraps spawned commands only"
            )
        } else {
            format!("tool `{tool}` requires `{feature}`, which is not implemented")
        }));
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
        input_revision: None,
        adopted_schema_sha256: None,
        runtime_sha256: None,
    })
}

/// Checks the pins and limits around each call of the operator's executor.
struct PinnedReadExecutor {
    binding: AdmittedToolBinding,
    descriptor: Descriptor,
    registry: RegistryBinding,
    dependency_directories: Vec<DirectoryIdentity>,
    workspace: PathBuf,
    inner: Arc<dyn ProfileToolReadExecutor>,
    receipts: Receipts,
    schema_guard: Option<Arc<SchemaGuard>>,
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
        descriptor::verify_descriptor_pin(&self.registry, &self.binding.descriptor_sha256)?;
        if descriptor::verify_dependencies(&self.descriptor, &self.workspace)?
            != self.dependency_directories
        {
            return Err(ProfileToolRefusal::new(
                RefusalKind::InvalidPin,
                format!(
                    "tool `{}` dependency directory changed since admission",
                    self.binding.tool_id
                ),
            )
            .into());
        }
        if let Some(guard) = &self.schema_guard {
            guard.check_before_execution(arguments)?;
        }
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

#[cfg(test)]
mod stage_facts_tests {
    use serde_json::{Value, json};

    use super::super::RefusalKind;
    use super::host_tool_facts;
    use crate::tools::profile_tools::descriptor::Descriptor;

    fn descriptor(output: &str, input: &str, stage: Option<Value>) -> Descriptor {
        let mut value = json!({
            "protocol": "profile-tool.descriptor/v1",
            "tool_id": "t",
            "name": "T",
            "version": "1",
            "description": "d",
            "effect": "propose-artifacts",
            "parameters": [],
            "schema": {
                "protocol": "profile-tool.schema/v1",
                "root": {"type": "object", "additionalProperties": false, "properties": {}},
                "inputs": [input],
            },
            "schema_sha256": "a".repeat(64),
            "runtime": {"kind": "pinned_process", "id": "x"},
            "limits": {"max_input_bytes": 1, "max_output_bytes": 1},
            "process": {
                "protocol": "profile-tool.process/v1",
                "executable": {"path": "/x", "sha256": "b".repeat(64)},
                "argv": [],
                "cwd": "/",
                "limits": {"timeout_ms": 1, "max_result_bytes": 1},
                "outputs": [{"id": "artifact", "path": output}],
            },
        });
        if let Some(stage) = stage {
            value["stage"] = stage;
        }
        serde_json::from_value(value).unwrap()
    }

    fn floor() -> Option<Value> {
        Some(json!({
            "required_checks": ["shape"],
            "attempts_per_target": 3,
            "attempts_per_run": 9,
            "deadline_seconds": 60,
        }))
    }

    #[test]
    fn the_facts_are_the_operators_values_as_declared() {
        let facts = host_tool_facts(&descriptor("out/a.json", "fixed/b.json", floor())).unwrap();
        assert_eq!(facts.output_path, "out/a.json");
        assert_eq!(facts.output_id, "artifact");
        assert_eq!(facts.schema_inputs, ["fixed/b.json"]);
        assert_eq!(facts.required_checks, ["shape"]);
        assert_eq!(facts.limits.attempts_per_target, 3);
        assert_eq!(facts.limits.attempts_per_run, 9);
        assert_eq!(facts.limits.deadline_seconds, 60);
        assert!(!facts.has_compiler);
        assert!(!facts.has_revision_argument);
    }

    /// A commit names the path as declared and a receipt compares it normalized.
    #[test]
    fn a_path_in_any_other_spelling_is_refused_before_a_commit_could_fail_on_it() {
        for (output, input) in [
            ("./out/a.json", "fixed/b.json"),
            ("out//a.json", "fixed/b.json"),
            ("out/a.json", "./fixed/b.json"),
            ("out/a.json", "fixed//b.json"),
            ("out/./a.json", "fixed/b.json"),
        ] {
            let refusal = host_tool_facts(&descriptor(output, input, floor())).unwrap_err();
            assert_eq!(refusal.kind, RefusalKind::StageContractRefused, "{output}");
        }
    }

    #[test]
    fn a_tool_with_no_floor_from_its_operator_is_refused() {
        let refusal = host_tool_facts(&descriptor("out/a.json", "fixed/b.json", None)).unwrap_err();
        assert_eq!(refusal.kind, RefusalKind::StageContractRefused);
    }
}
