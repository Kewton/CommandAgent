//! The operator's tool registry: one descriptor per opaque tool ID.
//!
//! The registry root is the operator root that `artifact_contract_checks` reads
//! from the process environment, but the descriptors live in their own directory
//! under their own protocol: the adapter protocol is neither reused nor widened.
//! Every path the descriptor resolves to must lie outside the workspace, so a
//! descriptor a model wrote is refused whatever it says about itself.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::effects::{EFFECT_PROPOSE_ARTIFACTS, RUNTIME_PINNED_PROCESS};
use super::protocol::ProcessDeclaration;
use super::schema::admission;
use super::{ProfileToolRefusal, RefusalKind};
use crate::planner::artifact_contract_checks::protocol;
use crate::planner::artifact_contract_checks::{CONFIG_ROOT_ENV, config_root_from_environment};
use crate::tools::extension::{ExtensionArgument, ExtensionArgumentKind};

pub const DESCRIPTOR_PROTOCOL: &str = "profile-tool.descriptor/v1";
pub const DESCRIPTOR_DIRECTORY: &str = "profile-tools";
pub const EFFECT_READ: &str = "read";
pub const RUNTIME_IN_PROCESS: &str = "in_process";
const MAX_DESCRIPTOR_BYTES: u64 = 1 << 20;
const MAX_DEPENDENCY_BYTES: u64 = 1 << 30;
const MAX_DEPENDENCIES: usize = 256;
const MAX_PARAMETERS: usize = 32;
const MAX_INPUT_BYTES: usize = 1 << 20;

/// An operator descriptor. Only this generic schema interprets its values.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub protocol: String,
    pub tool_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub effect: String,
    pub parameters: Vec<Parameter>,
    /// The G2 nested schema declaration. With it, `parameters` must be empty and
    /// `schema_sha256` pins this value instead.
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
    pub schema_sha256: String,
    pub runtime: Runtime,
    #[serde(default)]
    pub dependencies: Vec<DependencyPin>,
    #[serde(default)]
    pub requires: Vec<String>,
    pub limits: Limits,
    /// The G3 pinned process of a `propose-artifacts` tool. The whole value is part of
    /// the descriptor's bytes, so its digest pins this too.
    #[serde(default)]
    pub process: Option<ProcessDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    pub kind: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyPin {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
}

/// A descriptor admitted from the operator registry, with the exact bytes it
/// was admitted from recorded as a digest, and the directories it was admitted
/// from recorded by identity.
#[derive(Debug, Clone)]
pub(super) struct LoadedDescriptor {
    pub(super) descriptor: Descriptor,
    pub(super) sha256: String,
    pub(super) registry: RegistryBinding,
    pub(super) dependency_directories: Vec<DirectoryIdentity>,
}

/// A directory's device and inode, read from a descriptor opened on it without
/// following a symlink. The same path holding other bytes-equal content differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DirectoryIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
fn directory_identity(path: &Path) -> Result<DirectoryIdentity, String> {
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
    let metadata = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .and_then(|directory| directory.metadata())
        .map_err(|_| "is not a directory reached without a symlink".to_string())?;
    Ok(DirectoryIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

/// Nothing here can name a directory without following a link, so a run off Unix
/// is refused rather than left unbound.
#[cfg(not(unix))]
fn directory_identity(_path: &Path) -> Result<DirectoryIdentity, String> {
    Err("cannot be bound to a directory identity on this platform".to_string())
}

/// Where a descriptor was admitted from. The walk in [`crate::tools::dir_fd`]
/// refuses every symlink below the root, and the identities refuse a root or
/// registry directory that was replaced by another with the same bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RegistryBinding {
    root: PathBuf,
    root_identity: DirectoryIdentity,
    directory_identity: DirectoryIdentity,
    relative: PathBuf,
}

impl RegistryBinding {
    /// For a unit test that holds a real registry directory.
    #[cfg(test)]
    pub(super) fn capture_for_test(root: &Path, tool_id: &str) -> Self {
        Self::capture(root, tool_id).unwrap()
    }

    /// For a test of the parsing path, which reaches no registry.
    #[cfg(test)]
    pub(super) fn unreachable_for_test() -> Self {
        let identity = DirectoryIdentity {
            device: 0,
            inode: 0,
        };
        Self {
            root: PathBuf::new(),
            root_identity: identity,
            directory_identity: identity,
            relative: PathBuf::new(),
        }
    }

    fn capture(root: &Path, tool_id: &str) -> Result<Self, String> {
        Ok(Self {
            root: root.to_path_buf(),
            root_identity: directory_identity(root)?,
            directory_identity: directory_identity(&root.join(DESCRIPTOR_DIRECTORY))?,
            relative: Path::new(DESCRIPTOR_DIRECTORY).join(format!("{tool_id}.json")),
        })
    }

    fn check(&self) -> Result<(), String> {
        let holds = |path: &Path, admitted: DirectoryIdentity| {
            directory_identity(path).is_ok_and(|identity| identity == admitted)
        };
        if holds(&self.root, self.root_identity)
            && holds(
                &self.root.join(DESCRIPTOR_DIRECTORY),
                self.directory_identity,
            )
        {
            Ok(())
        } else {
            Err("is no longer reached from the directories it was admitted from".to_string())
        }
    }

    /// The identities hold on both sides of the walk, so the walked directories
    /// are the admitted ones even though `dir_fd` keeps its own descriptors.
    fn read(&self) -> Result<Vec<u8>, String> {
        self.check()?;
        let bytes = read_without_following(&self.root, &self.relative, MAX_DESCRIPTOR_BYTES)?;
        self.check()?;
        Ok(bytes)
    }

    /// G3: the identities are the ones the walk itself held while it read, so a directory
    /// swapped during the walk and put back is seen. `read` above stays as G1 and G2 use it.
    #[cfg(unix)]
    fn read_held(&self) -> Result<Vec<u8>, String> {
        let (file, held) = crate::tools::dir_fd::open_read_file_held(&self.root, &self.relative)
            .map_err(|_| "cannot be opened as a regular file without a symlink".to_string())?;
        if held.root != (self.root_identity.device, self.root_identity.inode)
            || held.parent
                != (
                    self.directory_identity.device,
                    self.directory_identity.inode,
                )
        {
            return Err(
                "is no longer reached from the directories it was admitted from".to_string(),
            );
        }
        read_bounded(file, MAX_DESCRIPTOR_BYTES)
    }

    #[cfg(not(unix))]
    fn read_held(&self) -> Result<Vec<u8>, String> {
        Err("cannot be read through a held directory on this platform".to_string())
    }
}

fn read_bounded(file: fs::File, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot be read".to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("exceeds {limit} bytes"));
    }
    Ok(bytes)
}

/// Read a regular file through the no-follow walk, with the bound applied to the
/// bytes read. The walk's own message names host paths, so it is not repeated.
pub(super) fn read_without_following(
    root: &Path,
    path: &Path,
    limit: u64,
) -> Result<Vec<u8>, String> {
    let file = crate::tools::dir_fd::open_read_file(root, path)
        .map_err(|_| "cannot be opened as a regular file without a symlink".to_string())?;
    read_bounded(file, limit)
}

fn refusal(kind: RefusalKind, reason: impl Into<String>) -> ProfileToolRefusal {
    ProfileToolRefusal::new(kind, reason)
}

/// Resolve one opaque ID. A missing root, an unknown ID, a symlink, a path
/// inside the workspace, and any descriptor or pin that does not validate are
/// refusals.
pub(super) fn load(
    tool_id: &str,
    workspace: &Path,
) -> Result<LoadedDescriptor, ProfileToolRefusal> {
    let root = config_root_from_environment().ok_or_else(|| {
        refusal(
            RefusalKind::RegistryUnavailable,
            format!(
                "{CONFIG_ROOT_ENV} is not set; the operator registry must be configured before the run"
            ),
        )
    })?;
    if !root.is_absolute() {
        return Err(refusal(
            RefusalKind::RegistryUnavailable,
            format!("{CONFIG_ROOT_ENV} must be an absolute path"),
        ));
    }
    let root = fs::canonicalize(&root).map_err(|error| {
        refusal(
            RefusalKind::RegistryUnavailable,
            format!("{CONFIG_ROOT_ENV} cannot be resolved: {error}"),
        )
    })?;
    ensure_outside_workspace(&root, workspace, "operator registry")?;
    let directory = root.join(DESCRIPTOR_DIRECTORY);
    let plain = fs::symlink_metadata(&directory).is_ok_and(|meta| meta.file_type().is_dir());
    if !plain {
        return Err(refusal(
            RefusalKind::UnknownToolId,
            format!(
                "tool `{tool_id}` is not registered: `{DESCRIPTOR_DIRECTORY}/` is missing or is a symlink"
            ),
        ));
    }
    let path = directory.join(format!("{tool_id}.json"));
    let metadata = fs::symlink_metadata(&path).map_err(|_| {
        refusal(
            RefusalKind::UnknownToolId,
            format!("tool `{tool_id}` is not registered by the operator"),
        )
    })?;
    if !metadata.file_type().is_file() {
        return Err(refusal(
            RefusalKind::WorkspaceDescriptor,
            format!("tool `{tool_id}` descriptor must be a regular file, not a symlink"),
        ));
    }
    let registry = RegistryBinding::capture(&root, tool_id).map_err(|reason| {
        refusal(
            RefusalKind::RegistryUnavailable,
            format!("{CONFIG_ROOT_ENV} {reason}"),
        )
    })?;
    let bytes = registry.read().map_err(|reason| {
        refusal(
            RefusalKind::InvalidDescriptor,
            format!("tool `{tool_id}` descriptor {reason}"),
        )
    })?;
    // The v1 depth limit stays as it is. Only a G2 schema is read deeper, through
    // its own bounded decode.
    if !admission::descriptor_carries_schema(&bytes, MAX_DESCRIPTOR_BYTES as usize) {
        protocol::check_json_depth(&bytes)
            .map_err(|reason| refusal(RefusalKind::InvalidDescriptor, reason))?;
    }
    let mut descriptor: Descriptor = serde_json::from_slice(&bytes).map_err(|error| {
        refusal(
            RefusalKind::InvalidDescriptor,
            format!("tool `{tool_id}` descriptor is invalid: {error}"),
        )
    })?;
    if descriptor.schema.is_some() {
        descriptor.schema = strict_schema(&bytes, tool_id)?;
    }
    validate(&descriptor, tool_id)?;
    let dependency_directories = verify_dependencies(&descriptor, workspace)?;
    Ok(LoadedDescriptor {
        descriptor,
        sha256: sha256_hex(&bytes),
        registry,
        dependency_directories,
    })
}

/// `schema` as the strict decode reads it. `serde_json` would keep the last of a repeated
/// key, and can land a bound one step off, so the text and the value in force could
/// disagree. `serde` also reads an array of the fields, where no key names `schema`, so
/// anything but an object is refused.
fn strict_schema(bytes: &[u8], tool_id: &str) -> Result<Option<Value>, ProfileToolRefusal> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| admission::decode_descriptor(text, MAX_DESCRIPTOR_BYTES as usize).ok())
        .and_then(|document| document.as_object().map(|fields| fields.get("schema").cloned()))
        .ok_or_else(|| {
            refusal(
                RefusalKind::InvalidDescriptor,
                format!(
                    "tool `{tool_id}` descriptor repeats a key, is not strict JSON, or is not an object"
                ),
            )
        })
}

fn validate(descriptor: &Descriptor, tool_id: &str) -> Result<(), ProfileToolRefusal> {
    let invalid = |reason: String| {
        refusal(
            RefusalKind::InvalidDescriptor,
            format!("tool `{tool_id}` descriptor is invalid: {reason}"),
        )
    };
    if descriptor.protocol != DESCRIPTOR_PROTOCOL {
        return Err(invalid(format!("protocol must be `{DESCRIPTOR_PROTOCOL}`")));
    }
    if descriptor.tool_id != tool_id {
        return Err(invalid(
            "tool_id differs from its registry file name".to_string(),
        ));
    }
    if descriptor.version.trim().is_empty() {
        return Err(invalid("version must not be empty".to_string()));
    }
    if descriptor.parameters.len() > MAX_PARAMETERS {
        return Err(invalid(format!(
            "at most {MAX_PARAMETERS} parameters may be declared"
        )));
    }
    if descriptor.dependencies.len() > MAX_DEPENDENCIES {
        return Err(invalid(format!(
            "at most {MAX_DEPENDENCIES} dependencies may be pinned"
        )));
    }
    if descriptor.process.is_some()
        && (descriptor.effect != EFFECT_PROPOSE_ARTIFACTS
            || descriptor.runtime.kind != RUNTIME_PINNED_PROCESS)
    {
        return Err(invalid(format!(
            "`process` belongs to effect `{EFFECT_PROPOSE_ARTIFACTS}` with runtime `{RUNTIME_PINNED_PROCESS}`"
        )));
    }
    let limits = descriptor.limits;
    if !(1..=MAX_INPUT_BYTES).contains(&limits.max_input_bytes)
        || !(1..=crate::tools::extension::MAX_EXTENSION_RESULT_BYTES)
            .contains(&limits.max_output_bytes)
    {
        return Err(invalid(format!(
            "limits must stay within 1..{MAX_INPUT_BYTES} input bytes and 1..{} output bytes",
            crate::tools::extension::MAX_EXTENSION_RESULT_BYTES
        )));
    }
    protocol::validate_sha256("schema_sha256", &descriptor.schema_sha256)
        .map_err(|reason| refusal(RefusalKind::InvalidPin, reason))?;
    if declared_pin(descriptor, tool_id)? != descriptor.schema_sha256 {
        return Err(refusal(
            RefusalKind::InvalidPin,
            format!("tool `{tool_id}` schema does not match schema_sha256"),
        ));
    }
    Ok(())
}

fn declared_pin(descriptor: &Descriptor, tool_id: &str) -> Result<String, ProfileToolRefusal> {
    let Some(schema) = &descriptor.schema else {
        return Ok(schema_pin(&descriptor.parameters));
    };
    if !descriptor.parameters.is_empty() {
        return Err(refusal(
            RefusalKind::InvalidDescriptor,
            format!(
                "tool `{tool_id}` descriptor is invalid: `parameters` must be empty next to `schema`"
            ),
        ));
    }
    Ok(sha256_hex(&serde_json::to_vec(schema).unwrap_or_default()))
}

/// The digest `schema_sha256` pins: the compact JSON of the parameter list, its
/// fields in declaration order.
pub fn schema_pin(parameters: &[Parameter]) -> String {
    let bytes = serde_json::to_vec(parameters).unwrap_or_default();
    sha256_hex(&bytes)
}

/// The closed argument types of the existing extension API. A nested type waits
/// for G2 and is unavailable, not silently flattened.
pub(super) fn closed_arguments(
    descriptor: &Descriptor,
) -> Result<Vec<ExtensionArgument>, ProfileToolRefusal> {
    descriptor
        .parameters
        .iter()
        .map(|parameter| {
            let kind = match parameter.kind.as_str() {
                "string" => ExtensionArgumentKind::String,
                "integer" => ExtensionArgumentKind::Integer,
                "boolean" => ExtensionArgumentKind::Boolean,
                "workspace_path" => ExtensionArgumentKind::WorkspacePath,
                "object" | "array" => {
                    return Err(refusal(
                        RefusalKind::Unavailable,
                        format!(
                            "tool `{}` parameter `{}` needs a nested schema, which is not implemented yet",
                            descriptor.tool_id, parameter.name
                        ),
                    ));
                }
                other => {
                    return Err(refusal(
                        RefusalKind::InvalidDescriptor,
                        format!(
                            "tool `{}` parameter `{}` has unknown kind `{other}`",
                            descriptor.tool_id, parameter.name
                        ),
                    ));
                }
            };
            Ok(if parameter.required {
                ExtensionArgument::required(&parameter.name, kind, &parameter.description)
            } else {
                ExtensionArgument::optional(&parameter.name, kind, &parameter.description)
            })
        })
        .collect()
}

/// Compare every dependency with its pin. Used at admission and again right
/// before an execution. Returns the identity of each dependency's directory, in
/// order, for the execution to compare with the admission's.
pub(super) fn verify_dependencies(
    descriptor: &Descriptor,
    workspace: &Path,
) -> Result<Vec<DirectoryIdentity>, ProfileToolRefusal> {
    let tool = &descriptor.tool_id;
    let mut directories = Vec::with_capacity(descriptor.dependencies.len());
    for dependency in &descriptor.dependencies {
        protocol::validate_sha256("dependency sha256", &dependency.sha256)
            .map_err(|reason| refusal(RefusalKind::InvalidPin, reason))?;
        if !dependency.path.is_absolute() {
            return Err(refusal(
                RefusalKind::InvalidPin,
                format!("tool `{tool}` dependency path must be absolute"),
            ));
        }
        let canonical = fs::canonicalize(&dependency.path).map_err(|_| {
            refusal(
                RefusalKind::InvalidPin,
                format!("tool `{tool}` dependency cannot be resolved"),
            )
        })?;
        ensure_outside_workspace(&canonical, workspace, "dependency")?;
        if canonical != dependency.path {
            return Err(refusal(
                RefusalKind::InvalidPin,
                format!("tool `{tool}` dependency path must be canonical, without a symlink"),
            ));
        }
        let invalid = |reason: String| {
            refusal(
                RefusalKind::InvalidPin,
                format!("tool `{tool}` dependency {reason}"),
            )
        };
        let bytes = read_dependency(&canonical).map_err(invalid)?;
        if sha256_hex(&bytes.0) != dependency.sha256 {
            return Err(refusal(
                RefusalKind::InvalidPin,
                format!("tool `{tool}` dependency does not match its pin"),
            ));
        }
        directories.push(bytes.1);
    }
    Ok(directories)
}

/// A canonical path has no symlink to follow, so the walk starts at the file
/// system root. The directory's identity holds on both sides of the walk.
fn read_dependency(canonical: &Path) -> Result<(Vec<u8>, DirectoryIdentity), String> {
    let directory = canonical
        .parent()
        .ok_or_else(|| "has no parent directory".to_string())?;
    let identity = directory_identity(directory)?;
    let bytes = read_without_following(Path::new("/"), canonical, MAX_DEPENDENCY_BYTES)?;
    if directory_identity(directory)? != identity {
        return Err("directory changed while it was read".to_string());
    }
    Ok((bytes, identity))
}

/// Compare the descriptor with the digest recorded at admission, reached from the
/// admitted root. A file replaced after admission, a symlink anywhere below the
/// root, and a root or directory replaced by another are refused.
pub(super) fn verify_descriptor_pin(
    registry: &RegistryBinding,
    expected_sha256: &str,
) -> Result<(), ProfileToolRefusal> {
    let bytes = registry.read().map_err(|reason| {
        refusal(
            RefusalKind::InvalidPin,
            format!("admitted descriptor {reason}"),
        )
    })?;
    if sha256_hex(&bytes) != expected_sha256 {
        return Err(refusal(
            RefusalKind::InvalidPin,
            "admitted descriptor changed since admission",
        ));
    }
    Ok(())
}

/// The G3 form of [`verify_descriptor_pin`], compared on the descriptors the walk held.
pub(super) fn verify_descriptor_pin_held(
    registry: &RegistryBinding,
    expected_sha256: &str,
) -> Result<(), ProfileToolRefusal> {
    let bytes = registry.read_held().map_err(|reason| {
        refusal(
            RefusalKind::InvalidPin,
            format!("admitted descriptor {reason}"),
        )
    })?;
    if sha256_hex(&bytes) != expected_sha256 {
        return Err(refusal(
            RefusalKind::InvalidPin,
            "admitted descriptor changed since admission",
        ));
    }
    Ok(())
}

const MAX_TREE_ENTRIES: usize = 200_000;
const MAX_TREE_BYTES: u64 = 4 << 30;

/// The identities behind one set of process pins, compared with the admission's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProcessPins {
    directories: Vec<DirectoryIdentity>,
}

/// The digest a `dependency_directories` or `link_directories` entry pins: for each entry
/// below the directory, sorted by relative path, a type letter, the relative path, and the
/// link target or the file's own digest. A symlink is recorded and never followed, and
/// anything but a file, directory, or symlink is refused.
pub fn directory_pin(directory: &Path) -> Result<String, String> {
    let mut lines = Vec::new();
    let mut budget = (MAX_TREE_ENTRIES, MAX_TREE_BYTES);
    collect_tree(directory, Path::new(""), &mut lines, &mut budget)?;
    lines.sort();
    let mut digest = Sha256::new();
    for line in lines {
        digest.update(line.as_bytes());
        digest.update([b'\n']);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn collect_tree(
    root: &Path,
    relative: &Path,
    lines: &mut Vec<String>,
    budget: &mut (usize, u64),
) -> Result<(), String> {
    let entries = fs::read_dir(root.join(relative)).map_err(|_| "cannot be listed".to_string())?;
    for entry in entries {
        let entry = entry.map_err(|_| "cannot be listed".to_string())?;
        budget.0 = budget
            .0
            .checked_sub(1)
            .ok_or_else(|| format!("holds more than {MAX_TREE_ENTRIES} entries"))?;
        let path = relative.join(entry.file_name());
        let shown = path.to_string_lossy().into_owned();
        let kind = entry
            .file_type()
            .map_err(|_| "cannot be inspected".to_string())?;
        if kind.is_symlink() {
            let target =
                fs::read_link(root.join(&path)).map_err(|_| "cannot be read".to_string())?;
            lines.push(format!("l\0{shown}\0{}", target.to_string_lossy()));
        } else if kind.is_dir() {
            lines.push(format!("d\0{shown}"));
            collect_tree(root, &path, lines, budget)?;
        } else if kind.is_file() {
            let bytes = read_without_following(root, &path, budget.1)?;
            budget.1 = budget.1.saturating_sub(bytes.len() as u64);
            lines.push(format!("f\0{shown}\0{}", sha256_hex(&bytes)));
        } else {
            return Err("holds something other than a file, directory, or symlink".to_string());
        }
    }
    Ok(())
}

/// The executable, each pinned directory, and the working directory: canonical, outside
/// the workspace, and holding what the descriptor pins. Used at admission, right before the
/// process, and right before the commit.
pub(super) fn verify_process_pins(
    declaration: &ProcessDeclaration,
    tool: &str,
    workspace: &Path,
) -> Result<ProcessPins, ProfileToolRefusal> {
    let invalid = |reason: String| {
        refusal(
            RefusalKind::InvalidPin,
            format!("tool `{tool}` process {reason}"),
        )
    };
    let place = |path: &Path, label: &str| -> Result<(), ProfileToolRefusal> {
        let canonical =
            fs::canonicalize(path).map_err(|_| invalid(format!("{label} cannot be resolved")))?;
        if canonical != path {
            return Err(invalid(format!(
                "{label} path must be canonical, without a symlink"
            )));
        }
        ensure_outside_workspace(&canonical, workspace, label)
    };
    let executable = &declaration.executable;
    place(&executable.path, "executable")?;
    let (bytes, parent) = read_dependency(&executable.path)
        .map_err(|reason| invalid(format!("executable {reason}")))?;
    if sha256_hex(&bytes) != executable.sha256 {
        return Err(invalid("executable does not match its pin".to_string()));
    }
    let mut directories = vec![parent];
    for pin in declaration
        .dependency_directories
        .iter()
        .chain(&declaration.link_directories)
    {
        place(&pin.path, "directory")?;
        directories.push(
            directory_identity(&pin.path)
                .map_err(|reason| invalid(format!("directory {reason}")))?,
        );
        let digest =
            directory_pin(&pin.path).map_err(|reason| invalid(format!("directory {reason}")))?;
        if digest != pin.sha256 {
            return Err(invalid("directory does not match its pin".to_string()));
        }
    }
    place(&declaration.cwd, "cwd")?;
    Ok(ProcessPins { directories })
}

fn ensure_outside_workspace(
    path: &Path,
    workspace: &Path,
    label: &str,
) -> Result<(), ProfileToolRefusal> {
    if path.starts_with(workspace) || workspace.starts_with(path) {
        return Err(refusal(
            RefusalKind::WorkspaceDescriptor,
            format!("{label} must lie outside the workspace"),
        ));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
