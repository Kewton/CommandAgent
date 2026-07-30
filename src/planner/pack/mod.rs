//! Strict assist/eval pack loading.
//!
//! YAML selects only Rust-registered components. It never defines executable
//! validation or prompt behavior.

use std::path::{Path, PathBuf};

use schema::{AssistPack, EvalPack};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub(crate) mod builtin;
mod floor;
pub(crate) mod runtime;
mod schema;
mod strict_yaml;
mod vocabulary;

pub use floor::{ConformanceError, ConformanceReport, conform, conform_directory};
pub use schema::{
    ArtifactSchema, AssistPackDocument, CheckBinding, EvalPackDocument, Injection, Literal,
    PackIdentity, Vocabulary,
};
pub use vocabulary::{
    AssistSource, CheckId, ExtractionId, InjectionPoint, NormalizerId, PackIntent, PackProfile,
    VocabularySource,
};

pub const ASSIST_FILE: &str = "assist.yaml";
pub const EVAL_FILE: &str = "eval.yaml";
const HASH_DOMAIN: &[u8] = b"commandagent-pack-v0\0";
const MAX_FILE_BYTES: u64 = 256 * 1024;

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
}

impl LoadedPack {
    pub fn id(&self) -> &str {
        &self.identity.id
    }
}

pub fn load_directory(path: &Path) -> Result<LoadedPack, PackError> {
    let assist = read_optional(path.join(ASSIST_FILE))?;
    let eval = read_optional(path.join(EVAL_FILE))?;
    parse_bytes(assist.as_deref(), eval.as_deref())
}

pub fn parse_bytes(
    assist_bytes: Option<&[u8]>,
    eval_bytes: Option<&[u8]>,
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
        hash: exact_byte_hash(assist_bytes, eval_bytes),
        assist,
        eval,
    })
}

pub fn exact_byte_hash(assist_bytes: Option<&[u8]>, eval_bytes: Option<&[u8]>) -> String {
    let mut digest = Sha256::new();
    digest.update(HASH_DOMAIN);
    update_hash_file(&mut digest, ASSIST_FILE, assist_bytes.unwrap_or_default());
    update_hash_file(&mut digest, EVAL_FILE, eval_bytes.unwrap_or_default());
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

#[cfg(test)]
mod tests;
