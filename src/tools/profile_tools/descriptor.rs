//! The operator's tool registry: one descriptor per opaque tool ID.
//!
//! The registry root is the operator root that `artifact_contract_checks` reads
//! from the process environment, but the descriptors live in their own directory
//! under their own protocol: the adapter protocol is neither reused nor widened.
//! Every path the descriptor resolves to must lie outside the workspace, so a
//! descriptor a model wrote is refused whatever it says about itself.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ProfileToolRefusal, RefusalKind};
use crate::planner::artifact_contract_checks::protocol;
use crate::planner::artifact_contract_checks::{
    CONFIG_ROOT_ENV, config_root_from_environment, read_regular_file,
};
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
    pub schema_sha256: String,
    pub runtime: Runtime,
    #[serde(default)]
    pub dependencies: Vec<DependencyPin>,
    #[serde(default)]
    pub requires: Vec<String>,
    pub limits: Limits,
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
/// was admitted from recorded as a digest.
#[derive(Debug, Clone)]
pub(super) struct LoadedDescriptor {
    pub(super) descriptor: Descriptor,
    pub(super) sha256: String,
    pub(super) path: PathBuf,
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
    let bytes = read_regular_file(&path, MAX_DESCRIPTOR_BYTES).map_err(|reason| {
        refusal(
            RefusalKind::InvalidDescriptor,
            format!("tool `{tool_id}` descriptor {reason}"),
        )
    })?;
    protocol::check_json_depth(&bytes)
        .map_err(|reason| refusal(RefusalKind::InvalidDescriptor, reason))?;
    let descriptor: Descriptor = serde_json::from_slice(&bytes).map_err(|error| {
        refusal(
            RefusalKind::InvalidDescriptor,
            format!("tool `{tool_id}` descriptor is invalid: {error}"),
        )
    })?;
    validate(&descriptor, tool_id)?;
    verify_dependencies(&descriptor, workspace)?;
    Ok(LoadedDescriptor {
        descriptor,
        sha256: sha256_hex(&bytes),
        path,
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
    if schema_pin(&descriptor.parameters) != descriptor.schema_sha256 {
        return Err(refusal(
            RefusalKind::InvalidPin,
            format!("tool `{tool_id}` schema does not match schema_sha256"),
        ));
    }
    Ok(())
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
/// before an execution.
pub(super) fn verify_dependencies(
    descriptor: &Descriptor,
    workspace: &Path,
) -> Result<(), ProfileToolRefusal> {
    let tool = &descriptor.tool_id;
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
        let bytes = read_regular_file(&canonical, MAX_DEPENDENCY_BYTES).map_err(|reason| {
            refusal(
                RefusalKind::InvalidPin,
                format!("tool `{tool}` dependency {reason}"),
            )
        })?;
        if sha256_hex(&bytes) != dependency.sha256 {
            return Err(refusal(
                RefusalKind::InvalidPin,
                format!("tool `{tool}` dependency does not match its pin"),
            ));
        }
    }
    Ok(())
}

/// Compare the descriptor file with the digest recorded at admission. A file
/// replaced after admission, including by a symlink, is refused.
pub(super) fn verify_descriptor_pin(
    path: &Path,
    expected_sha256: &str,
) -> Result<(), ProfileToolRefusal> {
    let bytes = read_regular_file(path, MAX_DESCRIPTOR_BYTES).map_err(|reason| {
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
