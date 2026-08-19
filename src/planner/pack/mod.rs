//! Strict assist/eval pack loading.
//!
//! YAML selects only Rust-registered components. It never defines executable
//! validation or prompt behavior.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schema::{AssistPack, EvalPack};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub(crate) mod builtin;
pub mod catalog;
pub(crate) mod checks;
mod floor;
mod material_document;
pub(crate) mod runtime;
mod schema;
mod score;
mod strict_yaml;
pub mod supply;
mod vocabulary;

pub use floor::{ConformanceError, ConformanceReport, conform, conform_directory};
pub use schema::{
    ArtifactSchema, AssistPackDocument, CheckBinding, EvalPackDocument, Injection, Literal,
    PackIdentity, Vocabulary,
};
pub use score::{
    AtomState, ScoreAtom, ScoreAtomVector, ScoreDeclaration, ScoreUsage, ScoreVector, ScoreWeight,
};
pub use supply::{
    Action as SupplyAction, Actor, JournalEntry, ScrubReport, StageReport, StagedFile,
    SuppliedPack, SupplyError, SupplyRoot,
};
pub use vocabulary::{
    AssistSource, CheckId, ExtractionId, InjectionPoint, NormalizerId, PackIntent, PackProfile,
    VocabularySource,
};

pub const ASSIST_FILE: &str = "assist.yaml";
pub const EVAL_FILE: &str = "eval.yaml";
const HASH_DOMAIN: &[u8] = b"commandagent-pack-v0\0";
const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_MATERIAL_BYTES: u64 = 65_536;
const MAX_TOTAL_MATERIAL_BYTES: u64 = 262_144;
const MATERIALS_DIRECTORY: &str = "materials";
/// Pack subtree of a repository root or an operator-supplied extension root.
pub const PACKS_DIRECTORY: &str = "packs";

#[derive(Debug, Error)]
pub enum PackError {
    #[error("pack must contain assist.yaml or eval.yaml")]
    Empty,
    #[error("pack path `{path}` is not a regular file")]
    NotRegularFile { path: PathBuf },
    #[error("failed to read pack file `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("pack file `{path}` exceeds {MAX_FILE_BYTES} bytes")]
    TooLarge { path: PathBuf },
    #[error("pack material `{path}` exceeds {MAX_MATERIAL_BYTES} bytes")]
    MaterialTooLarge { path: PathBuf },
    #[error("pack materials exceed {MAX_TOTAL_MATERIAL_BYTES} bytes in aggregate")]
    MaterialsTooLarge,
    #[error("pack contains unsupported member `{path}`")]
    UnsupportedMember { path: PathBuf },
    #[error("pack material name `{name}` is invalid")]
    InvalidMaterialName { name: String },
    #[error("pack material `{path}` is not valid UTF-8")]
    MaterialNotUtf8 { path: PathBuf },
    #[error("{file} is invalid: {reason}")]
    Invalid { file: &'static str, reason: String },
    #[error("assist.yaml and eval.yaml pack identities differ")]
    IdentityMismatch,
}

#[derive(Debug)]
pub struct LoadedPack {
    pub identity: PackIdentity,
    pub hash: String,
    pub assist: Option<AssistPackDocument>,
    pub eval: Option<EvalPackDocument>,
    pub materials: BTreeMap<String, Vec<u8>>,
}

impl LoadedPack {
    pub fn id(&self) -> &str {
        &self.identity.id
    }
}

pub fn load_directory(path: &Path) -> Result<LoadedPack, PackError> {
    validate_pack_directory(path)?;
    let assist = read_optional(path.join(ASSIST_FILE))?;
    let eval = read_optional(path.join(EVAL_FILE))?;
    let materials = read_materials(path)?;
    parse_members(assist.as_deref(), eval.as_deref(), materials)
}

pub fn parse_bytes(
    assist_bytes: Option<&[u8]>,
    eval_bytes: Option<&[u8]>,
) -> Result<LoadedPack, PackError> {
    parse_members(assist_bytes, eval_bytes, BTreeMap::new())
}

fn parse_members(
    assist_bytes: Option<&[u8]>,
    eval_bytes: Option<&[u8]>,
    materials: BTreeMap<String, Vec<u8>>,
) -> Result<LoadedPack, PackError> {
    if assist_bytes.is_none() && eval_bytes.is_none() {
        return Err(PackError::Empty);
    }
    let assist = assist_bytes
        .map(|bytes| strict_yaml::decode::<AssistPack>(ASSIST_FILE, bytes))
        .transpose()?
        .map(AssistPack::validate)
        .transpose()
        .map_err(|reason| PackError::Invalid {
            file: ASSIST_FILE,
            reason,
        })?;
    let eval = eval_bytes
        .map(|bytes| strict_yaml::decode::<EvalPack>(EVAL_FILE, bytes))
        .transpose()?
        .map(EvalPack::validate)
        .transpose()
        .map_err(|reason| PackError::Invalid {
            file: EVAL_FILE,
            reason,
        })?;
    let identity = match (&assist, &eval) {
        (Some(assist), Some(eval)) if assist.pack == eval.pack => assist.pack.clone(),
        (Some(_), Some(_)) => return Err(PackError::IdentityMismatch),
        (Some(assist), None) => assist.pack.clone(),
        (None, Some(eval)) => eval.pack.clone(),
        (None, None) => unreachable!("empty pack rejected above"),
    };
    Ok(LoadedPack {
        identity,
        hash: exact_byte_hash_with_materials(assist_bytes, eval_bytes, &materials),
        assist,
        eval,
        materials,
    })
}

pub fn exact_byte_hash(assist_bytes: Option<&[u8]>, eval_bytes: Option<&[u8]>) -> String {
    exact_byte_hash_with_materials(assist_bytes, eval_bytes, &BTreeMap::new())
}

fn exact_byte_hash_with_materials(
    assist_bytes: Option<&[u8]>,
    eval_bytes: Option<&[u8]>,
    materials: &BTreeMap<String, Vec<u8>>,
) -> String {
    let mut digest = Sha256::new();
    digest.update(HASH_DOMAIN);
    update_hash_file(&mut digest, ASSIST_FILE, assist_bytes.unwrap_or_default());
    update_hash_file(&mut digest, EVAL_FILE, eval_bytes.unwrap_or_default());
    for (name, bytes) in materials {
        update_hash_file(&mut digest, &format!("{MATERIALS_DIRECTORY}/{name}"), bytes);
    }
    format!("sha256:{:x}", digest.finalize())
}

fn update_hash_file(digest: &mut Sha256, name: &str, bytes: &[u8]) {
    digest.update((name.len() as u64).to_be_bytes());
    digest.update(name.as_bytes());
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

fn read_optional(path: PathBuf) -> Result<Option<Vec<u8>>, PackError> {
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(PackError::Io { path, source }),
    };
    if !metadata.file_type().is_file() {
        return Err(PackError::NotRegularFile { path });
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(PackError::TooLarge { path });
    }
    std::fs::read(&path)
        .map(Some)
        .map_err(|source| PackError::Io { path, source })
}

fn validate_pack_directory(path: &Path) -> Result<(), PackError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|source| PackError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.file_type().is_dir() {
        return Err(PackError::NotRegularFile {
            path: path.to_path_buf(),
        });
    }
    for entry in std::fs::read_dir(path).map_err(|source| PackError::Io {
        path: path.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| PackError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let name = entry.file_name();
        let supported = matches!(
            name.to_str(),
            Some(
                ASSIST_FILE
                    | EVAL_FILE
                    | MATERIALS_DIRECTORY
                    | catalog::PACK_PIN_FILE
                    | catalog::RETIRED_MARKER_FILE
            )
        );
        if !supported {
            return Err(PackError::UnsupportedMember { path: entry.path() });
        }
        if matches!(
            name.to_str(),
            Some(catalog::PACK_PIN_FILE | catalog::RETIRED_MARKER_FILE)
        ) && !entry
            .file_type()
            .map_err(|source| PackError::Io {
                path: entry.path(),
                source,
            })?
            .is_file()
        {
            return Err(PackError::NotRegularFile { path: entry.path() });
        }
    }
    Ok(())
}

fn read_materials(path: &Path) -> Result<BTreeMap<String, Vec<u8>>, PackError> {
    let directory = path.join(MATERIALS_DIRECTORY);
    let metadata = match std::fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(source) => {
            return Err(PackError::Io {
                path: directory,
                source,
            });
        }
    };
    if !metadata.file_type().is_dir() {
        return Err(PackError::NotRegularFile { path: directory });
    }
    let mut materials = BTreeMap::new();
    let mut total = 0_u64;
    for entry in std::fs::read_dir(&directory).map_err(|source| PackError::Io {
        path: directory.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| PackError::Io {
            path: directory.clone(),
            source,
        })?;
        let name =
            entry
                .file_name()
                .into_string()
                .map_err(|name| PackError::InvalidMaterialName {
                    name: name.to_string_lossy().into_owned(),
                })?;
        if !valid_material_name(&name) {
            return Err(PackError::InvalidMaterialName { name });
        }
        let member = entry.path();
        let metadata = std::fs::symlink_metadata(&member).map_err(|source| PackError::Io {
            path: member.clone(),
            source,
        })?;
        if !metadata.file_type().is_file() {
            return Err(PackError::NotRegularFile { path: member });
        }
        if metadata.len() > MAX_MATERIAL_BYTES {
            return Err(PackError::MaterialTooLarge { path: member });
        }
        total = total
            .checked_add(metadata.len())
            .ok_or(PackError::MaterialsTooLarge)?;
        if total > MAX_TOTAL_MATERIAL_BYTES {
            return Err(PackError::MaterialsTooLarge);
        }
        let bytes = std::fs::read(&member).map_err(|source| PackError::Io {
            path: member.clone(),
            source,
        })?;
        std::str::from_utf8(&bytes).map_err(|_| PackError::MaterialNotUtf8 { path: member })?;
        materials.insert(name, bytes);
    }
    Ok(materials)
}

pub(crate) fn valid_material_name(name: &str) -> bool {
    name.ends_with(".md")
        && name.len() > 3
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests;
