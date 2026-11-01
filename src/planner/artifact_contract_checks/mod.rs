//! `artifact_contract_check`: run one operator-registered external adapter and
//! bind its generic result envelope to final acceptance (Issue #933).
//!
//! Trust is split three ways:
//!
//! - The operator owns the registry. `COMMANDAGENT_ADAPTER_CONFIG_ROOT`, set
//!   before the run, names a directory outside the workspace whose
//!   `adapters/<adapter_id>.json` descriptors pin the executable, the tooling
//!   root, its members and allowed links, the required check/target rows, the
//!   expected hashes of initial inputs, and the limits. Nothing in a manifest,
//!   pack, or workspace request can add or change a descriptor.
//! - The model owns only the workspace request: a request ID and the
//!   workspace-relative inputs to check. It cannot name an executable, argv,
//!   module, model, or an expected hash.
//! - The host verifies the adapter's envelope: protocol, request ID, run nonce,
//!   input digests, the unique coverage of the required rows, the count
//!   identity, and the execution state. The domain verdict is capped at
//!   `partial`; it is recorded as a receipt and never lifts native acceptance or
//!   assurance.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::planner::capability_catalog::artifact::ArtifactContractCheck;

pub(crate) mod process;
pub(crate) mod protocol;
#[cfg(test)]
mod tests;

use protocol::{DomainVerdict, RowFacts, RowKey};

pub(crate) const CONFIG_ROOT_ENV: &str = "COMMANDAGENT_ADAPTER_CONFIG_ROOT";
pub(crate) const DESCRIPTOR_PROTOCOL: &str = "artifact-contract.adapter/v1";
const DESCRIPTOR_DIR: &str = "adapters";
const MAX_DESCRIPTOR_BYTES: u64 = 1 << 20;
const MAX_MEMBERS: usize = 4_096;
const MAX_LINKS: usize = 4_096;
const MAX_ARGS: usize = 32;
const MAX_ARG_BYTES: usize = 1_024;
const MAX_TOOL_TREE_ENTRIES: usize = 200_000;
const MAX_TOOL_TREE_DEPTH: usize = 64;
const MAX_INPUT_BYTES: u64 = 64 << 20;
const MAX_TOOLING_FILE_BYTES: u64 = 1 << 30;
const MAX_REASONS: usize = 8;

/// An operator descriptor. Only this generic schema interprets its values.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Descriptor {
    pub(crate) protocol: String,
    pub(crate) adapter_id: String,
    pub(crate) executable: PinnedExecutable,
    pub(crate) tooling_root: PathBuf,
    pub(crate) entrypoint: String,
    #[serde(default)]
    pub(crate) args: Vec<String>,
    #[serde(deserialize_with = "unique_map")]
    pub(crate) members: BTreeMap<String, String>,
    #[serde(default, deserialize_with = "unique_map")]
    pub(crate) links: BTreeMap<String, String>,
    pub(crate) required: Vec<RowSet>,
    #[serde(default)]
    pub(crate) optional: Vec<RowSet>,
    pub(crate) inputs: Vec<PinnedInput>,
    #[serde(default)]
    pub(crate) limits: Limits,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PinnedExecutable {
    pub(crate) path: PathBuf,
    pub(crate) sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RowSet {
    pub(crate) check_id: String,
    #[serde(default)]
    pub(crate) targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PinnedInput {
    pub(crate) input_id: String,
    #[serde(default)]
    pub(crate) sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Limits {
    pub(crate) timeout_seconds: u64,
    pub(crate) max_stream_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout_seconds: process::FIXED_TIMEOUT.as_secs(),
            max_stream_bytes: protocol::MAX_STREAM_BYTES,
        }
    }
}

/// A string map that refuses a repeated key instead of keeping the last value.
fn unique_map<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct UniqueMap;

    impl<'de> serde::de::Visitor<'de> for UniqueMap {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map of strings without repeated keys")
        }

        fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut map = BTreeMap::new();
            while let Some((key, value)) = access.next_entry::<String, String>()? {
                if map.contains_key(&key) {
                    return Err(serde::de::Error::custom(format!("repeated key `{key}`")));
                }
                map.insert(key, value);
            }
            Ok(map)
        }
    }

    deserializer.deserialize_map(UniqueMap)
}

/// A descriptor admitted from the operator registry.
#[derive(Debug, Clone)]
pub(crate) struct Admitted {
    pub(crate) descriptor: Descriptor,
    pub(crate) descriptor_sha256: String,
    pub(crate) config_root: PathBuf,
    pub(crate) descriptor_path: PathBuf,
}

/// The operator registry root, read only from the process environment that
/// the operator set before the run.
pub(crate) fn config_root_from_environment() -> Option<PathBuf> {
    std::env::var_os(CONFIG_ROOT_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Load-time admission used by the capability catalog, so an unset, unknown,
/// or invalid registry entry, and operator tooling that is missing or does not
/// match its pins and links, is refused while the manifest or pack loads,
/// before any provider is built. The workspace separation is added by
/// [`preflight`] once the run's workspace, profile, and pack are known.
pub(crate) fn admit_from_environment(adapter_id: &str) -> Result<(), String> {
    let admitted = admit(config_root_from_environment().as_deref(), adapter_id)?;
    verify_tooling(&admitted.descriptor).map(|_| ())
}

/// Pre-provider check for the adapters that this run's profile and effective
/// pack declare: the registry, descriptor, tooling root, and executable must
/// lie outside `workspace`, and the tooling must match its pins and links.
///
/// The adapter IDs come from the same declarations final acceptance runs:
/// the effective profile's registered manifest (re-read without following a
/// symlink or waiting on a FIFO, bounded, and required to match the hash
/// recorded when it was loaded) and the effective pack selection (the CLI's
/// choice or an inherited one, verified for identity, hash, and pin as final
/// acceptance verifies it). Other loaded or previously registered profiles
/// are not consulted. When neither declares `artifact_contract_check`, this
/// returns without reading the registry; it reads the pack selection
/// variables and, for a registered external profile that declares the check,
/// its manifest.
pub fn preflight(workspace: &Path, profile: &str) -> Result<(), String> {
    let mut adapter_ids = profile_adapter_ids(profile)?;
    adapter_ids.extend(
        crate::planner::pack::runtime::selected_artifact_adapter_ids()
            .map_err(|error| format!("selected pack cannot be verified: {error:#}"))?,
    );
    if adapter_ids.is_empty() {
        return Ok(());
    }
    let workspace = fs::canonicalize(workspace)
        .map_err(|error| format!("workspace cannot be resolved: {error}"))?;
    for adapter_id in adapter_ids {
        let admitted = admit(config_root_from_environment().as_deref(), &adapter_id)?;
        separate_from_workspace(&admitted, &workspace)
            .and_then(|()| verify_tooling(&admitted.descriptor).map(|_| ()))
            .map_err(|reason| format!("artifact contract adapter `{adapter_id}`: {reason}"))?;
    }
    Ok(())
}

/// [`preflight`] for a workflow child node, whose workspace is its origin. The
/// pack selection is the one the outer run installed for the whole process.
pub fn preflight_for_workflow_node(workspace: &Path, profile: &str) -> Result<(), String> {
    preflight(workspace, profile)
}

fn profile_adapter_ids(profile: &str) -> Result<BTreeSet<String>, String> {
    let check_id = crate::planner::capability_catalog::artifact::ID;
    match crate::planner::extension_profiles::find(profile) {
        Some(extension) if extension.contract_checks.iter().any(|id| id == check_id) => {
            registered_manifest_adapter_ids(
                Path::new(extension.manifest_path),
                extension.manifest_hash,
            )
            .map_err(|reason| format!("profile `{profile}` manifest {reason}"))
        }
        _ => Ok(BTreeSet::new()),
    }
}

/// Re-read a registered manifest only as the exact bytes that were loaded:
/// a safe regular-file open, the loader's byte bound, and the registration
/// hash. A manifest changed since loading is refused, so the IDs equal the
/// loaded declaration that final acceptance runs.
pub(crate) fn registered_manifest_adapter_ids(
    path: &Path,
    expected_hash: &str,
) -> Result<BTreeSet<String>, String> {
    let bytes = read_bounded(
        path,
        crate::planner::profile_manifest::source::MAX_MANIFEST_BYTES,
    )?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if crate::planner::profile_manifest::source::exact_byte_hash(name, &bytes) != expected_hash {
        return Err("changed since it was loaded; restart the run".to_string());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "is not UTF-8".to_string())?;
    let manifest: toml::Value =
        toml::from_str(text).map_err(|error| format!("is invalid: {error}"))?;
    check_binding_adapter_ids(&manifest)
}

/// The `adapter_id` of every `artifact_contract_check` entry declared under
/// the top-level `checks` bindings. Objects of the same shape elsewhere are
/// not declarations and are ignored; a malformed declaration is an error.
pub(crate) fn check_binding_adapter_ids(
    manifest: &toml::Value,
) -> Result<BTreeSet<String>, String> {
    let check_id = crate::planner::capability_catalog::artifact::ID;
    let mut ids = BTreeSet::new();
    let Some(bindings) = manifest.get("checks") else {
        return Ok(ids);
    };
    let bindings = bindings
        .as_table()
        .ok_or_else(|| "`checks` is not a table".to_string())?;
    for (binding, entries) in bindings {
        let entries = entries
            .as_array()
            .ok_or_else(|| format!("`checks.{binding}` is not an array"))?;
        for entry in entries {
            if entry.get("id").and_then(toml::Value::as_str) != Some(check_id) {
                continue;
            }
            let adapter = entry
                .get("params")
                .and_then(|params| params.get("adapter_id"))
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("`checks.{binding}` {check_id} has no string adapter_id"))?;
            ids.insert(adapter.to_string());
        }
    }
    Ok(ids)
}

pub(crate) fn admit(config_root: Option<&Path>, adapter_id: &str) -> Result<Admitted, String> {
    let Some(config_root) = config_root else {
        return Err(format!(
            "{CONFIG_ROOT_ENV} is not set; the operator registry must be configured before the run"
        ));
    };
    if !config_root.is_absolute() {
        return Err(format!("{CONFIG_ROOT_ENV} must be an absolute path"));
    }
    protocol::validate_slug(adapter_id).map_err(|reason| format!("adapter_id {reason}"))?;
    let root = fs::canonicalize(config_root)
        .map_err(|error| format!("{CONFIG_ROOT_ENV} cannot be resolved: {error}"))?;
    if !root.is_dir() {
        return Err(format!("{CONFIG_ROOT_ENV} is not a directory"));
    }
    let directory = root.join(DESCRIPTOR_DIR);
    if !plain_directory(&directory) {
        return Err(format!(
            "operator registry `{DESCRIPTOR_DIR}/` is missing or is a symlink"
        ));
    }
    let path = directory.join(format!("{adapter_id}.json"));
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| format!("adapter `{adapter_id}` is not registered by the operator"))?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "adapter `{adapter_id}` descriptor must be a regular file, not a symlink"
        ));
    }
    let bytes = read_bounded(&path, MAX_DESCRIPTOR_BYTES)
        .map_err(|reason| format!("adapter `{adapter_id}` descriptor {reason}"))?;
    protocol::check_json_depth(&bytes)?;
    let descriptor: Descriptor = serde_json::from_slice(&bytes)
        .map_err(|error| format!("adapter `{adapter_id}` descriptor is invalid: {error}"))?;
    validate_descriptor(&descriptor, adapter_id)
        .map_err(|reason| format!("adapter `{adapter_id}` descriptor is invalid: {reason}"))?;
    Ok(Admitted {
        descriptor,
        descriptor_sha256: sha256_hex(&bytes),
        config_root: root,
        descriptor_path: path,
    })
}

fn validate_descriptor(descriptor: &Descriptor, adapter_id: &str) -> Result<(), String> {
    if descriptor.protocol != DESCRIPTOR_PROTOCOL {
        return Err(format!("protocol must be `{DESCRIPTOR_PROTOCOL}`"));
    }
    if descriptor.adapter_id != adapter_id {
        return Err("adapter_id differs from its registry file name".to_string());
    }
    canonical_absolute(&descriptor.executable.path, "executable.path")?;
    if !descriptor.executable.path.is_file() {
        return Err("executable.path is not a file".to_string());
    }
    protocol::validate_sha256("executable.sha256", &descriptor.executable.sha256)?;
    canonical_absolute(&descriptor.tooling_root, "tooling_root")?;
    if !descriptor.tooling_root.is_dir() {
        return Err("tooling_root is not a directory".to_string());
    }
    safe_relative(&descriptor.entrypoint, "entrypoint")?;
    if !descriptor.members.contains_key(&descriptor.entrypoint) {
        return Err("entrypoint must be a pinned member".to_string());
    }
    if descriptor.members.is_empty() || descriptor.members.len() > MAX_MEMBERS {
        return Err(format!("members must pin 1..{MAX_MEMBERS} files"));
    }
    for (member, digest) in &descriptor.members {
        safe_relative(member, "member")?;
        protocol::validate_sha256("member sha256", digest)?;
    }
    if descriptor.links.len() > MAX_LINKS {
        return Err(format!("links may list at most {MAX_LINKS} entries"));
    }
    for (link, target) in &descriptor.links {
        safe_relative(link, "link")?;
        safe_relative(target, "link target")?;
    }
    if descriptor.args.len() > MAX_ARGS
        || descriptor
            .args
            .iter()
            .any(|arg| arg.len() > MAX_ARG_BYTES || arg.contains('\0'))
    {
        return Err(format!(
            "args must be at most {MAX_ARGS} strings of {MAX_ARG_BYTES} bytes without NUL"
        ));
    }
    let (required, optional) = row_sets(descriptor)?;
    if required.is_empty() {
        return Err("required must name at least one check".to_string());
    }
    if required.len() + optional.len() > protocol::MAX_ROWS {
        return Err(format!(
            "at most {} rows may be registered",
            protocol::MAX_ROWS
        ));
    }
    if descriptor.inputs.is_empty() || descriptor.inputs.len() > protocol::MAX_INPUTS {
        return Err(format!(
            "inputs must name 1..{} inputs",
            protocol::MAX_INPUTS
        ));
    }
    let mut inputs = BTreeSet::new();
    for input in &descriptor.inputs {
        protocol::validate_opaque_id("input_id", &input.input_id)?;
        if !inputs.insert(input.input_id.as_str()) {
            return Err(format!("input `{}` is repeated", input.input_id));
        }
        if let Some(digest) = &input.sha256 {
            protocol::validate_sha256("input sha256", digest)?;
        }
    }
    let limits = descriptor.limits;
    if !(1..=process::FIXED_TIMEOUT.as_secs()).contains(&limits.timeout_seconds)
        || !(1..=protocol::MAX_STREAM_BYTES).contains(&limits.max_stream_bytes)
    {
        return Err(format!(
            "limits must stay within 1..{} seconds and 1..{} stream bytes",
            process::FIXED_TIMEOUT.as_secs(),
            protocol::MAX_STREAM_BYTES
        ));
    }
    Ok(())
}

type RowSets = (BTreeSet<RowKey>, BTreeSet<RowKey>);

fn row_sets(descriptor: &Descriptor) -> Result<RowSets, String> {
    let mut all = BTreeSet::new();
    let mut expand = |sets: &[RowSet]| -> Result<BTreeSet<RowKey>, String> {
        let mut rows = BTreeSet::new();
        for set in sets {
            protocol::validate_opaque_id("check_id", &set.check_id)?;
            let targets = if set.targets.is_empty() {
                vec![None]
            } else {
                set.targets.iter().cloned().map(Some).collect()
            };
            for target in targets {
                if let Some(target) = &target {
                    protocol::validate_opaque_id("target_id", target)?;
                }
                let key = (set.check_id.clone(), target);
                if !all.insert(key.clone()) {
                    return Err(format!("row `{}` is registered twice", key.0));
                }
                rows.insert(key);
            }
        }
        Ok(rows)
    };
    let required = expand(&descriptor.required)?;
    let optional = expand(&descriptor.optional)?;
    Ok((required, optional))
}

/// The host receipt of one adapter run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub(crate) adapter_id: String,
    pub(crate) request: String,
    pub(crate) request_id: Option<String>,
    pub(crate) stopped_at: &'static str,
    pub(crate) descriptor_sha256: Option<String>,
    pub(crate) executable_sha256: Option<String>,
    pub(crate) input_digests: BTreeMap<String, String>,
    pub(crate) process: Option<ProcessReceipt>,
    pub(crate) facts: Option<RowFacts>,
    pub(crate) uncapped: DomainVerdict,
    pub(crate) domain: DomainVerdict,
    pub(crate) native_passed: bool,
    pub(crate) reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessReceipt {
    pub(crate) ended: &'static str,
    pub(crate) exit_code: Option<i32>,
    pub(crate) elapsed_ms: u128,
    pub(crate) reaped: bool,
    pub(crate) group_stopped: bool,
    pub(crate) stdin_complete: bool,
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
}

impl Receipt {
    fn new(check: &ArtifactContractCheck) -> Self {
        Self {
            adapter_id: check.adapter_id.clone(),
            request: check.request.clone(),
            request_id: None,
            stopped_at: "registry",
            descriptor_sha256: None,
            executable_sha256: None,
            input_digests: BTreeMap::new(),
            process: None,
            facts: None,
            uncapped: DomainVerdict::None,
            domain: DomainVerdict::None,
            native_passed: false,
            reasons: Vec::new(),
        }
    }

    fn fail(mut self, stopped_at: &'static str, reasons: Vec<String>) -> Self {
        self.stopped_at = stopped_at;
        self.reasons = reasons;
        self.reasons.truncate(MAX_REASONS);
        self.uncapped = DomainVerdict::None;
        self.domain = DomainVerdict::None;
        self.native_passed = false;
        self
    }

    /// Native failure reasons; empty only when every required row passed.
    pub(crate) fn failures(&self) -> Vec<String> {
        if self.native_passed {
            return Vec::new();
        }
        let detail = self
            .reasons
            .first()
            .map(String::as_str)
            .unwrap_or("unspecified failure");
        vec![format!(
            "artifact contract check `{}` failed at {}: {detail}",
            self.adapter_id, self.stopped_at
        )]
    }
}

/// Run one check from final acceptance and record its receipt.
pub(crate) fn run_and_record(
    workspace: &Path,
    check: &ArtifactContractCheck,
    events_path: Option<&Path>,
    source: &str,
    owner_id: &str,
) -> Vec<String> {
    let receipt = run_check(
        workspace,
        check,
        config_root_from_environment().as_deref(),
        || false,
    );
    record(&receipt, events_path, source, owner_id);
    receipt.failures()
}

pub(crate) fn run_check<F>(
    workspace: &Path,
    check: &ArtifactContractCheck,
    config_root: Option<&Path>,
    is_cancelled: F,
) -> Receipt
where
    F: Fn() -> bool,
{
    let receipt = Receipt::new(check);
    let admitted = match admit(config_root, &check.adapter_id) {
        Ok(admitted) => admitted,
        Err(reason) => return receipt.fail("registry", vec![reason]),
    };
    let mut receipt = Receipt {
        descriptor_sha256: Some(admitted.descriptor_sha256.clone()),
        ..receipt
    };
    let workspace = match fs::canonicalize(workspace) {
        Ok(workspace) => workspace,
        Err(error) => {
            return receipt.fail(
                "trust",
                vec![format!("workspace cannot be resolved: {error}")],
            );
        }
    };
    if let Err(reason) = separate_from_workspace(&admitted, &workspace) {
        return receipt.fail("trust", vec![reason]);
    }
    let descriptor = &admitted.descriptor;
    match verify_tooling(descriptor) {
        Ok(executable_sha256) => receipt.executable_sha256 = Some(executable_sha256),
        Err(reason) => return receipt.fail("pin", vec![reason]),
    }
    let request = read_request(&workspace, &check.request);
    remember_bound_paths(&workspace, check, descriptor, request.as_ref().ok());
    let request = match request {
        Ok(request) => request,
        Err(reason) => return receipt.fail("request", vec![reason]),
    };
    receipt.request_id = Some(request.request_id.clone());
    let inputs = match bind_inputs(&workspace, descriptor, &request) {
        Ok(inputs) => inputs,
        Err(reason) => return receipt.fail("input", vec![reason]),
    };
    receipt.input_digests = inputs
        .iter()
        .map(|input| (input.input_id.clone(), input.sha256.clone()))
        .collect();
    let nonce = uuid::Uuid::now_v7().to_string();
    let envelope = protocol::Envelope {
        protocol: protocol::ENVELOPE_PROTOCOL,
        request_id: &request.request_id,
        nonce: &nonce,
        workspace: &workspace.to_string_lossy(),
        inputs: inputs
            .iter()
            .map(|input| protocol::EnvelopeInput {
                input_id: &input.input_id,
                path: &input.relative,
                sha256: &input.sha256,
            })
            .collect(),
    };
    let stdin = serde_json::to_vec(&envelope).unwrap_or_default();
    if stdin.is_empty() || stdin.len() > protocol::MAX_REQUEST_BYTES {
        return receipt.fail(
            "request",
            vec![format!(
                "adapter request envelope exceeds {} bytes",
                protocol::MAX_REQUEST_BYTES
            )],
        );
    }
    let mut args = vec![
        descriptor
            .tooling_root
            .join(&descriptor.entrypoint)
            .into_os_string(),
    ];
    args.extend(descriptor.args.iter().map(OsString::from));
    let launch = process::Launch {
        executable: descriptor.executable.path.clone(),
        args,
        cwd: descriptor.tooling_root.clone(),
        tool_root: descriptor.tooling_root.clone(),
        stdin,
        timeout: Duration::from_secs(descriptor.limits.timeout_seconds),
        max_stream_bytes: descriptor.limits.max_stream_bytes,
    };
    let observation = match process::run(&launch, is_cancelled) {
        Ok(observation) => observation,
        Err(error) => {
            return receipt.fail("spawn", vec![format!("adapter could not start: {error}")]);
        }
    };
    let exit_code = match observation.ended {
        process::Ended::Exited(code) => code,
        _ => None,
    };
    receipt.process = Some(ProcessReceipt {
        ended: observation.ended.as_str(),
        exit_code,
        elapsed_ms: observation.elapsed.as_millis(),
        reaped: observation.reaped,
        group_stopped: observation.group_stopped,
        stdin_complete: observation.stdin_complete,
        stdout_bytes: observation.stdout.len(),
        stderr_bytes: observation.stderr.len(),
    });

    let mut after = Vec::new();
    for input in &inputs {
        // Re-resolve without following symlinks and open without blocking, so
        // an input swapped for a FIFO, device, or symlink fails promptly.
        match workspace_file(&workspace, &input.relative)
            .and_then(|path| digest_file(&path, MAX_INPUT_BYTES))
        {
            Ok(digest) if digest == input.sha256 => {}
            _ => after.push(format!(
                "input `{}` changed while the adapter ran",
                input.input_id
            )),
        }
    }
    if let Err(reason) = verify_tooling(descriptor) {
        after.push(format!(
            "operator tooling changed while the adapter ran: {reason}"
        ));
    }
    if !after.is_empty() {
        return receipt.fail("binding", after);
    }
    let exit_code = match (observation.ended, exit_code) {
        (process::Ended::Exited(_), Some(code @ (0 | 1))) => code,
        (process::Ended::Exited(_), Some(code)) => {
            return receipt.fail("process", vec![format!("adapter exited with code {code}")]);
        }
        (ended, _) => {
            return receipt.fail(
                "process",
                vec![format!("adapter run ended as {}", ended.as_str())],
            );
        }
    };
    if !observation.stdin_complete {
        return receipt.fail(
            "process",
            vec!["adapter did not take its whole request on stdin".to_string()],
        );
    }
    let result = match protocol::parse_result(&observation.stdout) {
        Ok(result) => result,
        Err(reason) => return receipt.fail("envelope", vec![reason]),
    };
    let (required, optional) = match row_sets(descriptor) {
        Ok(rows) => rows,
        Err(reason) => return receipt.fail("registry", vec![reason]),
    };
    let expectation = protocol::Expectation {
        request_id: request.request_id.clone(),
        nonce,
        input_digests: receipt.input_digests.clone(),
        required,
        optional,
    };
    let facts = match protocol::validate_result(&result, &expectation, exit_code) {
        Ok(facts) => facts,
        Err(violations) => return receipt.fail("envelope", violations),
    };
    receipt.stopped_at = "verified";
    receipt.facts = Some(facts);
    receipt.uncapped = facts.uncapped_verdict();
    receipt.domain = protocol::cap_domain(receipt.uncapped);
    receipt.native_passed = facts.native_passed();
    receipt.reasons = row_reasons(&result, &expectation);
    receipt
}

fn row_reasons(
    result: &protocol::AdapterResult,
    expectation: &protocol::Expectation,
) -> Vec<String> {
    result
        .checks
        .iter()
        .filter(|row| {
            let required = expectation
                .required
                .contains(&(row.check_id.clone(), row.target_id.clone()));
            row.status == protocol::RowStatus::Failed
                || (required && row.status != protocol::RowStatus::Passed)
        })
        .take(MAX_REASONS)
        .map(|row| {
            let target = row
                .target_id
                .as_deref()
                .map(|target| format!("/`{target}`"))
                .unwrap_or_default();
            let status = match row.status {
                protocol::RowStatus::Failed => "failed",
                protocol::RowStatus::Unresolved => "unresolved",
                protocol::RowStatus::NotRun => "not_run",
                protocol::RowStatus::NotProvided => "not_provided",
                protocol::RowStatus::Passed => "passed",
            };
            format!("row `{}`{target} is {status}", row.check_id)
        })
        .collect()
}

fn separate_from_workspace(admitted: &Admitted, workspace: &Path) -> Result<(), String> {
    let descriptor = &admitted.descriptor;
    for (label, path) in [
        ("operator registry", admitted.config_root.as_path()),
        ("adapter descriptor", admitted.descriptor_path.as_path()),
        ("tooling root", descriptor.tooling_root.as_path()),
        ("adapter executable", descriptor.executable.path.as_path()),
    ] {
        let resolved = fs::canonicalize(path)
            .map_err(|error| format!("{label} cannot be resolved: {error}"))?;
        if resolved.starts_with(workspace) {
            return Err(format!(
                "{label} resolves inside the workspace; a self-approved registry is refused"
            ));
        }
    }
    for (label, root) in [
        ("operator registry", admitted.config_root.as_path()),
        ("tooling root", descriptor.tooling_root.as_path()),
    ] {
        if workspace.starts_with(root) {
            return Err(format!("the workspace lies inside the {label}"));
        }
    }
    Ok(())
}

/// Verify the pinned executable, every pinned member, every allowed link, and
/// that the tooling tree holds no other symlink. Returns the executable hash.
fn verify_tooling(descriptor: &Descriptor) -> Result<String, String> {
    canonical_absolute(&descriptor.executable.path, "executable.path")?;
    let executable = digest_file(&descriptor.executable.path, MAX_TOOLING_FILE_BYTES)?;
    if executable != descriptor.executable.sha256 {
        return Err("executable does not match its operator pin".to_string());
    }
    let root = &descriptor.tooling_root;
    canonical_absolute(root, "tooling_root")?;
    for (link, target) in &descriptor.links {
        let metadata = fs::symlink_metadata(root.join(link))
            .map_err(|_| format!("allowed link `{link}` is missing"))?;
        let resolved = fs::canonicalize(root.join(link)).ok();
        let expected = fs::canonicalize(root.join(target)).ok();
        if !metadata.file_type().is_symlink()
            || resolved.is_none()
            || resolved != expected
            || !expected
                .as_deref()
                .is_some_and(|path| path.starts_with(root))
        {
            return Err(format!(
                "link `{link}` does not resolve to its expected package root"
            ));
        }
    }
    walk_tooling_tree(root, &descriptor.links)?;
    for (member, expected) in &descriptor.members {
        let path = root.join(member);
        let resolved =
            fs::canonicalize(&path).map_err(|_| format!("pinned member `{member}` is missing"))?;
        if !resolved.starts_with(root) || !resolved.is_file() {
            return Err(format!("pinned member `{member}` escapes the tooling root"));
        }
        if &digest_file(&resolved, MAX_TOOLING_FILE_BYTES)? != expected {
            return Err(format!("pinned member `{member}` does not match its pin"));
        }
    }
    Ok(executable)
}

fn walk_tooling_tree(root: &Path, links: &BTreeMap<String, String>) -> Result<(), String> {
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut seen = 0usize;
    while let Some((directory, depth)) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("tooling tree cannot be read: {error}"))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("tooling tree cannot be read: {error}"))?;
            seen += 1;
            if seen > MAX_TOOL_TREE_ENTRIES {
                return Err(format!(
                    "tooling tree exceeds {MAX_TOOL_TREE_ENTRIES} entries"
                ));
            }
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|error| format!("tooling tree cannot be read: {error}"))?;
            if file_type.is_symlink() {
                let relative = path
                    .strip_prefix(root)
                    .map(|relative| relative.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                if !links.contains_key(&relative) {
                    return Err(format!(
                        "tooling tree holds an unregistered symlink `{relative}`"
                    ));
                }
            } else if file_type.is_dir() {
                if depth >= MAX_TOOL_TREE_DEPTH {
                    return Err(format!(
                        "tooling tree is deeper than {MAX_TOOL_TREE_DEPTH} levels"
                    ));
                }
                pending.push((path, depth + 1));
            }
        }
    }
    Ok(())
}

fn read_request(workspace: &Path, relative: &str) -> Result<protocol::Request, String> {
    let path = workspace_file(workspace, relative)
        .map_err(|reason| format!("request `{relative}` {reason}"))?;
    let bytes = read_bounded(&path, protocol::MAX_REQUEST_BYTES as u64)
        .map_err(|reason| format!("request `{relative}` {reason}"))?;
    protocol::parse_request(&bytes)
}

#[derive(Debug, Clone)]
struct BoundInput {
    input_id: String,
    relative: String,
    sha256: String,
}

/// Bind the request inputs to the operator's input list. The request names
/// paths only; expected hashes come from the descriptor.
fn bind_inputs(
    workspace: &Path,
    descriptor: &Descriptor,
    request: &protocol::Request,
) -> Result<Vec<BoundInput>, String> {
    if !names_registered_inputs(descriptor, request) {
        return Err(
            "request inputs must name exactly the operator-registered input IDs".to_string(),
        );
    }
    let mut bound = Vec::new();
    for input in &request.inputs {
        let path = workspace_file(workspace, &input.path)
            .map_err(|reason| format!("input `{}` {reason}", input.input_id))?;
        let sha256 = digest_file(&path, MAX_INPUT_BYTES)?;
        let pin = descriptor
            .inputs
            .iter()
            .find(|pinned| pinned.input_id == input.input_id)
            .and_then(|pinned| pinned.sha256.as_deref());
        if pin.is_some_and(|pin| pin != sha256) {
            return Err(format!(
                "input `{}` does not match its operator pin",
                input.input_id
            ));
        }
        bound.push(BoundInput {
            input_id: input.input_id.clone(),
            relative: input.path.clone(),
            sha256,
        });
    }
    Ok(bound)
}

fn names_registered_inputs(descriptor: &Descriptor, request: &protocol::Request) -> bool {
    let declared = descriptor
        .inputs
        .iter()
        .map(|input| input.input_id.as_str())
        .collect::<BTreeSet<_>>();
    let requested = request
        .inputs
        .iter()
        .map(|input| input.input_id.as_str())
        .collect::<BTreeSet<_>>();
    declared == requested
}

/// Workspace names each admitted check last recorded (Issue #949), before
/// `bind_inputs`: the declared request, plus, when the request parses and
/// names exactly the operator's input IDs, every input that passes the
/// lexical guard, whether or not it then binds. Names are kept verbatim and
/// keyed by canonical workspace; see `final_acceptance::artifact_repair_paths`.
type BoundPaths = BTreeMap<PathBuf, BTreeMap<(String, String), Vec<String>>>;
static BOUND_PATHS: Mutex<BoundPaths> = Mutex::new(BTreeMap::new());
const MAX_REMEMBERED_WORKSPACES: usize = 64;

fn remember_bound_paths(
    workspace: &Path,
    check: &ArtifactContractCheck,
    descriptor: &Descriptor,
    request: Option<&protocol::Request>,
) {
    let mut paths = vec![check.request.clone()];
    if let Some(request) = request.filter(|request| names_registered_inputs(descriptor, request)) {
        paths.extend(
            request
                .inputs
                .iter()
                .map(|input| input.path.clone())
                .filter(|path| {
                    safe_relative(path, "path").is_ok()
                        && !crate::tools::sensitive_path::relative_path_is_sensitive(Path::new(
                            path,
                        ))
                }),
        );
    }
    let mut remembered = BOUND_PATHS.lock().unwrap_or_else(PoisonError::into_inner);
    if !remembered.contains_key(workspace) && remembered.len() >= MAX_REMEMBERED_WORKSPACES {
        remembered.pop_first();
    }
    let key = (check.adapter_id.clone(), check.request.clone());
    remembered
        .entry(workspace.to_path_buf())
        .or_default()
        .insert(key, paths);
}

/// The paths [`remember_bound_paths`] kept for `workspace`; empty when no
/// admitted check ran there.
pub(crate) fn bound_workspace_paths(workspace: &Path) -> Vec<String> {
    let Ok(workspace) = fs::canonicalize(workspace) else {
        return Vec::new();
    };
    let remembered = BOUND_PATHS.lock().unwrap_or_else(PoisonError::into_inner);
    remembered
        .get(&workspace)
        .into_iter()
        .flat_map(|checks| checks.values().flatten().cloned())
        .collect()
}

/// A safe workspace-relative regular file: no parent or absolute component,
/// no symlink on the way, and no credential target.
fn workspace_file(workspace: &Path, relative: &str) -> Result<PathBuf, String> {
    safe_relative(relative, "path")?;
    if crate::tools::sensitive_path::relative_path_is_sensitive(Path::new(relative)) {
        return Err("names a protected credential path".to_string());
    }
    let mut current = workspace.to_path_buf();
    for component in Path::new(relative).components() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|_| "is missing".to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("passes through a symlink".to_string());
        }
    }
    if !current.is_file() {
        return Err("is not a regular file".to_string());
    }
    Ok(current)
}

fn safe_relative(value: &str, label: &str) -> Result<(), String> {
    crate::tools::path_guard::validate_workspace_relative(value)
        .map_err(|error| format!("{label} `{value}` is invalid: {error}"))?;
    if Path::new(value)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("{label} `{value}` must use plain path components"));
    }
    Ok(())
}

fn canonical_absolute(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute"));
    }
    match fs::canonicalize(path) {
        Ok(resolved) if resolved == path => Ok(()),
        Ok(_) => Err(format!("{label} must be the operator-resolved realpath")),
        Err(error) => Err(format!("{label} cannot be resolved: {error}")),
    }
}

fn plain_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
}

/// Why a safe regular-file read failed. Shared by the adapter host, the
/// manifest and pin re-reads, and the pack loader, so every pre-provider
/// file read uses one implementation.
#[derive(Debug)]
pub(crate) enum SafeReadError {
    /// The open failed, including `ELOOP` for a final symlink.
    Open(std::io::Error),
    /// The opened descriptor's metadata could not be read.
    Inspect(std::io::Error),
    /// The opened descriptor is not a regular file (FIFO, device, ...).
    NotRegular,
    /// Reading failed after a successful open.
    Read(std::io::Error),
    /// More than the limit could be read.
    TooLarge(u64),
    /// The file fits its own limit but not the remaining shared budget.
    BudgetExceeded,
}

impl SafeReadError {
    /// Whether the open was refused because the final component is a symlink.
    pub(crate) fn is_symlink_refusal(&self) -> bool {
        #[cfg(unix)]
        {
            matches!(self, Self::Open(error) if error.raw_os_error() == Some(libc::ELOOP))
        }
        #[cfg(not(unix))]
        {
            false
        }
    }
}

impl std::fmt::Display for SafeReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open(error) => write!(formatter, "cannot be opened as a plain file: {error}"),
            Self::Inspect(error) => write!(formatter, "cannot be inspected: {error}"),
            Self::NotRegular => formatter.write_str("is not a regular file"),
            Self::Read(error) => write!(formatter, "cannot be read: {error}"),
            Self::TooLarge(limit) => write!(formatter, "exceeds {limit} bytes"),
            Self::BudgetExceeded => formatter.write_str("exceeds the remaining shared byte budget"),
        }
    }
}

/// Open a regular file for reading without following a final symlink and
/// without blocking on a FIFO or device, then confirm the opened descriptor is
/// a regular file. A component swapped between the caller's checks and this
/// open is still possible; the open itself never waits on a special file.
fn open_regular_file(path: &Path) -> Result<File, SafeReadError> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(SafeReadError::Open)?;
    let metadata = file.metadata().map_err(SafeReadError::Inspect)?;
    if !metadata.file_type().is_file() {
        return Err(SafeReadError::NotRegular);
    }
    Ok(file)
}

fn open_regular(path: &Path) -> Result<File, String> {
    open_regular_file(path).map_err(|error| error.to_string())
}

/// Read at most `limit` bytes of a regular file through [`open_regular_file`];
/// a file that yields more than `limit` bytes is refused, never truncated.
pub(crate) fn read_regular_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, SafeReadError> {
    let file = open_regular_file(path)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(SafeReadError::Read)?;
    if bytes.len() as u64 > limit {
        return Err(SafeReadError::TooLarge(limit));
    }
    Ok(bytes)
}

/// Read one of several files that share a byte `budget`: at most
/// `min(file_limit, budget)` bytes, measured on the bytes actually read. A file
/// over `file_limit` is `TooLarge`; a file within `file_limit` that does not
/// fit the remaining budget is `BudgetExceeded`. On success the bytes read are
/// taken from the budget.
pub(crate) fn read_regular_within_budget(
    path: &Path,
    file_limit: u64,
    budget: &mut u64,
) -> Result<Vec<u8>, SafeReadError> {
    let limit = file_limit.min(*budget);
    let bytes = match read_regular_bounded(path, limit) {
        Err(SafeReadError::TooLarge(_)) if limit < file_limit => {
            // Over the budget; it is over the file's own limit only if it
            // also yields more than `file_limit` bytes.
            return Err(match read_regular_bounded(path, file_limit) {
                Err(SafeReadError::TooLarge(limit)) => SafeReadError::TooLarge(limit),
                _ => SafeReadError::BudgetExceeded,
            });
        }
        other => other?,
    };
    *budget -= bytes.len() as u64;
    Ok(bytes)
}

/// [`read_regular_bounded`] with a message, for host modules that must read a
/// regular file before any provider is built.
pub(crate) fn read_regular_file(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    read_bounded(path, limit)
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    read_regular_bounded(path, limit).map_err(|error| error.to_string())
}

fn digest_file(path: &Path, limit: u64) -> Result<String, String> {
    let mut file = open_regular(path).map_err(|reason| format!("a hashed file {reason}"))?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|error| format!("cannot hash a file: {error}"))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > limit {
            return Err(format!("a hashed file exceeds {limit} bytes"));
        }
        hasher.update(&chunk[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn record(receipt: &Receipt, events_path: Option<&Path>, source: &str, owner_id: &str) {
    let facts = receipt.facts.unwrap_or_default();
    let process = receipt.process.as_ref().map(|process| {
        json!({
            "ended": process.ended,
            "exit_code": process.exit_code,
            "elapsed_ms": process.elapsed_ms,
            "reaped": process.reaped,
            "group_stopped": process.group_stopped,
            "stdin_complete": process.stdin_complete,
            "stdout_bytes": process.stdout_bytes,
            "stderr_bytes": process.stderr_bytes,
        })
    });
    crate::eval_events::emit(
        events_path,
        json!({
            "event": "artifact_contract_check_result",
            "source": source,
            "owner_id": owner_id,
            "check_id": crate::planner::capability_catalog::artifact::ID,
            "adapter_id": receipt.adapter_id,
            "request": receipt.request,
            "request_id": receipt.request_id,
            "status": if receipt.native_passed { "passed" } else { "failed" },
            "stopped_at": receipt.stopped_at,
            "descriptor_sha256": receipt.descriptor_sha256,
            "executable_sha256": receipt.executable_sha256,
            "input_digests": receipt.input_digests,
            "process": process,
            "child_env_keys": process::CHILD_ENV_KEYS,
            "platform_env_keys": process::KNOWN_PLATFORM_ENV_KEYS,
            "network_isolation": "not_provided",
            "rows": facts.rows,
            "required_rows_passed": facts.required_passed,
            "known_mismatch": facts.known_mismatch,
            "required_unresolved": facts.required_unresolved,
            "required_not_run": facts.required_not_run,
            "optional_unverified": facts.optional_unverified,
            "optional_missing": facts.optional_missing,
            "domain_verdict": receipt.domain.as_str(),
            "domain_cap": protocol::DOMAIN_CAP.as_str(),
            "domain_cap_applied": receipt.domain != receipt.uncapped,
            "native_effect": "failure_only",
            "reasons": receipt
                .reasons
                .iter()
                .map(|reason| crate::eval_events::body_snippet(reason))
                .collect::<Vec<_>>(),
        }),
    );
    let mut line = format!(
        "Artifact contract checks ({source} `{owner_id}`): adapter `{}` {}; domain {} (cap {})",
        receipt.adapter_id,
        if receipt.native_passed {
            "passed"
        } else {
            "failed"
        },
        receipt.domain.as_str(),
        protocol::DOMAIN_CAP.as_str()
    );
    if let Some(reason) = receipt.failures().first() {
        line.push_str(" — ");
        line.push_str(reason);
    }
    crate::eval_events::append_run_summary(events_path, &line);
}
