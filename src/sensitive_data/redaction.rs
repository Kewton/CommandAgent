//! The single exact-value scrub, stream carry, YAML handling, and the
//! runnable-YAML/identity refusals. #504 owns this leaf.

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use super::{MAX_CATALOG_VALUES, MAX_DOTENV_BYTES, MAX_DOTENV_FILES, REDACTED, SecretCatalog};

/// A strict projection refusal. The message names the position, never the
/// secret, so a caller can fail honestly without leaking.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretScrubError {
    #[error("refusing to project a value whose dynamic key would expose a secret at {position}")]
    DynamicKey { position: String },
}

/// A refusal to save a runnable command/YAML or a confirmation identity that
/// still contains a registered secret.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("refusing to persist {field}: it contains a registered secret ({kind})")]
pub struct RunnableSecretRefusal {
    pub field: String,
    pub kind: &'static str,
}

/// Refusal raised while collecting root dotenv sources.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CollectionRefusal {
    #[error("failed to read {path}: {message}")]
    Read { path: String, message: String },
    #[error("refusing to follow the symlinked dotenv source {path}")]
    SymlinkNotFollowed { path: String },
    #[error("root dotenv collection exceeded {limit} files")]
    TooManyFiles { limit: usize },
    #[error("dotenv source {path} exceeded the {limit}-byte collection cap")]
    FileTooLarge { path: String, limit: usize },
}

/// The result of a bounded root dotenv collection.
#[derive(Debug)]
pub struct DotenvCollection {
    pub catalog: SecretCatalog,
    /// Count of matching symlinked entries that were skipped without following.
    pub skipped_symlinks: usize,
}

/// Replace every registered value with the marker. Longest-first ordering makes
/// the replacement prefix-safe, and byte equality keeps it UTF-8 safe.
pub fn scrub_text(values: &[String], text: &str) -> String {
    if values.is_empty() || text.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for value in values {
        if out.contains(value.as_str()) {
            out = out.replace(value.as_str(), REDACTED);
        }
    }
    out
}

fn key_contains_secret(values: &[String], key: &str) -> bool {
    values.iter().any(|value| key.contains(value.as_str()))
}

fn scrub_key(values: &[String], key: &str) -> String {
    scrub_text(values, key)
}

/// Strict recursive scrub. Refuses a dynamic key that would expose a secret.
pub fn scrub_value_strict(values: &[String], value: &mut Value) -> Result<(), SecretScrubError> {
    let mut position = vec!["root".to_string()];
    scrub_value_strict_at(values, value, &mut position)
}

fn scrub_value_strict_at(
    values: &[String],
    value: &mut Value,
    position: &mut Vec<String>,
) -> Result<(), SecretScrubError> {
    match value {
        Value::String(text) => {
            *text = scrub_text(values, text);
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                position.push(format!("[{index}]"));
                scrub_value_strict_at(values, item, position)?;
                position.pop();
            }
            Ok(())
        }
        Value::Object(map) => {
            if map.keys().any(|key| key_contains_secret(values, key)) {
                return Err(SecretScrubError::DynamicKey {
                    position: position.join("."),
                });
            }
            for item in map.values_mut() {
                position.push("{}".to_string());
                scrub_value_strict_at(values, item, position)?;
                position.pop();
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
    }
}

/// Lenient recursive scrub for boundaries that must not fail. A dynamic key is
/// rewritten in place rather than refused.
pub fn scrub_value_lenient(values: &[String], value: &mut Value) {
    match value {
        Value::String(text) => *text = scrub_text(values, text),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| scrub_value_lenient(values, item)),
        Value::Object(map) => {
            let entries = std::mem::take(map);
            let mut cleaned = serde_json::Map::with_capacity(entries.len());
            for (key, mut item) in entries {
                scrub_value_lenient(values, &mut item);
                cleaned.insert(scrub_key(values, &key), item);
            }
            *map = cleaned;
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Scrub every string scalar and string key of a YAML value.
pub fn scrub_yaml_value(values: &[String], value: &mut serde_yaml::Value) {
    match value {
        serde_yaml::Value::String(text) => *text = scrub_text(values, text),
        serde_yaml::Value::Sequence(items) => items
            .iter_mut()
            .for_each(|item| scrub_yaml_value(values, item)),
        serde_yaml::Value::Mapping(map) => {
            let entries = std::mem::take(map);
            let mut cleaned = serde_yaml::Mapping::with_capacity(entries.len());
            for (mut key, mut item) in entries {
                scrub_yaml_value(values, &mut item);
                if let serde_yaml::Value::String(text) = &mut key {
                    *text = scrub_text(values, text);
                }
                cleaned.insert(key, item);
            }
            *map = cleaned;
        }
        serde_yaml::Value::Tagged(tagged) => scrub_yaml_value(values, &mut tagged.value),
        serde_yaml::Value::Null | serde_yaml::Value::Bool(_) | serde_yaml::Value::Number(_) => {}
    }
}

/// True when a runnable YAML document contains a registered secret, either as a
/// scalar or in its raw text when the document cannot be parsed.
pub fn runnable_yaml_contains_secret(catalog: &SecretCatalog, text: &str) -> bool {
    if catalog.is_empty() {
        return false;
    }
    match serde_yaml::from_str::<serde_yaml::Value>(text) {
        Ok(mut value) => {
            let mut found = false;
            let hits = catalog.values();
            yaml_contains(hits, &value, &mut found);
            // Scrub is idempotent; a second pass confirms the projection.
            scrub_yaml_value(hits, &mut value);
            found
        }
        Err(_) => catalog.contains(text),
    }
}

fn yaml_contains(values: &[String], value: &serde_yaml::Value, found: &mut bool) {
    if *found {
        return;
    }
    match value {
        serde_yaml::Value::String(text) => *found = values.iter().any(|hit| text.contains(hit)),
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                yaml_contains(values, item, found);
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (key, item) in map {
                yaml_contains(values, key, found);
                yaml_contains(values, item, found);
            }
        }
        serde_yaml::Value::Tagged(tagged) => yaml_contains(values, &tagged.value, found),
        _ => {}
    }
}

/// Refuse a runnable command/YAML value that still contains a secret.
pub fn refuse_runnable(
    catalog: &SecretCatalog,
    field: &str,
    text: &str,
) -> Result<(), RunnableSecretRefusal> {
    let kind = if runnable_yaml_contains_secret(catalog, text) {
        "runnable_yaml"
    } else if catalog.contains(text) {
        "runnable_command"
    } else {
        return Ok(());
    };
    Err(RunnableSecretRefusal {
        field: field.to_string(),
        kind,
    })
}

/// Refuse a confirmation identity (a hash source or canonical key) that still
/// contains a secret. Used before an identity is hashed and stored.
pub fn refuse_identity(
    catalog: &SecretCatalog,
    field: &str,
    text: &str,
) -> Result<(), RunnableSecretRefusal> {
    if catalog.contains(text) {
        return Err(RunnableSecretRefusal {
            field: field.to_string(),
            kind: "identity",
        });
    }
    Ok(())
}

/// A chunk-boundary-safe scrubber. It holds back up to `max_value_len - 1`
/// bytes (and any straddling registered value) so a secret split across chunks
/// is never partially emitted.
pub struct StreamScrubber {
    catalog: Arc<SecretCatalog>,
    pending: String,
}

impl StreamScrubber {
    pub fn new(catalog: Arc<SecretCatalog>) -> Self {
        Self {
            catalog,
            pending: String::new(),
        }
    }

    /// Emit the safe prefix of `chunk`; retains the part that could still be a
    /// partial secret.
    pub fn push(&mut self, chunk: &str) -> String {
        if self.catalog.is_empty() {
            return chunk.to_string();
        }
        self.pending.push_str(chunk);
        let values = self.catalog.values();
        let hold_back = self.catalog.max_value_len().saturating_sub(1);
        let mut boundary = self.pending.len().saturating_sub(hold_back);
        if boundary > 0 {
            'outer: for value in values {
                let length = value.len();
                let mut search = 0;
                while let Some(relative) = self.pending[search..].find(value.as_str()) {
                    let start = search + relative;
                    if start >= boundary {
                        break;
                    }
                    if start + length > boundary {
                        boundary = start;
                        break 'outer;
                    }
                    search = start + 1;
                }
            }
        }
        while boundary > 0 && !self.pending.is_char_boundary(boundary) {
            boundary -= 1;
        }
        let emitted: String = self.pending.drain(..boundary).collect();
        self.catalog.scrub(&emitted)
    }

    /// Flush the retained tail.
    pub fn finish(&mut self) -> String {
        let tail = std::mem::take(&mut self.pending);
        self.catalog.scrub(&tail)
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

impl std::fmt::Debug for StreamScrubber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let held = self.pending.len();
        formatter
            .debug_struct("StreamScrubber")
            .field("catalog", &self.catalog)
            .field("pending", &format_args!("<{held} byte(s) held>"))
            .finish()
    }
}

/// Collect the secret values of the root dotenv sources.
///
/// The controller reads only the three strict template names as non-secrets;
/// every other `.env`/`.env.*` regular file at the root is read with a bounded,
/// no-follow walk. A symlinked source is skipped (never followed) and counted,
/// a read failure is reported instead of silently dropping a secret, and the
/// per-file/entry caps are enforced.
pub fn collect_scoped_dotenv(root: &Path) -> Result<DotenvCollection, CollectionRefusal> {
    let mut names = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DotenvCollection {
                catalog: SecretCatalog::new(),
                skipped_symlinks: 0,
            });
        }
        Err(error) => {
            return Err(CollectionRefusal::Read {
                path: root.display().to_string(),
                message: error.to_string(),
            });
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_dotenv_source_name(&name) {
            continue;
        }
        names.push(name);
    }
    names.sort();

    let mut catalog = SecretCatalog::new();
    let mut skipped_symlinks = 0usize;
    let mut counted = 0usize;
    for name in names {
        let path = root.join(&name);
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            skipped_symlinks += 1;
            continue;
        }
        if catalog.len() >= MAX_CATALOG_VALUES {
            break;
        }
        counted += 1;
        if counted > MAX_DOTENV_FILES {
            return Err(CollectionRefusal::TooManyFiles {
                limit: MAX_DOTENV_FILES,
            });
        }
        if metadata.len() as usize > MAX_DOTENV_BYTES {
            return Err(CollectionRefusal::FileTooLarge {
                path: path.display().to_string(),
                limit: MAX_DOTENV_BYTES,
            });
        }
        let content = std::fs::read_to_string(&path).map_err(|error| CollectionRefusal::Read {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        for (key, value) in parse_dotenv(&content) {
            catalog.register_named(&key, &value);
        }
    }
    Ok(DotenvCollection {
        catalog,
        skipped_symlinks,
    })
}

fn is_dotenv_source_name(name: &str) -> bool {
    if crate::tools::sensitive_path::is_template_name(name) {
        return false;
    }
    name == ".env" || name.starts_with(".env.")
}

/// Parse dotenv text with the same trim/quote interpretation as the existing
/// loader, so the catalog and the loaded key agree.
pub fn parse_dotenv(content: &str) -> Vec<(String, String)> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            Some((key.trim().to_string(), value))
        })
        .collect()
}
