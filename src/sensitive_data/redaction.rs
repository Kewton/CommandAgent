//! The single exact-value scrub, stream carry, YAML handling, and the
//! runnable-YAML/identity refusals. #504 owns this leaf.

use std::collections::BTreeSet;
use std::io::Read as _;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use super::{
    MAX_CATALOG_VALUES, MAX_DOTENV_BYTES, MAX_DOTENV_FILES, MIN_GENERIC_SECRET_LEN, SecretCatalog,
};

/// Keys whose value is a fixed schema identifier (event name, status, verdict,
/// type, ...). A short secret never redacts one of these values whole; the
/// identifier is preserved and the coincidence is not a leak.
const FIXED_SCHEMA_KEYS: &[&str] = &[
    "event",
    "schema_version",
    "status",
    "verdict",
    "type",
    "kind",
    "classification",
    "level",
    "ok",
    "phase",
    "stage",
    "family",
    "envelope_version",
    "lifecycle_stage",
    "source_ref",
    "source_refs",
    "role",
    "caller_role",
    "caller_scope",
    "tool_name",
    "step_kind",
    "judgement",
    "direction",
    "action",
];

fn is_fixed_schema_key(key: &str) -> bool {
    FIXED_SCHEMA_KEYS.contains(&key)
}

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
    #[error("root dotenv collection exceeded {limit} values")]
    TooManyValues { limit: usize },
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

/// Replace all long registered values with the marker.
///
/// Occurrences are collected as spans and merged, so two overlapping secrets
/// (or a secret repeated with overlap) are removed as one region and no
/// fragment of either is emitted.
fn replace_long(catalog: &SecretCatalog, text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for value in catalog.values() {
        if value.len() < MIN_GENERIC_SECRET_LEN {
            continue;
        }
        let mut search = 0usize;
        while let Some(relative) = text[search..].find(value.as_str()) {
            let start = search + relative;
            spans.push((start, start + value.len()));
            let step = text[start..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            search = start + step;
        }
    }
    if spans.is_empty() {
        return text.to_string();
    }
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(spans.len());
    for (start, end) in spans {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
            continue;
        }
        merged.push((start, end));
    }
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for (start, end) in merged {
        out.push_str(&text[cursor..start]);
        out.push_str(catalog.marker());
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    out
}

/// Replace every registered value with the marker. Long values are replaced as
/// substrings; a short value redacts the whole text, because a short substring
/// replacement cannot be projected safely.
pub fn scrub_text(catalog: &SecretCatalog, text: &str) -> String {
    if catalog.is_empty() || text.is_empty() {
        return text.to_string();
    }
    let replaced = replace_long(catalog, text);
    if catalog.has_short() && catalog.contains_short(&replaced) {
        return catalog.marker().to_string();
    }
    replaced
}

/// Replace long values inside an object key. Keys are never redacted whole and
/// short values never rewrite a key, so fixed identifiers survive.
pub fn scrub_key(catalog: &SecretCatalog, key: &str) -> String {
    replace_long(catalog, key)
}

fn scrub_string(catalog: &SecretCatalog, key: Option<&str>, text: &str) -> String {
    let replaced = replace_long(catalog, text);
    if catalog.has_short()
        && catalog.contains_short(&replaced)
        && !key.is_some_and(is_fixed_schema_key)
    {
        return catalog.marker().to_string();
    }
    replaced
}

/// Strict recursive scrub. Refuses a dynamic key that contains a long secret.
pub fn scrub_value_strict(
    catalog: &SecretCatalog,
    value: &mut Value,
) -> Result<(), SecretScrubError> {
    let mut position = vec!["root".to_string()];
    scrub_value_strict_at(catalog, value, None, &mut position)
}

fn scrub_value_strict_at(
    catalog: &SecretCatalog,
    value: &mut Value,
    key: Option<&str>,
    position: &mut Vec<String>,
) -> Result<(), SecretScrubError> {
    match value {
        Value::String(text) => {
            *text = scrub_string(catalog, key, text);
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                position.push(format!("[{index}]"));
                scrub_value_strict_at(catalog, item, None, position)?;
                position.pop();
            }
            Ok(())
        }
        Value::Object(map) => {
            if map.keys().any(|candidate| catalog.contains_long(candidate)) {
                return Err(SecretScrubError::DynamicKey {
                    position: position.join("."),
                });
            }
            for (candidate, item) in map.iter_mut() {
                position.push("{}".to_string());
                scrub_value_strict_at(catalog, item, Some(candidate), position)?;
                position.pop();
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
    }
}

/// Lenient recursive scrub for boundaries that must not fail. A dynamic key
/// that contains a long secret is rewritten to a distinct safe key so no
/// evidence is lost and no fixed key is corrupted.
pub fn scrub_value_lenient(catalog: &SecretCatalog, value: &mut Value) {
    scrub_value_lenient_at(catalog, value, None);
}

fn scrub_value_lenient_at(catalog: &SecretCatalog, value: &mut Value, key: Option<&str>) {
    match value {
        Value::String(text) => *text = scrub_string(catalog, key, text),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| scrub_value_lenient_at(catalog, item, None)),
        Value::Object(map) => {
            let entries = std::mem::take(map);
            let mut cleaned = serde_json::Map::with_capacity(entries.len());
            for (candidate, mut item) in entries {
                scrub_value_lenient_at(catalog, &mut item, Some(&candidate));
                let unique = unique_json_key(catalog, &cleaned, &candidate);
                cleaned.insert(unique, item);
            }
            *map = cleaned;
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn unique_json_key(
    catalog: &SecretCatalog,
    cleaned: &serde_json::Map<String, Value>,
    key: &str,
) -> String {
    let base = scrub_key(catalog, key);
    if !cleaned.contains_key(&base) {
        return base;
    }
    let mut index = 1usize;
    loop {
        let candidate = format!("{base}#{index}");
        if !cleaned.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

/// Scrub every string scalar and string key of a YAML value.
pub fn scrub_yaml_value(catalog: &SecretCatalog, value: &mut serde_yaml::Value) {
    scrub_yaml_at(catalog, value, &mut BTreeSet::new());
}

fn scrub_yaml_at(
    catalog: &SecretCatalog,
    value: &mut serde_yaml::Value,
    used: &mut BTreeSet<String>,
) {
    match value {
        serde_yaml::Value::String(text) => *text = scrub_string(catalog, None, text),
        serde_yaml::Value::Sequence(items) => items
            .iter_mut()
            .for_each(|item| scrub_yaml_at(catalog, item, used)),
        serde_yaml::Value::Mapping(map) => {
            let entries = std::mem::take(map);
            let mut cleaned = serde_yaml::Mapping::new();
            for (key, mut item) in entries {
                scrub_yaml_at(catalog, &mut item, used);
                let key = unique_yaml_key(catalog, used, key);
                cleaned.insert(key, item);
            }
            *map = cleaned;
        }
        serde_yaml::Value::Tagged(tagged) => scrub_yaml_at(catalog, &mut tagged.value, used),
        serde_yaml::Value::Null | serde_yaml::Value::Bool(_) | serde_yaml::Value::Number(_) => {}
    }
}

fn unique_yaml_key(
    catalog: &SecretCatalog,
    used: &mut BTreeSet<String>,
    key: serde_yaml::Value,
) -> serde_yaml::Value {
    let serde_yaml::Value::String(text) = &key else {
        return key;
    };
    let mut candidate = scrub_key(catalog, text);
    if used.contains(&candidate) {
        let base = candidate.clone();
        let mut index = 1usize;
        loop {
            let next = format!("{base}#{index}");
            if !used.contains(&next) {
                candidate = next;
                break;
            }
            index += 1;
        }
    }
    used.insert(candidate.clone());
    serde_yaml::Value::String(candidate)
}

/// True when a runnable YAML document contains a registered secret, either as a
/// scalar or in its raw text when the document cannot be parsed.
pub fn runnable_yaml_contains_secret(catalog: &SecretCatalog, text: &str) -> bool {
    if catalog.is_empty() {
        return false;
    }
    match serde_yaml::from_str::<serde_yaml::Value>(text) {
        Ok(value) => {
            let mut found = false;
            yaml_contains(catalog, &value, &mut found);
            found
        }
        Err(_) => catalog.contains(text),
    }
}

fn yaml_contains(catalog: &SecretCatalog, value: &serde_yaml::Value, found: &mut bool) {
    if *found {
        return;
    }
    match value {
        serde_yaml::Value::String(text) => *found = catalog.contains(text),
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                yaml_contains(catalog, item, found);
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (key, item) in map {
                yaml_contains(catalog, key, found);
                yaml_contains(catalog, item, found);
            }
        }
        serde_yaml::Value::Tagged(tagged) => yaml_contains(catalog, &tagged.value, found),
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

/// A chunk-boundary-safe scrubber. It holds back enough bytes that no secret is
/// ever partially emitted, even when occurrences overlap or span multibyte
/// characters.
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
        let hold_back = self.catalog.max_value_len().saturating_sub(1);
        let mut boundary = self.pending.len().saturating_sub(hold_back);
        // Pull the boundary back to the start of every occurrence that would be
        // cut, repeating to a fixpoint so an overlapping shorter value cannot
        // leak its prefix.
        loop {
            let mut adjusted = boundary;
            for value in self.catalog.values() {
                if value.is_empty() {
                    continue;
                }
                let length = value.len();
                let mut search = 0usize;
                while let Some(relative) = self.pending[search..].find(value.as_str()) {
                    let start = search + relative;
                    if start >= adjusted {
                        break;
                    }
                    if start + length > adjusted {
                        adjusted = start;
                        break;
                    }
                    let step = self.pending[start..]
                        .chars()
                        .next()
                        .map(char::len_utf8)
                        .unwrap_or(1);
                    search = start + step;
                }
            }
            if adjusted == boundary {
                break;
            }
            boundary = adjusted;
            if boundary == 0 {
                break;
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
/// no-follow open. A symlinked source is skipped (never followed) and counted;
/// a read failure, or exceeding the file/value caps, is reported instead of
/// silently dropping a secret.
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
    let mut counted_files = 0usize;
    let mut counted_values = 0usize;
    for name in names {
        let path = root.join(&name);
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            skipped_symlinks += 1;
            continue;
        }
        counted_files += 1;
        if counted_files > MAX_DOTENV_FILES {
            return Err(CollectionRefusal::TooManyFiles {
                limit: MAX_DOTENV_FILES,
            });
        }
        let content = read_bounded_regular(&path)?;
        for (key, value) in parse_dotenv(&content) {
            if is_secret_value(&key, &value) {
                counted_values += 1;
                if counted_values > MAX_CATALOG_VALUES {
                    return Err(CollectionRefusal::TooManyValues {
                        limit: MAX_CATALOG_VALUES,
                    });
                }
            }
            catalog.register_named(&key, &value);
        }
    }
    Ok(DotenvCollection {
        catalog,
        skipped_symlinks,
    })
}

fn is_secret_value(name: &str, value: &str) -> bool {
    super::is_credential_name(name) || value.trim().len() >= MIN_GENERIC_SECRET_LEN
}

fn read_bounded_regular(path: &Path) -> Result<String, CollectionRefusal> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Never follow a symlink that replaced the entry after the metadata
        // check, and never open a directory as a dotenv source.
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|error| CollectionRefusal::Read {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
    let metadata = file.metadata().map_err(|error| CollectionRefusal::Read {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    if metadata.len() as usize > MAX_DOTENV_BYTES {
        return Err(CollectionRefusal::FileTooLarge {
            path: path.display().to_string(),
            limit: MAX_DOTENV_BYTES,
        });
    }
    let mut bytes = Vec::new();
    file.take(MAX_DOTENV_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| CollectionRefusal::Read {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
    if bytes.len() > MAX_DOTENV_BYTES {
        return Err(CollectionRefusal::FileTooLarge {
            path: path.display().to_string(),
            limit: MAX_DOTENV_BYTES,
        });
    }
    String::from_utf8(bytes).map_err(|error| CollectionRefusal::Read {
        path: path.display().to_string(),
        message: error.to_string(),
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
