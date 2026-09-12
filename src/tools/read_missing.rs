//! Classify only an ordinary, policy-allowed missing Read target. This is an
//! error classification guard, not an atomic/openat replacement for Read.
use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use super::path_guard::{strip_redundant_root_prefix, validate_workspace_relative};
use super::registry::ToolContext;
use super::workspace_policy::ensure_tool_path_allowed;

#[derive(Debug, thiserror::Error)]
#[error("read_path_missing: path does not exist: {relative}; no content was read")]
pub(crate) struct MissingRead {
    pub(crate) path: PathBuf,
    relative: String,
    #[source]
    source: anyhow::Error,
}

pub(crate) fn from_error(error: &anyhow::Error) -> Option<&MissingRead> {
    error.downcast_ref()
}

/// Capture before lookup so a failure after lookup can also be checked against
/// the original root/ancestors. Absolute/salvaged requests retain legacy errors:
/// normalization must not erase evidence of a symlink for this new allowance.
pub(crate) struct ReadFailureContext {
    proof: Option<Proof>,
}

struct Proof {
    requested_root: PathBuf,
    root: PathBuf,
    path: PathBuf,
    relative: String,
    ancestors: Vec<(PathBuf, Metadata)>,
}

impl ReadFailureContext {
    pub(crate) fn capture(context: &ToolContext, raw: &str) -> Self {
        Self {
            proof: Proof::capture(context, raw),
        }
    }

    pub(crate) fn classify_lookup(&self, error: anyhow::Error, normalized: &str) -> anyhow::Error {
        let Some(proof) = &self.proof else {
            return error;
        };
        let selected = proof
            .root
            .join(strip_redundant_root_prefix(&proof.root, normalized));
        self.classify(error, Some(&selected))
    }

    pub(crate) fn classify(&self, error: anyhow::Error, resolved: Option<&Path>) -> anyhow::Error {
        let Some(proof) = &self.proof else {
            return error;
        };
        // Preserve the existing workdir-prefix typo hint rather than suggesting
        // that the model create a second copy of an existing required file.
        if error.to_string().starts_with("path_not_found_recoverable:")
            || error
                .downcast_ref::<std::io::Error>()
                .is_none_or(|error| error.kind() != ErrorKind::NotFound)
            || resolved.is_some_and(|path| path != proof.path)
            || !proof.still_ordinary_missing()
        {
            return error;
        }
        MissingRead {
            path: proof.path.clone(),
            relative: proof.relative.clone(),
            source: error,
        }
        .into()
    }
}

impl Proof {
    fn capture(context: &ToolContext, raw: &str) -> Option<Self> {
        validate_workspace_relative(raw).ok()?;
        if super::path_guard::normalize_workspace_path(&context.root, raw)
            .ok()?
            .is_some()
        {
            return None;
        }
        let root = context.root.canonicalize().ok()?;
        let relative: PathBuf = strip_redundant_root_prefix(&root, raw)
            .components()
            .filter(|component| !matches!(component, Component::CurDir))
            .collect();
        if relative.as_os_str().is_empty() {
            return None;
        }
        let path = root.join(&relative);
        ensure_tool_path_allowed(&root, &root.join(raw), context.workspace_policy).ok()?;
        ensure_tool_path_allowed(&root, &path, context.workspace_policy).ok()?;
        if [raw, relative.to_str()?].iter().any(|path| {
            crate::minimal_loop::protected_paths::matching_path(
                &root,
                path,
                &context.protected_paths,
            )
            .is_some()
        }) {
            return None;
        }
        let root_metadata = std::fs::symlink_metadata(&root).ok()?;
        if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
            return None;
        }
        let mut ancestors = vec![(root.clone(), root_metadata)];
        let mut current = root.clone();
        let mut components = relative.components().peekable();
        while let Some(component) = components.next() {
            current.push(component);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => return None,
                Ok(metadata) if components.peek().is_some() => {
                    if !metadata.is_dir() {
                        return None;
                    }
                    ancestors.push((current.clone(), metadata));
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => break,
                Err(_) => return None,
            }
        }
        Some(Self {
            requested_root: context.root.clone(),
            root,
            path,
            relative: relative.to_str()?.to_string(),
            ancestors,
        })
    }

    fn still_ordinary_missing(&self) -> bool {
        if self.requested_root.canonicalize().ok().as_ref() != Some(&self.root) {
            return false;
        }
        for (path, before) in &self.ancestors {
            let Ok(after) = std::fs::symlink_metadata(path) else {
                return false;
            };
            if !after.is_dir() || after.file_type().is_symlink() || !same_directory(before, &after)
            {
                return false;
            }
        }
        let Ok(relative) = self.path.strip_prefix(&self.root) else {
            return false;
        };
        let mut current = self.root.clone();
        let mut components = relative.components().peekable();
        while let Some(component) = components.next() {
            current.push(component);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink()
                        || (components.peek().is_some() && !metadata.is_dir())
                    {
                        return false;
                    }
                }
                Err(error) => return error.kind() == ErrorKind::NotFound,
            }
        }
        false
    }
}

#[cfg(unix)]
fn same_directory(before: &Metadata, after: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn same_directory(_before: &Metadata, _after: &Metadata) -> bool {
    // Until a stable directory identity is available here, do not grant the
    // additional recoverability on this platform.
    false
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) mod test_hook;
