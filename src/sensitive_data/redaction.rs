//! The single exact-value scrub, stream carry, YAML handling, bounded dotenv
//! collection, and the refusal types. #504 owns this leaf.
//!
//! Registration/scrub contract (Issue #504 design 3-5, 2026-09-29):
//! - Only values at least [`MIN_GENERIC_SECRET_LEN`] bytes long are secrets at
//!   all. A shorter credential value is refused at the source; a shorter
//!   non-credential value is simply not a secret.
//! - Every registered value is replaced by an exact, case-sensitive match. No
//!   free field is redacted whole.
//! - A top-level fixed schema identifier (event/schema/status/verdict/type/...)
//!   is never rewritten, so a value that happens to collide with one is
//!   preserved rather than corrupting the schema.
//! - JSON/YAML keep element count, are idempotent, and leave a key unchanged
//!   unless that key actually contained a secret.

use std::collections::BTreeSet;
use std::io::Read as _;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use super::{MAX_DOTENV_BYTES, MAX_DOTENV_FILES, MIN_GENERIC_SECRET_LEN, SecretCatalog};

/// Top-level keys whose value is a fixed schema identifier. Such a value is
/// never rewritten, so a registered value that collides with it cannot corrupt
/// the schema.
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

/// A refusal to register a value. The value itself is never named.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistrationRefusal {
    #[error(
        "refusing to register the credential value for {name}: it is shorter than {MIN_GENERIC_SECRET_LEN} characters"
    )]
    TooShort { name: String },
    #[error("refusing to register the credential value for {name}: it equals a reserved marker")]
    Reserved { name: String },
    #[error("refusing to register more than {limit} secret values")]
    CatalogFull { limit: usize },
}

/// A refusal to save a runnable command/YAML or a confirmation identity that
/// still contains a registered secret.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("refusing to persist {field}: it contains a registered secret ({kind})")]
pub struct RunnableSecretRefusal {
    pub field: String,
    pub kind: &'static str,
}

/// A refusal raised while collecting a root dotenv source. No message contains
/// a secret value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CollectionRefusal {
    #[error("failed to read {path}: {message}")]
    Read { path: String, message: String },
    #[error("root dotenv collection exceeded {limit} files")]
    TooManyFiles { limit: usize },
    #[error("root dotenv collection exceeded {limit} values")]
    TooManyValues { limit: usize },
    #[error("dotenv source {path} exceeded the {limit}-byte collection cap")]
    FileTooLarge { path: String, limit: usize },
    #[error(
        "refusing to register the credential value for {name}: it is shorter than {limit} characters"
    )]
    CredentialTooShort { name: String, limit: usize },
    #[error("refusing to register the credential value for {name}: it equals a reserved marker")]
    ReservedValue { name: String },
}

/// The bounded dotenv collection result. The catalog is returned even when some
/// source was refused, so a caller can install it and still scrub what it read.
#[derive(Debug)]
pub struct DotenvCollection {
    pub catalog: SecretCatalog,
    pub skipped_symlinks: usize,
}

/// A collection that refused one or more sources, carrying the partial catalog.
#[derive(Debug)]
pub struct CollectionFailure {
    pub refusals: Vec<CollectionRefusal>,
    pub catalog: SecretCatalog,
}

impl std::fmt::Display for CollectionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.refusals.first() {
            Some(refusal) => write!(formatter, "{refusal}"),
            None => write!(formatter, "dotenv collection refused"),
        }
    }
}

impl std::error::Error for CollectionFailure {}

/// Replace every registered value with the marker.
///
/// Occurrences are collected as spans and merged, so two overlapping secrets
/// (or a secret repeated with overlap) are removed as one region and no
/// fragment of either is emitted.
fn replace_values(catalog: &SecretCatalog, text: &str) -> String {
    if text.is_empty() || catalog.is_empty() {
        return text.to_string();
    }
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for value in catalog.values() {
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

/// Exact-value replacement for free text.
pub fn scrub_text(catalog: &SecretCatalog, text: &str) -> String {
    replace_values(catalog, text)
}

/// Exact-value replacement for an object key.
pub fn scrub_key(catalog: &SecretCatalog, key: &str) -> String {
    replace_values(catalog, key)
}

fn scrub_string(catalog: &SecretCatalog, fixed: bool, text: &str) -> String {
    if fixed {
        text.to_string()
    } else {
        replace_values(catalog, text)
    }
}

/// Strict recursive scrub. Refuses a dynamic key that contains a secret.
pub fn scrub_value_strict(
    catalog: &SecretCatalog,
    value: &mut Value,
) -> Result<(), SecretScrubError> {
    let mut position = vec!["root".to_string()];
    strict_at(catalog, value, 1, false, &mut position)
}

fn strict_at(
    catalog: &SecretCatalog,
    value: &mut Value,
    depth: usize,
    fixed: bool,
    position: &mut Vec<String>,
) -> Result<(), SecretScrubError> {
    match value {
        Value::String(text) => {
            *text = scrub_string(catalog, fixed, text);
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                position.push(format!("[{index}]"));
                strict_at(catalog, item, depth + 1, false, position)?;
                position.pop();
            }
            Ok(())
        }
        Value::Object(map) => {
            if map.keys().any(|key| catalog.contains(key)) {
                return Err(SecretScrubError::DynamicKey {
                    position: position.join("."),
                });
            }
            for (key, item) in map.iter_mut() {
                let fixed = depth == 1 && is_fixed_schema_key(key);
                position.push("{}".to_string());
                strict_at(catalog, item, depth + 1, fixed, position)?;
                position.pop();
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
    }
}

/// Lenient recursive scrub for boundaries that must not fail. A dynamic key
/// that contains a secret is projected to a distinct safe key, so no evidence
/// is lost and unrelated keys are left unchanged.
pub fn scrub_value_lenient(catalog: &SecretCatalog, value: &mut Value) {
    lenient_at(catalog, value, 1, false);
}

fn lenient_at(catalog: &SecretCatalog, value: &mut Value, depth: usize, fixed: bool) {
    match value {
        Value::String(text) => *text = scrub_string(catalog, fixed, text),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| lenient_at(catalog, item, depth + 1, false)),
        Value::Object(map) => {
            let entries = std::mem::take(map);
            let mut cleaned = serde_json::Map::with_capacity(entries.len());
            for (key, mut item) in entries {
                let fixed = depth == 1 && is_fixed_schema_key(&key);
                lenient_at(catalog, &mut item, depth + 1, fixed);
                let key = if fixed {
                    key
                } else {
                    unique_json_key(catalog, &cleaned, &key)
                };
                cleaned.insert(key, item);
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

/// Scrub every string scalar and string key of a YAML value. Key uniqueness is
/// tracked per mapping, and a key is only renamed when it actually changed.
pub fn scrub_yaml_value(catalog: &SecretCatalog, value: &mut serde_yaml::Value) {
    scrub_yaml_at(catalog, value, 1, false);
}

fn scrub_yaml_at(
    catalog: &SecretCatalog,
    value: &mut serde_yaml::Value,
    depth: usize,
    fixed: bool,
) {
    match value {
        serde_yaml::Value::String(text) => *text = scrub_string(catalog, fixed, text),
        serde_yaml::Value::Sequence(items) => items
            .iter_mut()
            .for_each(|item| scrub_yaml_at(catalog, item, depth + 1, false)),
        serde_yaml::Value::Mapping(map) => {
            let entries = std::mem::take(map);
            let mut cleaned = serde_yaml::Mapping::new();
            let mut used: BTreeSet<String> = BTreeSet::new();
            for (key, mut item) in entries {
                let fixed = depth == 1
                    && matches!(&key, serde_yaml::Value::String(name) if is_fixed_schema_key(name));
                scrub_yaml_at(catalog, &mut item, depth + 1, fixed);
                let key = scrub_yaml_key(catalog, key, depth, &mut used);
                cleaned.insert(key, item);
            }
            *map = cleaned;
        }
        serde_yaml::Value::Tagged(tagged) => {
            scrub_yaml_at(catalog, &mut tagged.value, depth, false)
        }
        serde_yaml::Value::Null | serde_yaml::Value::Bool(_) | serde_yaml::Value::Number(_) => {}
    }
}

fn scrub_yaml_key(
    catalog: &SecretCatalog,
    key: serde_yaml::Value,
    depth: usize,
    used: &mut BTreeSet<String>,
) -> serde_yaml::Value {
    match key {
        serde_yaml::Value::String(text) => {
            if depth == 1 && is_fixed_schema_key(&text) {
                used.insert(text.clone());
                return serde_yaml::Value::String(text);
            }
            let replaced = scrub_key(catalog, &text);
            let candidate = if used.contains(&replaced) {
                let mut index = 1usize;
                loop {
                    let next = format!("{replaced}#{index}");
                    if !used.contains(&next) {
                        break next;
                    }
                    index += 1;
                }
            } else {
                replaced
            };
            used.insert(candidate.clone());
            serde_yaml::Value::String(candidate)
        }
        serde_yaml::Value::Sequence(mut items) => {
            items
                .iter_mut()
                .for_each(|item| scrub_yaml_at(catalog, item, depth + 1, false));
            serde_yaml::Value::Sequence(items)
        }
        serde_yaml::Value::Tagged(mut tagged) => {
            scrub_yaml_at(catalog, &mut tagged.value, depth, false);
            serde_yaml::Value::Tagged(tagged)
        }
        other => other,
    }
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
/// no-follow open. A read failure, a file/value cap, or a too-short/reserved
/// credential value is reported instead of silently dropping a secret. The
/// partial catalog is always returned so an emitted record can still be
/// scrubbed.
pub fn collect_scoped_dotenv(
    root: &Path,
) -> std::result::Result<DotenvCollection, CollectionFailure> {
    let mut catalog = SecretCatalog::new();
    let mut refusals = Vec::new();
    let mut skipped_symlinks = 0usize;
    let mut names = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DotenvCollection {
                catalog,
                skipped_symlinks,
            });
        }
        Err(error) => {
            return Err(CollectionFailure {
                refusals: vec![CollectionRefusal::Read {
                    path: root.display().to_string(),
                    message: error.to_string(),
                }],
                catalog,
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

    let mut counted_files = 0usize;
    for name in names {
        let path = root.join(&name);
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            // Never follow a symlinked source.
            skipped_symlinks += 1;
            continue;
        }
        counted_files += 1;
        if counted_files > MAX_DOTENV_FILES {
            refusals.push(CollectionRefusal::TooManyFiles {
                limit: MAX_DOTENV_FILES,
            });
            break;
        }
        let (content, refusal) = read_bounded(&path);
        if let Some(refusal) = refusal {
            refusals.push(refusal);
        }
        for (key, value) in parse_dotenv(&content) {
            let result = if super::is_credential_name(&key) {
                catalog.register_credential(&key, &value)
            } else {
                catalog.register_plain(&value)
            };
            if let Err(refusal) = result {
                refusals.push(registration_refusal(&key, refusal));
            }
        }
    }
    if refusals.is_empty() {
        Ok(DotenvCollection {
            catalog,
            skipped_symlinks,
        })
    } else {
        Err(CollectionFailure { refusals, catalog })
    }
}

fn registration_refusal(name: &str, refusal: RegistrationRefusal) -> CollectionRefusal {
    match refusal {
        RegistrationRefusal::TooShort { .. } => CollectionRefusal::CredentialTooShort {
            name: name.to_string(),
            limit: MIN_GENERIC_SECRET_LEN,
        },
        RegistrationRefusal::Reserved { .. } => CollectionRefusal::ReservedValue {
            name: name.to_string(),
        },
        RegistrationRefusal::CatalogFull { limit } => CollectionRefusal::TooManyValues { limit },
    }
}

/// Read a dotenv source with a bounded, no-follow open.
///
/// Over the cap it still returns the readable prefix (cut at a line boundary)
/// so the secrets at the head of the file are registered, together with the
/// refusal.
fn read_bounded(path: &Path) -> (String, Option<CollectionRefusal>) {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) => {
            return (
                String::new(),
                Some(CollectionRefusal::Read {
                    path: path.display().to_string(),
                    message: error.to_string(),
                }),
            );
        }
    };
    let (mut bytes, mut refusal) = (Vec::new(), None);
    if let Err(error) = file
        .take(MAX_DOTENV_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
    {
        return (
            String::new(),
            Some(CollectionRefusal::Read {
                path: path.display().to_string(),
                message: error.to_string(),
            }),
        );
    }
    if bytes.len() > MAX_DOTENV_BYTES {
        bytes.truncate(MAX_DOTENV_BYTES);
        if let Some(newline) = bytes.iter().rposition(|byte| *byte == b'\n') {
            bytes.truncate(newline + 1);
        }
        refusal = Some(CollectionRefusal::FileTooLarge {
            path: path.display().to_string(),
            limit: MAX_DOTENV_BYTES,
        });
    }
    let content = String::from_utf8_lossy(&bytes).into_owned();
    (content, refusal)
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
