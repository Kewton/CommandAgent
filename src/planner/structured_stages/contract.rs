//! The admission of a declared stage set (Issue #975, G4).
//!
//! A profile or pack declares opaque stages, targets, extra dependencies, and extra required
//! checks. Everything that decides an obligation's floor comes from the host: the output path,
//! the schema inputs, the operator's required check IDs, and the operator's limits. A
//! declaration can add a dependency or a check and narrow a limit. It can remove nothing.
//!
//! Edges are derived from `schema.inputs` and the outputs' paths, so forgetting to declare a
//! dependency cannot hide one (S13). Nothing here reads a file or starts a process.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::tools::profile_tools::protocol::MAX_CHECKS;

pub(crate) const MAX_STAGES: usize = 32;
pub(crate) const MAX_TARGETS: usize = 64;
pub(crate) const MAX_EDGES: usize = 128;
pub(crate) const MAX_REQUIRED_CHECKS: usize = MAX_CHECKS;
pub(crate) const MAX_INPUTS_PER_TOOL: usize = 64;
pub(crate) const MAX_ID_BYTES: usize = 128;
pub(crate) const MAX_PATH_BYTES: usize = 1024;
pub(crate) const HOST_MAX_ATTEMPTS_PER_TARGET: u32 = 8;
pub(crate) const HOST_MAX_ATTEMPTS_PER_RUN: u32 = 64;
pub(crate) const HOST_MAX_DEADLINE_SECONDS: u64 = 3600;

/// One obligation's name. Both parts are opaque to the host.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TargetKey {
    pub(crate) stage: String,
    pub(crate) target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TargetDeclaration {
    pub(crate) id: String,
    pub(crate) tool_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StageDeclaration {
    pub(crate) id: String,
    pub(crate) targets: Vec<TargetDeclaration>,
    /// Added to the edges the host derives. It cannot take one away.
    #[serde(default)]
    pub(crate) depends_on: Vec<String>,
    /// Added to the operator's IDs. It cannot take one away.
    #[serde(default)]
    pub(crate) required_checks: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NarrowedStageLimits {
    #[serde(default)]
    pub(crate) attempts_per_target: Option<u32>,
    #[serde(default)]
    pub(crate) attempts_per_run: Option<u32>,
    #[serde(default)]
    pub(crate) deadline_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StageSetDeclaration {
    pub(crate) stages: Vec<StageDeclaration>,
    #[serde(default)]
    pub(crate) limits: NarrowedStageLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct StageLimits {
    pub(crate) attempts_per_target: u32,
    pub(crate) attempts_per_run: u32,
    pub(crate) deadline_seconds: u64,
}

/// What the operator's descriptor and its G2 schema say about one tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostToolFacts {
    pub(crate) tool_id: String,
    pub(crate) output_id: String,
    pub(crate) output_path: String,
    pub(crate) schema_inputs: Vec<String>,
    pub(crate) has_revision_argument: bool,
    pub(crate) has_compiler: bool,
    pub(crate) required_checks: Vec<String>,
    pub(crate) limits: StageLimits,
}

/// The facts about a run that decide whether stages may be declared in it at all. Read anew for
/// each session: a workflow child reconciles the sandbox with its own workspace, so an earlier
/// answer does not carry over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StageRunFacts {
    /// False for `--sandbox off`, for `auto` with no backend, and when no policy is installed.
    os_sandbox_enforced: bool,
    workflow_child: bool,
    /// A treatment runs in a workspace of its own, where a ledger would start over (D10).
    recovers_by_itself: bool,
}

impl StageRunFacts {
    /// What this process holds now. This is the only way a production caller gets a value.
    pub(crate) fn of_session(config: &crate::config::Config) -> Self {
        Self {
            os_sandbox_enforced: crate::tools::os_sandbox::is_applied(),
            workflow_child: crate::planner::external_reproducer::is_workflow_node(config),
            recovers_by_itself: config.recovery_plan_auto_runs > 0,
        }
    }

    #[cfg(test)]
    pub(crate) const fn for_test(os_sandbox_enforced: bool, workflow_child: bool) -> Self {
        Self {
            os_sandbox_enforced,
            workflow_child,
            recovers_by_itself: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractRefusalKind {
    NoStages,
    TooManyStages,
    TooManyTargets,
    TooManyEdges,
    TooManyRequiredChecks,
    TooManyInputs,
    InvalidId,
    DuplicateStage,
    DuplicateTarget,
    DuplicateDependency,
    DuplicateCheck,
    DuplicateToolFacts,
    StageWithoutTargets,
    UnknownDependency,
    UnknownTool,
    ToolInTwoTargets,
    NoOperatorChecks,
    InvalidPath,
    OutputCollision,
    Cycle,
    SchemaNarrowedByUpstream,
    InvalidLimit,
    WidenedLimit,
    OsSandboxNotEnforced,
    WorkflowChild,
    RecoversByItself,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContractRefusal {
    pub(crate) kind: ContractRefusalKind,
    /// The opaque ID or path the refusal is about. Empty when it is about the whole set.
    pub(crate) subject: String,
}

fn refused(kind: ContractRefusalKind, subject: impl Into<String>) -> ContractRefusal {
    ContractRefusal {
        kind,
        subject: subject.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AdmittedInput {
    pub(crate) path: String,
    /// The target whose output this input is. `None` is a fixed input.
    pub(crate) upstream: Option<TargetKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AdmittedTarget {
    pub(crate) key: TargetKey,
    pub(crate) tool_id: String,
    pub(crate) output_id: String,
    pub(crate) output_path: String,
    pub(crate) required_checks: BTreeSet<String>,
    pub(crate) inputs: Vec<AdmittedInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AdmittedStage {
    pub(crate) id: String,
    /// Direct dependencies: derived and declared together.
    pub(crate) depends_on: BTreeSet<String>,
    pub(crate) targets: Vec<AdmittedTarget>,
}

/// An admitted stage set, upstream before downstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct StageContract {
    stages: Vec<AdmittedStage>,
    limits: StageLimits,
}

impl StageContract {
    pub(crate) fn stages(&self) -> &[AdmittedStage] {
        &self.stages
    }

    pub(crate) fn limits(&self) -> StageLimits {
        self.limits
    }

    pub(crate) fn targets(&self) -> impl Iterator<Item = &AdmittedTarget> {
        self.stages.iter().flat_map(|stage| stage.targets.iter())
    }

    pub(crate) fn target(&self, key: &TargetKey) -> Option<&AdmittedTarget> {
        self.targets().find(|target| &target.key == key)
    }

    pub(crate) fn target_for_tool(&self, tool_id: &str) -> Option<&AdmittedTarget> {
        self.targets().find(|target| target.tool_id == tool_id)
    }

    pub(crate) fn stage(&self, id: &str) -> Option<&AdmittedStage> {
        self.stages.iter().find(|stage| stage.id == id)
    }

    /// Ties a saved ledger to the graph, checks, and limits it was written under.
    pub(crate) fn sha256(&self) -> String {
        let canonical = serde_json::to_vec(self).unwrap_or_default();
        format!("{:x}", Sha256::digest(&canonical))
    }
}

/// With no OS boundary, every command the run executes can reach the ledger: Bash, and also a
/// plan's verify and a dependency setup, so taking Bash away would not close it (D6). A workflow
/// child's obligations have no owner in this version (D12), and neither have an automatic
/// recovery's: its treatment works on a copy of the workspace, under a ledger of its own.
fn refuse_for_run(facts: StageRunFacts) -> Result<(), ContractRefusal> {
    if facts.workflow_child {
        return Err(refused(ContractRefusalKind::WorkflowChild, ""));
    }
    if !facts.os_sandbox_enforced {
        return Err(refused(ContractRefusalKind::OsSandboxNotEnforced, ""));
    }
    if facts.recovers_by_itself {
        return Err(refused(ContractRefusalKind::RecoversByItself, ""));
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn require_id(id: &str) -> Result<(), ContractRefusal> {
    if valid_id(id) {
        Ok(())
    } else {
        Err(refused(ContractRefusalKind::InvalidId, ""))
    }
}

/// A workspace-relative path in one spelling: no absolute start, no `..`, and no `.` or empty
/// segments left.
pub(crate) fn normalize_path(path: &str) -> Result<String, ContractRefusal> {
    let invalid = || refused(ContractRefusalKind::InvalidPath, "");
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
    {
        return Err(invalid());
    }
    let mut segments = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => return Err(invalid()),
            other => segments.push(other),
        }
    }
    if segments.is_empty() {
        return Err(invalid());
    }
    Ok(segments.join("/"))
}

/// The form paths are compared in. A case-insensitive filesystem makes two spellings one file,
/// so a false match costs a refusal or an extra edge while a missed one hides a dependency.
fn comparison_form(path: &str) -> String {
    path.to_lowercase()
}

/// True when one path is the other or lies below it.
fn paths_overlap(left: &str, right: &str) -> bool {
    let (left, right) = (comparison_form(left), comparison_form(right));
    let below = |inner: &str, outer: &str| {
        inner.len() > outer.len()
            && inner.starts_with(outer)
            && inner.as_bytes()[outer.len()] == b'/'
    };
    left == right || below(&left, &right) || below(&right, &left)
}

fn effective_limits(
    declaration: NarrowedStageLimits,
    tools: &[&HostToolFacts],
) -> Result<StageLimits, ContractRefusal> {
    let mut limits = StageLimits {
        attempts_per_target: HOST_MAX_ATTEMPTS_PER_TARGET,
        attempts_per_run: HOST_MAX_ATTEMPTS_PER_RUN,
        deadline_seconds: HOST_MAX_DEADLINE_SECONDS,
    };
    for tool in tools {
        let own = tool.limits;
        if !(1..=HOST_MAX_ATTEMPTS_PER_TARGET).contains(&own.attempts_per_target)
            || !(1..=HOST_MAX_ATTEMPTS_PER_RUN).contains(&own.attempts_per_run)
            || !(1..=HOST_MAX_DEADLINE_SECONDS).contains(&own.deadline_seconds)
        {
            return Err(refused(
                ContractRefusalKind::InvalidLimit,
                tool.tool_id.as_str(),
            ));
        }
        limits.attempts_per_target = limits.attempts_per_target.min(own.attempts_per_target);
        limits.attempts_per_run = limits.attempts_per_run.min(own.attempts_per_run);
        limits.deadline_seconds = limits.deadline_seconds.min(own.deadline_seconds);
    }
    let narrow_u32 = |value: Option<u32>, floor: &mut u32| match value {
        None => Ok(()),
        Some(0) => Err(refused(ContractRefusalKind::InvalidLimit, "")),
        Some(narrowed) if narrowed > *floor => Err(refused(ContractRefusalKind::WidenedLimit, "")),
        Some(narrowed) => {
            *floor = narrowed;
            Ok(())
        }
    };
    narrow_u32(
        declaration.attempts_per_target,
        &mut limits.attempts_per_target,
    )?;
    narrow_u32(declaration.attempts_per_run, &mut limits.attempts_per_run)?;
    match declaration.deadline_seconds {
        None => {}
        Some(0) => return Err(refused(ContractRefusalKind::InvalidLimit, "")),
        Some(narrowed) if narrowed > limits.deadline_seconds => {
            return Err(refused(ContractRefusalKind::WidenedLimit, ""));
        }
        Some(narrowed) => limits.deadline_seconds = narrowed,
    }
    Ok(limits)
}

/// The IDs a target must report: exactly these, each `passed`. The operator's are the floor,
/// and an operator that requires nothing leaves nothing a seal could mean.
fn required_checks(
    stage: &StageDeclaration,
    tool: &HostToolFacts,
) -> Result<BTreeSet<String>, ContractRefusal> {
    if tool.required_checks.is_empty() {
        return Err(refused(
            ContractRefusalKind::NoOperatorChecks,
            tool.tool_id.as_str(),
        ));
    }
    let mut checks = BTreeSet::new();
    for list in [&tool.required_checks, &stage.required_checks] {
        let mut own = BTreeSet::new();
        for id in list {
            require_id(id)?;
            if !own.insert(id) {
                return Err(refused(ContractRefusalKind::DuplicateCheck, id.as_str()));
            }
            checks.insert(id.clone());
        }
    }
    if checks.len() > MAX_REQUIRED_CHECKS {
        return Err(refused(
            ContractRefusalKind::TooManyRequiredChecks,
            stage.id.as_str(),
        ));
    }
    Ok(checks)
}

struct Resolved<'a> {
    stage: &'a StageDeclaration,
    target: &'a TargetDeclaration,
    tool: &'a HostToolFacts,
    output_path: String,
    inputs: Vec<String>,
}

fn resolve_targets<'a>(
    declaration: &'a StageSetDeclaration,
    facts: &'a [HostToolFacts],
) -> Result<Vec<Resolved<'a>>, ContractRefusal> {
    let mut by_tool: BTreeMap<&str, &HostToolFacts> = BTreeMap::new();
    for tool in facts {
        if by_tool.insert(tool.tool_id.as_str(), tool).is_some() {
            return Err(refused(
                ContractRefusalKind::DuplicateToolFacts,
                tool.tool_id.as_str(),
            ));
        }
    }
    let mut resolved = Vec::new();
    let mut used_tools = BTreeSet::new();
    let mut stage_ids = BTreeSet::new();
    for stage in &declaration.stages {
        require_id(&stage.id)?;
        if !stage_ids.insert(stage.id.as_str()) {
            return Err(refused(
                ContractRefusalKind::DuplicateStage,
                stage.id.as_str(),
            ));
        }
        if stage.targets.is_empty() {
            return Err(refused(
                ContractRefusalKind::StageWithoutTargets,
                stage.id.as_str(),
            ));
        }
        let mut target_ids = BTreeSet::new();
        for target in &stage.targets {
            require_id(&target.id)?;
            if !target_ids.insert(target.id.as_str()) {
                return Err(refused(
                    ContractRefusalKind::DuplicateTarget,
                    target.id.as_str(),
                ));
            }
            let tool = by_tool
                .get(target.tool_id.as_str())
                .copied()
                .ok_or_else(|| {
                    refused(ContractRefusalKind::UnknownTool, target.tool_id.as_str())
                })?;
            if !used_tools.insert(target.tool_id.as_str()) {
                return Err(refused(
                    ContractRefusalKind::ToolInTwoTargets,
                    target.tool_id.as_str(),
                ));
            }
            if tool.schema_inputs.len() > MAX_INPUTS_PER_TOOL {
                return Err(refused(
                    ContractRefusalKind::TooManyInputs,
                    tool.tool_id.as_str(),
                ));
            }
            let inputs = tool
                .schema_inputs
                .iter()
                .map(|path| normalize_path(path))
                .collect::<Result<Vec<_>, _>>()?;
            resolved.push(Resolved {
                stage,
                target,
                tool,
                output_path: normalize_path(&tool.output_path)?,
                inputs,
            });
        }
    }
    Ok(resolved)
}

fn refuse_output_collisions(resolved: &[Resolved<'_>]) -> Result<(), ContractRefusal> {
    for (index, left) in resolved.iter().enumerate() {
        for right in &resolved[index + 1..] {
            if paths_overlap(&left.output_path, &right.output_path) {
                return Err(refused(
                    ContractRefusalKind::OutputCollision,
                    right.output_path.as_str(),
                ));
            }
        }
    }
    Ok(())
}

/// Upstream stage to downstream stage. A stage reading its own output is a cycle: its commit
/// would change the input its own check ran on.
fn collect_edges(
    declaration: &StageSetDeclaration,
    resolved: &[Resolved<'_>],
) -> Result<BTreeSet<(String, String)>, ContractRefusal> {
    let known = declaration
        .stages
        .iter()
        .map(|stage| stage.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut edges = BTreeSet::new();
    for reader in resolved {
        for input in &reader.inputs {
            for writer in resolved {
                if paths_overlap(input, &writer.output_path) {
                    if writer.stage.id == reader.stage.id {
                        return Err(refused(
                            ContractRefusalKind::Cycle,
                            reader.stage.id.as_str(),
                        ));
                    }
                    edges.insert((writer.stage.id.clone(), reader.stage.id.clone()));
                }
            }
        }
    }
    for stage in &declaration.stages {
        let mut declared = BTreeSet::new();
        for dependency in &stage.depends_on {
            if !known.contains(dependency.as_str()) {
                return Err(refused(
                    ContractRefusalKind::UnknownDependency,
                    dependency.as_str(),
                ));
            }
            if !declared.insert(dependency.as_str()) {
                return Err(refused(
                    ContractRefusalKind::DuplicateDependency,
                    dependency.as_str(),
                ));
            }
            if dependency == &stage.id {
                return Err(refused(ContractRefusalKind::Cycle, stage.id.as_str()));
            }
            edges.insert((dependency.clone(), stage.id.clone()));
        }
    }
    if edges.len() > MAX_EDGES {
        return Err(refused(ContractRefusalKind::TooManyEdges, ""));
    }
    Ok(edges)
}

/// Upstream first; among the ready ones, by ID. What never becomes ready is on a cycle.
fn stage_order(
    declaration: &StageSetDeclaration,
    edges: &BTreeSet<(String, String)>,
) -> Result<Vec<String>, ContractRefusal> {
    let mut waiting_on: BTreeMap<&str, usize> = declaration
        .stages
        .iter()
        .map(|stage| (stage.id.as_str(), 0))
        .collect();
    for (_, downstream) in edges {
        if let Some(count) = waiting_on.get_mut(downstream.as_str()) {
            *count += 1;
        }
    }
    let mut ready = waiting_on
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(waiting_on.len());
    while let Some(next) = ready.pop_first() {
        order.push(next.to_string());
        for (upstream, downstream) in edges {
            if upstream != next {
                continue;
            }
            if let Some(count) = waiting_on.get_mut(downstream.as_str()) {
                *count -= 1;
                if *count == 0 {
                    ready.insert(downstream.as_str());
                }
            }
        }
    }
    if order.len() != waiting_on.len() {
        return Err(refused(ContractRefusalKind::Cycle, ""));
    }
    Ok(order)
}

fn admitted_inputs(
    reader: &Resolved<'_>,
    resolved: &[Resolved<'_>],
) -> Result<Vec<AdmittedInput>, ContractRefusal> {
    let mut inputs = Vec::with_capacity(reader.inputs.len());
    for path in &reader.inputs {
        let writer = resolved
            .iter()
            .find(|writer| paths_overlap(path, &writer.output_path));
        // A commit names its output's spelling and a new version is matched by it, so an input
        // that only overlaps an output, or spells it another way, could never take one up.
        if writer.is_some_and(|writer| &writer.output_path != path) {
            return Err(refused(ContractRefusalKind::InvalidPath, path.as_str()));
        }
        let upstream = writer.map(|writer| TargetKey {
            stage: writer.stage.id.clone(),
            target: writer.target.id.clone(),
        });
        // W1: the public schema is fixed for the run, so upstream content may not narrow it.
        if upstream.is_some() && (reader.tool.has_revision_argument || reader.tool.has_compiler) {
            return Err(refused(
                ContractRefusalKind::SchemaNarrowedByUpstream,
                reader.tool.tool_id.as_str(),
            ));
        }
        inputs.push(AdmittedInput {
            path: path.clone(),
            upstream,
        });
    }
    Ok(inputs)
}

/// The two steps of [`StageContractDraft`] as one, for what is judged on its own. `Err` names what
/// is wrong and nothing is kept.
#[cfg(test)]
pub(crate) fn admit(
    run: StageRunFacts,
    declaration: &StageSetDeclaration,
    facts: &[HostToolFacts],
) -> Result<StageContract, ContractRefusal> {
    refuse_for_run(run)?;
    Ok(StageContractDraft::new(declaration, facts)?.contract)
}

/// A declaration judged by everything a run's facts have no say in. An entry that runs before
/// the OS sandbox is installed can judge this far. Only a session turns it into a contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageContractDraft {
    contract: StageContract,
}

impl StageContractDraft {
    pub(crate) fn new(
        declaration: &StageSetDeclaration,
        facts: &[HostToolFacts],
    ) -> Result<Self, ContractRefusal> {
        Ok(Self {
            contract: judge_declaration(declaration, facts)?,
        })
    }

    pub(crate) fn for_run(&self, run: StageRunFacts) -> Result<StageContract, ContractRefusal> {
        refuse_for_run(run)?;
        Ok(self.contract.clone())
    }

    /// The contract's digest. A run's facts only refuse a contract, so they are no part of it.
    pub(crate) fn contract_sha256(&self) -> String {
        self.contract.sha256()
    }

    /// The inputs of a tool that some stage's output feeds. G2 waits for a receipt for these.
    pub(crate) fn upstream_inputs(&self, tool_id: &str) -> BTreeSet<String> {
        self.contract
            .target_for_tool(tool_id)
            .into_iter()
            .flat_map(|target| target.inputs.iter())
            .filter(|input| input.upstream.is_some())
            .map(|input| input.path.clone())
            .collect()
    }

    pub(crate) fn target_for_tool(&self, tool_id: &str) -> Option<&AdmittedTarget> {
        self.contract.target_for_tool(tool_id)
    }

    pub(crate) fn tool_ids(&self) -> impl Iterator<Item = &str> {
        self.contract
            .targets()
            .map(|target| target.tool_id.as_str())
    }
}

fn judge_declaration(
    declaration: &StageSetDeclaration,
    facts: &[HostToolFacts],
) -> Result<StageContract, ContractRefusal> {
    if declaration.stages.is_empty() {
        return Err(refused(ContractRefusalKind::NoStages, ""));
    }
    if declaration.stages.len() > MAX_STAGES {
        return Err(refused(ContractRefusalKind::TooManyStages, ""));
    }
    let target_count = declaration
        .stages
        .iter()
        .map(|stage| stage.targets.len())
        .sum::<usize>();
    if target_count > MAX_TARGETS {
        return Err(refused(ContractRefusalKind::TooManyTargets, ""));
    }
    let resolved = resolve_targets(declaration, facts)?;
    refuse_output_collisions(&resolved)?;
    let edges = collect_edges(declaration, &resolved)?;
    let order = stage_order(declaration, &edges)?;
    let tools = resolved.iter().map(|item| item.tool).collect::<Vec<_>>();
    let limits = effective_limits(declaration.limits, &tools)?;
    if (limits.attempts_per_run as usize) < target_count {
        return Err(refused(ContractRefusalKind::InvalidLimit, ""));
    }
    let mut stages = Vec::with_capacity(order.len());
    for id in order {
        let mut targets = Vec::new();
        for item in resolved.iter().filter(|item| item.stage.id == id) {
            targets.push(AdmittedTarget {
                key: TargetKey {
                    stage: id.clone(),
                    target: item.target.id.clone(),
                },
                tool_id: item.tool.tool_id.clone(),
                output_id: item.tool.output_id.clone(),
                output_path: item.output_path.clone(),
                required_checks: required_checks(item.stage, item.tool)?,
                inputs: admitted_inputs(item, &resolved)?,
            });
        }
        let depends_on = edges
            .iter()
            .filter(|(_, downstream)| downstream == &id)
            .map(|(upstream, _)| upstream.clone())
            .collect();
        stages.push(AdmittedStage {
            id,
            depends_on,
            targets,
        });
    }
    Ok(StageContract { stages, limits })
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{
        admit_enforced as admit, facts, one_target, two_stage_contract,
    };
    use super::{
        ContractRefusalKind, HOST_MAX_ATTEMPTS_PER_RUN, HOST_MAX_ATTEMPTS_PER_TARGET,
        HOST_MAX_DEADLINE_SECONDS, HostToolFacts, MAX_EDGES, MAX_ID_BYTES, MAX_INPUTS_PER_TOOL,
        MAX_REQUIRED_CHECKS, MAX_STAGES, MAX_TARGETS, NarrowedStageLimits, StageDeclaration,
        StageRunFacts, StageSetDeclaration, TargetDeclaration, TargetKey, normalize_path,
    };

    fn set(stages: Vec<StageDeclaration>) -> StageSetDeclaration {
        StageSetDeclaration {
            stages,
            limits: NarrowedStageLimits::default(),
        }
    }

    fn kind(declaration: &StageSetDeclaration, tools: &[HostToolFacts]) -> ContractRefusalKind {
        admit(declaration, tools).unwrap_err().kind
    }

    /// `count` independent stages, one target each, with no paths in common.
    fn independent(count: usize) -> (StageSetDeclaration, Vec<HostToolFacts>) {
        let stages = (0..count)
            .map(|index| one_target(&format!("s{index:03}"), &format!("t{index:03}")))
            .collect();
        let tools = (0..count)
            .map(|index| facts(&format!("t{index:03}"), &format!("out/{index:03}"), &[]))
            .collect();
        (set(stages), tools)
    }

    #[test]
    fn the_stage_count_limit_is_exact() {
        let (at, tools) = independent(MAX_STAGES);
        assert!(admit(&at, &tools).is_ok());
        let (over, tools) = independent(MAX_STAGES + 1);
        assert_eq!(kind(&over, &tools), ContractRefusalKind::TooManyStages);
    }

    #[test]
    fn the_target_count_limit_is_exact() {
        let build = |count: usize| {
            let mut stages = vec![
                StageDeclaration {
                    id: "a".to_string(),
                    targets: Vec::new(),
                    depends_on: Vec::new(),
                    required_checks: Vec::new(),
                },
                StageDeclaration {
                    id: "b".to_string(),
                    targets: Vec::new(),
                    depends_on: Vec::new(),
                    required_checks: Vec::new(),
                },
            ];
            let mut tools = Vec::new();
            for index in 0..count {
                let tool = format!("t{index:03}");
                stages[index % 2].targets.push(TargetDeclaration {
                    id: tool.clone(),
                    tool_id: tool.clone(),
                });
                tools.push(facts(&tool, &format!("out/{index:03}"), &[]));
            }
            (set(stages), tools)
        };
        let (at, tools) = build(MAX_TARGETS);
        assert!(admit(&at, &tools).is_ok());
        let (over, tools) = build(MAX_TARGETS + 1);
        assert_eq!(kind(&over, &tools), ContractRefusalKind::TooManyTargets);
    }

    #[test]
    fn the_edge_count_limit_is_exact() {
        let build = |count: usize| {
            let (mut declaration, tools) = independent(20);
            let mut added = 0;
            'all: for downstream in 1..20 {
                for upstream in 0..downstream {
                    if added == count {
                        break 'all;
                    }
                    declaration.stages[downstream]
                        .depends_on
                        .push(format!("s{upstream:03}"));
                    added += 1;
                }
            }
            assert_eq!(added, count);
            (declaration, tools)
        };
        let (at, tools) = build(MAX_EDGES);
        assert!(admit(&at, &tools).is_ok());
        let (over, tools) = build(MAX_EDGES + 1);
        assert_eq!(kind(&over, &tools), ContractRefusalKind::TooManyEdges);
    }

    #[test]
    fn the_required_check_limit_counts_the_operators_and_the_declared_together() {
        let declared = |count: usize| {
            let mut stage = one_target("s", "t");
            stage.required_checks = (0..count).map(|index| format!("extra{index}")).collect();
            set(vec![stage])
        };
        let tools = [facts("t", "out/a", &[])];
        let floor = tools[0].required_checks.len();
        assert!(admit(&declared(MAX_REQUIRED_CHECKS - floor), &tools).is_ok());
        assert_eq!(
            kind(&declared(MAX_REQUIRED_CHECKS - floor + 1), &tools),
            ContractRefusalKind::TooManyRequiredChecks
        );
    }

    #[test]
    fn the_input_count_and_id_and_path_limits_are_exact() {
        let with_inputs = |count: usize| {
            let inputs = (0..count)
                .map(|index| format!("in/{index}"))
                .collect::<Vec<_>>();
            let refs = inputs.iter().map(String::as_str).collect::<Vec<_>>();
            vec![facts("t", "out/a", &refs)]
        };
        let declaration = set(vec![one_target("s", "t")]);
        assert!(admit(&declaration, &with_inputs(MAX_INPUTS_PER_TOOL)).is_ok());
        assert_eq!(
            kind(&declaration, &with_inputs(MAX_INPUTS_PER_TOOL + 1)),
            ContractRefusalKind::TooManyInputs
        );

        let long = "x".repeat(MAX_ID_BYTES);
        let tools = [facts("t", "out/a", &[])];
        assert!(admit(&set(vec![one_target(&long, "t")]), &tools).is_ok());
        let longer = "x".repeat(MAX_ID_BYTES + 1);
        assert_eq!(
            kind(&set(vec![one_target(&longer, "t")]), &tools),
            ContractRefusalKind::InvalidId
        );
    }

    #[test]
    fn unknown_references_and_duplicates_are_refused_by_type() {
        let tools = [facts("t", "out/a", &[]), facts("u", "out/b", &[])];

        assert_eq!(
            kind(&set(vec![]), &tools),
            ContractRefusalKind::NoStages,
            "a declared set with nothing in it is not off"
        );
        assert_eq!(
            kind(&set(vec![one_target("s", "missing")]), &tools),
            ContractRefusalKind::UnknownTool
        );
        let mut stage = one_target("s", "t");
        stage.depends_on = vec!["nowhere".to_string()];
        assert_eq!(
            kind(&set(vec![stage]), &tools),
            ContractRefusalKind::UnknownDependency
        );
        assert_eq!(
            kind(
                &set(vec![one_target("s", "t"), one_target("s", "u")]),
                &tools
            ),
            ContractRefusalKind::DuplicateStage
        );
        let mut twice = one_target("s", "t");
        twice.targets.push(TargetDeclaration {
            id: "t".to_string(),
            tool_id: "u".to_string(),
        });
        assert_eq!(
            kind(&set(vec![twice]), &tools),
            ContractRefusalKind::DuplicateTarget
        );
        assert_eq!(
            kind(
                &set(vec![one_target("s", "t"), one_target("r", "t")]),
                &tools
            ),
            ContractRefusalKind::ToolInTwoTargets
        );
        let mut empty = one_target("s", "t");
        empty.targets.clear();
        assert_eq!(
            kind(&set(vec![empty]), &tools),
            ContractRefusalKind::StageWithoutTargets
        );
        let mut repeated = one_target("s", "t");
        repeated.depends_on = vec!["r".to_string(), "r".to_string()];
        assert_eq!(
            kind(&set(vec![repeated, one_target("r", "u")]), &tools),
            ContractRefusalKind::DuplicateDependency
        );
        assert_eq!(
            kind(
                &set(vec![one_target("s", "t")]),
                &[facts("t", "out/a", &[]), facts("t", "out/b", &[])]
            ),
            ContractRefusalKind::DuplicateToolFacts
        );
        let mut bad_id = one_target("s", "t");
        bad_id.id = "s/../x".to_string();
        assert_eq!(
            kind(&set(vec![bad_id]), &tools),
            ContractRefusalKind::InvalidId
        );
    }

    #[test]
    fn a_cycle_is_refused_whether_derived_declared_or_on_one_stage() {
        // Declared.
        let mut a = one_target("a", "t");
        a.depends_on = vec!["b".to_string()];
        let mut b = one_target("b", "u");
        b.depends_on = vec!["a".to_string()];
        let tools = [facts("t", "out/a", &[]), facts("u", "out/b", &[])];
        assert_eq!(kind(&set(vec![a, b]), &tools), ContractRefusalKind::Cycle);

        // Derived from the paths alone.
        let tools = [
            facts("t", "out/a", &["out/b"]),
            facts("u", "out/b", &["out/a"]),
        ];
        assert_eq!(
            kind(
                &set(vec![one_target("a", "t"), one_target("b", "u")]),
                &tools
            ),
            ContractRefusalKind::Cycle
        );

        // A stage that reads its own output, and one that depends on itself.
        let tools = [facts("t", "out/a", &["out/a"])];
        assert_eq!(
            kind(&set(vec![one_target("a", "t")]), &tools),
            ContractRefusalKind::Cycle
        );
        let mut own = one_target("a", "t");
        own.depends_on = vec!["a".to_string()];
        assert_eq!(
            kind(&set(vec![own]), &[facts("t", "out/a", &[])]),
            ContractRefusalKind::Cycle
        );
    }

    #[test]
    fn an_edge_comes_from_the_paths_even_when_nothing_declares_it() {
        let contract = two_stage_contract();
        assert_eq!(contract.stages()[0].id, "alpha");
        assert_eq!(contract.stages()[1].id, "beta");
        assert!(contract.stages()[1].depends_on.contains("alpha"));
        let input = &contract.stages()[1].targets[0].inputs[0];
        assert_eq!(
            input.upstream,
            Some(TargetKey {
                stage: "alpha".to_string(),
                target: "main".to_string()
            })
        );
    }

    #[test]
    fn another_spelling_of_a_path_does_not_hide_an_edge_or_a_collision() {
        // The same edge through `./` and a doubled slash. Another case is the same file on some
        // filesystems, and no commit could name it, so it is refused rather than left without
        // an edge.
        let tools = [
            facts("t", "out/a.json", &[]),
            facts("u", "out/b.json", &["./out//a.json"]),
        ];
        let contract = admit(
            &set(vec![one_target("beta", "u"), one_target("alpha", "t")]),
            &tools,
        )
        .unwrap();
        assert_eq!(contract.stages()[0].id, "alpha");
        assert!(contract.stages()[1].depends_on.contains("alpha"));
        let tools = [
            facts("t", "out/a.json", &[]),
            facts("u", "out/b.json", &["OUT/A.JSON"]),
        ];
        assert_eq!(
            kind(
                &set(vec![one_target("beta", "u"), one_target("alpha", "t")]),
                &tools
            ),
            ContractRefusalKind::InvalidPath
        );
        for second in ["./out//a.json", "OUT/a.json", "out/a.json/inner"] {
            let tools = [facts("t", "out/a.json", &[]), facts("u", second, &[])];
            assert_eq!(
                kind(
                    &set(vec![one_target("a", "t"), one_target("b", "u")]),
                    &tools
                ),
                ContractRefusalKind::OutputCollision,
                "{second}"
            );
        }
    }

    #[test]
    fn a_name_that_only_starts_like_an_output_is_not_a_dependency() {
        let tools = [
            facts("t", "out/a", &[]),
            facts("u", "out/ab", &["out/a.b", "out/abc"]),
        ];
        let contract = admit(
            &set(vec![one_target("a", "t"), one_target("b", "u")]),
            &tools,
        )
        .unwrap();
        let target = contract.target_for_tool("u").unwrap();
        assert!(target.inputs.iter().all(|input| input.upstream.is_none()));
        assert!(contract.stage("b").unwrap().depends_on.is_empty());
    }

    #[test]
    fn a_path_outside_the_workspace_shape_is_refused() {
        for path in ["", "/etc/x", "../x", "a/../../x", "a\\b", ".", "a\0b"] {
            assert_eq!(
                normalize_path(path).unwrap_err().kind,
                ContractRefusalKind::InvalidPath,
                "{path:?}"
            );
        }
        assert_eq!(normalize_path("./a//b/").unwrap(), "a/b");
    }

    #[test]
    fn upstream_content_may_not_narrow_a_fixed_schema() {
        for (revision, compiler) in [(true, false), (false, true)] {
            let mut downstream = facts("u", "out/b", &["out/a"]);
            downstream.has_revision_argument = revision;
            downstream.has_compiler = compiler;
            let tools = [facts("t", "out/a", &[]), downstream];
            assert_eq!(
                kind(
                    &set(vec![one_target("a", "t"), one_target("b", "u")]),
                    &tools
                ),
                ContractRefusalKind::SchemaNarrowedByUpstream
            );
        }
        // On a fixed input, the same schema stays allowed: nothing upstream can change it.
        let mut fixed = facts("t", "out/a", &["fixed/in.json"]);
        fixed.has_revision_argument = true;
        fixed.has_compiler = true;
        assert!(admit(&set(vec![one_target("a", "t")]), &[fixed]).is_ok());
    }

    #[test]
    fn the_operators_checks_are_a_floor_a_declaration_can_only_add_to() {
        let mut stage = one_target("s", "t");
        stage.required_checks = vec!["declared".to_string()];
        let tools = [facts("t", "out/a", &[])];
        let contract = admit(&set(vec![stage]), &tools).unwrap();
        let required = &contract.target_for_tool("t").unwrap().required_checks;
        for id in &tools[0].required_checks {
            assert!(required.contains(id));
        }
        assert!(required.contains("declared"));

        let mut bare = facts("t", "out/a", &[]);
        bare.required_checks.clear();
        let mut declaring = one_target("s", "t");
        declaring.required_checks = vec!["declared".to_string()];
        assert_eq!(
            kind(&set(vec![declaring]), &[bare]),
            ContractRefusalKind::NoOperatorChecks,
            "a declaration alone cannot be the floor"
        );

        let mut repeated = facts("t", "out/a", &[]);
        repeated.required_checks = vec!["c".to_string(), "c".to_string()];
        assert_eq!(
            kind(&set(vec![one_target("s", "t")]), &[repeated]),
            ContractRefusalKind::DuplicateCheck
        );
    }

    #[test]
    fn host_limits_hold_at_their_edge_and_a_declaration_only_narrows() {
        let declaration = set(vec![one_target("s", "t")]);
        let with = |target: u32, run: u32, deadline: u64| {
            let mut tool = facts("t", "out/a", &[]);
            tool.limits.attempts_per_target = target;
            tool.limits.attempts_per_run = run;
            tool.limits.deadline_seconds = deadline;
            vec![tool]
        };
        let (target, run, deadline) = (
            HOST_MAX_ATTEMPTS_PER_TARGET,
            HOST_MAX_ATTEMPTS_PER_RUN,
            HOST_MAX_DEADLINE_SECONDS,
        );
        assert_eq!((target, run, deadline), (8, 64, 3600));
        assert!(admit(&declaration, &with(target, run, deadline)).is_ok());
        for over in [
            with(target + 1, run, deadline),
            with(target, run + 1, deadline),
            with(target, run, deadline + 1),
            with(0, run, deadline),
            with(target, 0, deadline),
            with(target, run, 0),
        ] {
            assert_eq!(kind(&declaration, &over), ContractRefusalKind::InvalidLimit);
        }

        let tools = with(4, 10, 100);
        let narrowed = StageSetDeclaration {
            stages: declaration.stages.clone(),
            limits: NarrowedStageLimits {
                attempts_per_target: Some(2),
                attempts_per_run: Some(3),
                deadline_seconds: Some(50),
            },
        };
        let limits = admit(&narrowed, &tools).unwrap().limits();
        assert_eq!(limits.attempts_per_target, 2);
        assert_eq!(limits.attempts_per_run, 3);
        assert_eq!(limits.deadline_seconds, 50);

        for widened in [
            NarrowedStageLimits {
                attempts_per_target: Some(5),
                ..NarrowedStageLimits::default()
            },
            NarrowedStageLimits {
                attempts_per_run: Some(11),
                ..NarrowedStageLimits::default()
            },
            NarrowedStageLimits {
                deadline_seconds: Some(101),
                ..NarrowedStageLimits::default()
            },
        ] {
            let attempt = StageSetDeclaration {
                stages: declaration.stages.clone(),
                limits: widened,
            };
            assert_eq!(kind(&attempt, &tools), ContractRefusalKind::WidenedLimit);
        }
    }

    #[test]
    fn a_run_limit_below_the_target_count_could_never_finish() {
        let (declaration, mut tools) = independent(3);
        for tool in &mut tools {
            tool.limits.attempts_per_run = 2;
        }
        assert_eq!(
            kind(&declaration, &tools),
            ContractRefusalKind::InvalidLimit
        );
    }

    #[test]
    fn the_run_facts_are_what_the_process_holds_when_they_are_asked_for() {
        use crate::tools::os_sandbox::{
            SandboxAvailability, SandboxMode, SandboxPolicy, SandboxTestState, install_for_test,
        };
        let _sandbox = SandboxTestState::acquire();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let tmp = temp.path().join("sandbox/run");
        std::fs::create_dir_all(&tmp).unwrap();
        let mut config = crate::tools::profile_tools::effects::config_for_test(&root);
        config.state_dir = temp.path().join("state");
        let of_session = || StageRunFacts::of_session(&config);

        // Nothing installed: nothing is enforced, and no earlier answer stands in for this one.
        assert_eq!(of_session(), StageRunFacts::for_test(false, false));
        install_for_test(
            SandboxMode::Auto,
            SandboxPolicy::new(&root, &tmp),
            SandboxAvailability::Available,
        );
        assert_eq!(of_session(), StageRunFacts::for_test(true, false));
        install_for_test(
            SandboxMode::Off,
            SandboxPolicy::new(&root, &tmp),
            SandboxAvailability::Available,
        );
        assert_eq!(of_session(), StageRunFacts::for_test(false, false));
        install_for_test(
            SandboxMode::Auto,
            SandboxPolicy::new(&root, &tmp),
            SandboxAvailability::Unavailable("no backend in this test".to_string()),
        );
        assert_eq!(of_session(), StageRunFacts::for_test(false, false));

        crate::planner::external_reproducer::mark_workflow_node(&config).unwrap();
        assert_eq!(of_session(), StageRunFacts::for_test(false, true));
    }

    #[test]
    fn a_draft_is_judged_without_the_run_and_only_a_run_makes_it_a_contract() {
        let declaration = set(vec![one_target("upper", "a"), one_target("lower", "b")]);
        let tools = [
            facts("a", "out/a", &["fixed/seed"]),
            facts("b", "out/b", &["out/a", "fixed/other"]),
        ];
        let draft = super::StageContractDraft::new(&declaration, &tools).unwrap();
        assert_eq!(
            draft.upstream_inputs("b"),
            std::collections::BTreeSet::from(["out/a".to_string()])
        );
        assert!(draft.upstream_inputs("a").is_empty());
        assert_eq!(
            draft
                .for_run(StageRunFacts::for_test(false, false))
                .unwrap_err()
                .kind,
            ContractRefusalKind::OsSandboxNotEnforced
        );
        assert_eq!(
            draft
                .for_run(StageRunFacts::for_test(true, true))
                .unwrap_err()
                .kind,
            ContractRefusalKind::WorkflowChild
        );
        assert_eq!(
            draft.for_run(StageRunFacts::for_test(true, false)).unwrap(),
            admit(&declaration, &tools).unwrap()
        );
    }

    #[test]
    fn an_input_that_only_overlaps_an_output_or_spells_it_otherwise_cannot_take_it_up() {
        for input in ["OUT/A", "out/a/below", "out"] {
            let declaration = set(vec![one_target("upper", "a"), one_target("lower", "b")]);
            let tools = [facts("a", "out/a", &[]), facts("b", "other/b", &[input])];
            assert_eq!(
                kind(&declaration, &tools),
                ContractRefusalKind::InvalidPath,
                "{input}"
            );
        }
    }

    /// B4: a treatment's workspace is another ledger, where the counts, the deadline, and a
    /// secret's mark would all be gone.
    #[test]
    fn no_contract_comes_out_of_a_run_that_may_recover_by_itself() {
        let declaration = set(vec![one_target("s", "t")]);
        let tools = [facts("t", "out/a", &[])];
        let recovering = StageRunFacts {
            os_sandbox_enforced: true,
            workflow_child: false,
            recovers_by_itself: true,
        };
        let draft = super::StageContractDraft::new(&declaration, &tools).unwrap();
        assert_eq!(
            draft.for_run(recovering).unwrap_err().kind,
            ContractRefusalKind::RecoversByItself
        );

        let mut config =
            crate::tools::profile_tools::effects::config_for_test(std::path::Path::new("/none"));
        assert!(!StageRunFacts::of_session(&config).recovers_by_itself);
        config.recovery_plan_auto_runs = 1;
        assert!(StageRunFacts::of_session(&config).recovers_by_itself);
    }

    #[test]
    fn no_contract_comes_out_of_a_run_without_an_os_boundary_or_in_a_workflow_child() {
        let declaration = set(vec![one_target("s", "t")]);
        let tools = [facts("t", "out/a", &[])];
        let run = |os_sandbox_enforced, workflow_child| StageRunFacts {
            os_sandbox_enforced,
            workflow_child,
            recovers_by_itself: false,
        };
        let kind_for = |facts| super::admit(facts, &declaration, &tools).unwrap_err().kind;
        assert_eq!(
            kind_for(run(false, false)),
            ContractRefusalKind::OsSandboxNotEnforced
        );
        assert_eq!(
            kind_for(run(true, true)),
            ContractRefusalKind::WorkflowChild
        );
        assert_eq!(
            kind_for(run(false, true)),
            ContractRefusalKind::WorkflowChild
        );
        assert!(super::admit(run(true, false), &declaration, &tools).is_ok());

        // The run is judged first: the refusal does not depend on what was declared.
        assert_eq!(
            super::admit(run(false, false), &set(vec![]), &tools)
                .unwrap_err()
                .kind,
            ContractRefusalKind::OsSandboxNotEnforced
        );
    }

    #[test]
    fn the_contract_digest_follows_the_graph_checks_and_limits() {
        let base = two_stage_contract();
        assert_eq!(base.sha256(), two_stage_contract().sha256());
        let mut tool = facts("t", "out/a", &[]);
        let once = admit(
            &set(vec![one_target("s", "t")]),
            std::slice::from_ref(&tool),
        )
        .unwrap();
        tool.limits.attempts_per_target = 1;
        let lowered = admit(&set(vec![one_target("s", "t")]), &[tool]).unwrap();
        assert_ne!(once.sha256(), lowered.sha256());
    }
}
