//! Run-scoped secret value catalog and the single exact-value scrub.
//!
//! Issue #504 owns this leaf. It replaces format-guessing redaction with one
//! exact-value scrub shared by every persistence and display boundary. #543
//! and #544 consume the catalog/context, the whole-value scrub, the stream
//! carry, and the runnable-YAML/identity refusal declared here.
//!
//! Design invariants:
//! - Values are registered per run. A process-global singleton never merges
//!   one workspace's secrets into another's catalog; the registry is keyed by
//!   the scope (workspace root and events path) that registered it, and the
//!   minimal-loop entry resolves its own workspace's scope, never another
//!   run's thread-local one.
//! - [`SecretCatalog`] never prints its values through `Debug`.
//! - Only values at least [`MIN_GENERIC_SECRET_LEN`] bytes long are secrets. A
//!   shorter credential value is refused at the source; a shorter non-secret is
//!   simply not registered.
//! - Every registered value is replaced by an exact match with a marker chosen
//!   so it contains no registered value; re-applying the scrub is idempotent.

mod redaction;
#[cfg(test)]
mod tests;

pub use redaction::{
    CollectionFailure, CollectionRefusal, DotenvCollection, RegistrationRefusal,
    RunnableSecretRefusal, SecretScrubError, StreamScrubber, collect_scoped_dotenv, parse_dotenv,
    refuse_identity, refuse_runnable, runnable_yaml_contains_secret, scrub_yaml_value,
};

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// The default marker the exact-value scrub substitutes when no registered
/// value collides with it. Use [`SecretCatalog::marker`] for the actual marker.
pub const REDACTED: &str = "<redacted>";

/// Minimum length for a value to be treated as a secret. A shorter credential
/// value is refused at the source instead of being registered, so it never
/// corrupts a fixed identifier.
pub const MIN_GENERIC_SECRET_LEN: usize = 8;

/// Bounded root dotenv collection limits (mirrors the corpus contract).
pub const MAX_DOTENV_FILES: usize = 64;
pub const MAX_DOTENV_BYTES: usize = 1024 * 1024;
pub const MAX_CATALOG_VALUES: usize = 1024;

/// Candidate markers, tried in order; the first that contains no registered
/// value wins. The fallback is a single character no registered value contains.
const MARKER_CANDIDATES: [&str; 4] = ["<redacted>", "[redacted]", "[hidden]", "«hidden»"];

/// True when `value` equals one of the reserved marker candidates. A credential
/// value equal to one of these is refused at the source.
pub fn is_marker_candidate(value: &str) -> bool {
    MARKER_CANDIDATES.contains(&value)
}

/// Case-insensitive credential-name predicate used to decide that a value is a
/// credential (a secret) rather than an ordinary dotenv value.
pub fn is_credential_name(name: &str) -> bool {
    let upper = name.trim().to_ascii_uppercase();
    [
        "TOKEN",
        "API_KEY",
        "APIKEY",
        "API-KEY",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "CREDENTIAL",
        "PRIVATE",
        "ACCESS_KEY",
    ]
    .iter()
    .any(|needle| upper.contains(needle))
}

/// The set of exact secret values owned by one run.
///
/// Duplicate removal, longest-first ordering, and case-sensitive byte
/// equality keep the replacement deterministic and UTF-8 safe.
#[derive(Clone)]
pub struct SecretCatalog {
    values: Vec<String>,
    marker: String,
}

impl Default for SecretCatalog {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            marker: REDACTED.to_string(),
        }
    }
}

impl SecretCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// The marker actually used for this catalog. It never contains a
    /// registered value.
    pub fn marker(&self) -> &str {
        &self.marker
    }

    /// Longest registered value in bytes. Zero when the catalog is empty.
    pub fn max_value_len(&self) -> usize {
        self.values.first().map(String::len).unwrap_or(0)
    }

    pub fn values(&self) -> &[String] {
        &self.values
    }

    /// Register a value leniently: a value shorter than
    /// [`MIN_GENERIC_SECRET_LEN`] is not a secret and is not registered. Used by
    /// tests and by forced registration; the config path uses the checked
    /// [`Self::register_credential`] / [`Self::register_plain`].
    pub fn register(&mut self, value: &str) {
        let value = value.trim();
        if value.is_empty() || value.len() < MIN_GENERIC_SECRET_LEN {
            return;
        }
        let _ = self.try_push(value);
    }

    /// Register a credential value. A value shorter than
    /// [`MIN_GENERIC_SECRET_LEN`] or equal to a reserved marker is refused.
    pub fn register_credential(
        &mut self,
        name: &str,
        value: &str,
    ) -> Result<(), RegistrationRefusal> {
        let value = value.trim();
        if value.is_empty() {
            return Ok(());
        }
        if value.len() < MIN_GENERIC_SECRET_LEN {
            return Err(RegistrationRefusal::TooShort {
                name: name.to_string(),
            });
        }
        if is_marker_candidate(value) {
            return Err(RegistrationRefusal::Reserved {
                name: name.to_string(),
            });
        }
        self.try_push(value)
    }

    /// Register a non-credential value only when it can be a secret: at least
    /// [`MIN_GENERIC_SECRET_LEN`] bytes and not a reserved marker. Anything else
    /// is simply not a secret.
    pub fn register_plain(&mut self, value: &str) -> Result<(), RegistrationRefusal> {
        let value = value.trim();
        if value.is_empty() || value.len() < MIN_GENERIC_SECRET_LEN || is_marker_candidate(value) {
            return Ok(());
        }
        self.try_push(value)
    }

    /// Register a value from a named source using the credential/non-credential
    /// policy, ignoring any refusal.
    pub fn register_named(&mut self, name: &str, value: &str) {
        if is_credential_name(name) {
            let _ = self.register_credential(name, value);
        } else {
            let _ = self.register_plain(value);
        }
    }

    /// Merge another catalog's values, refusing on overflow.
    pub fn try_merge(&mut self, other: &SecretCatalog) -> Result<(), RegistrationRefusal> {
        for value in &other.values {
            self.try_push(value)?;
        }
        Ok(())
    }

    /// Merge another catalog's values, dropping silently on overflow.
    pub fn merge(&mut self, other: &SecretCatalog) {
        for value in &other.values {
            let _ = self.try_push(value);
        }
    }

    /// True when any registered value occurs in `text`.
    pub fn contains(&self, text: &str) -> bool {
        self.values
            .iter()
            .any(|value| text.contains(value.as_str()))
    }

    /// Byte offset of the first registered value in `text`, without revealing it.
    pub fn first_match(&self, text: &str) -> Option<usize> {
        self.values
            .iter()
            .filter_map(|value| text.find(value.as_str()))
            .min()
    }

    /// Replace every registered value with the marker.
    pub fn scrub(&self, text: &str) -> String {
        redaction::scrub_text(self, text)
    }

    /// Replace registered values inside an object key.
    pub fn scrub_key(&self, key: &str) -> String {
        redaction::scrub_key(self, key)
    }

    /// Strict recursive scrub of a JSON value. Refuses (without leaking) a
    /// dynamic key that contains a secret, so callers that cannot project the
    /// value safely can treat the result as a failure.
    pub fn scrub_value(&self, value: &mut serde_json::Value) -> Result<(), SecretScrubError> {
        if self.is_empty() {
            return Ok(());
        }
        redaction::scrub_value_strict(self, value)
    }

    /// Lenient recursive scrub used at boundaries that cannot fail. A dynamic
    /// key that contains a secret is projected to a distinct safe key so no
    /// evidence is lost and unrelated keys are left unchanged.
    pub fn scrub_value_lenient(&self, value: &mut serde_json::Value) {
        if self.is_empty() {
            return;
        }
        redaction::scrub_value_lenient(self, value);
    }

    /// Scrub a YAML value in place (all string scalars/keys).
    pub fn scrub_yaml(&self, value: &mut serde_yaml::Value) {
        if self.is_empty() {
            return;
        }
        redaction::scrub_yaml_value(self, value);
    }

    /// A redaction context sharing this catalog behind an `Arc`.
    pub fn context(self: &Arc<Self>) -> RedactionContext {
        RedactionContext {
            catalog: Arc::clone(self),
        }
    }

    fn try_push(&mut self, value: &str) -> Result<(), RegistrationRefusal> {
        if self.values.iter().any(|existing| existing == value) {
            return Ok(());
        }
        if self.values.len() >= MAX_CATALOG_VALUES {
            return Err(RegistrationRefusal::CatalogFull {
                limit: MAX_CATALOG_VALUES,
            });
        }
        self.values.push(value.to_string());
        self.values
            .sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
        self.refresh_marker();
        Ok(())
    }

    /// Choose a marker that contains no registered value. The candidates are
    /// tried in order, then U+FFFD, then any other code point, so the final
    /// marker is collision-free for any finite catalog.
    fn refresh_marker(&mut self) {
        for candidate in MARKER_CANDIDATES {
            if !self
                .values
                .iter()
                .any(|value| candidate.contains(value.as_str()))
            {
                self.marker = candidate.to_string();
                return;
            }
        }
        if !self.values.iter().any(|value| value.contains('\u{fffd}')) {
            self.marker = "\u{fffd}".to_string();
            return;
        }
        for code in 1u32..=0x10ffff {
            if (0xd800..=0xdfff).contains(&code) {
                continue;
            }
            if let Some(character) = char::from_u32(code) {
                let candidate = character.to_string();
                if !self.values.iter().any(|value| value.contains(&candidate)) {
                    self.marker = candidate;
                    return;
                }
            }
        }
        self.marker = "\u{fffd}\u{fffe}".to_string();
    }
}

impl std::fmt::Debug for SecretCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.values.len();
        formatter
            .debug_struct("SecretCatalog")
            .field("values", &format_args!("<{len} redacted value(s)>"))
            .finish()
    }
}

/// A shareable handle to one run's catalog. Cheap to clone and safe to send to
/// provider worker threads.
#[derive(Clone, Default)]
pub struct RedactionContext {
    catalog: Arc<SecretCatalog>,
}

impl RedactionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_catalog(catalog: SecretCatalog) -> Self {
        Self {
            catalog: Arc::new(catalog),
        }
    }

    pub fn catalog(&self) -> &SecretCatalog {
        &self.catalog
    }

    /// The shared catalog handle, for consumers that need to send it to a
    /// worker thread.
    pub fn catalog_arc(&self) -> Arc<SecretCatalog> {
        Arc::clone(&self.catalog)
    }

    pub fn is_empty(&self) -> bool {
        self.catalog.is_empty()
    }

    pub fn marker(&self) -> &str {
        self.catalog.marker()
    }

    pub fn scrub_text(&self, text: &str) -> String {
        self.catalog.scrub(text)
    }

    pub fn scrub_key(&self, key: &str) -> String {
        self.catalog.scrub_key(key)
    }

    /// The standard strict scrub. A dynamic key that cannot be projected safely
    /// is refused rather than corrupted.
    pub fn scrub_value(&self, value: &mut serde_json::Value) -> Result<(), SecretScrubError> {
        self.catalog.scrub_value(value)
    }

    /// The lenient scrub for boundaries that cannot fail.
    pub fn scrub_value_lenient(&self, value: &mut serde_json::Value) {
        self.catalog.scrub_value_lenient(value);
    }

    pub fn contains(&self, text: &str) -> bool {
        self.catalog.contains(text)
    }

    /// A chunk-boundary-safe stream scrubber over this catalog.
    pub fn stream_scrubber(&self) -> StreamScrubber {
        StreamScrubber::new(Arc::clone(&self.catalog))
    }

    /// Refuses a runnable YAML/command value that contains a secret.
    pub fn refuse_runnable(&self, field: &str, text: &str) -> Result<(), RunnableSecretRefusal> {
        redaction::refuse_runnable(&self.catalog, field, text)
    }

    /// Refuses a confirmation identity that still contains a secret.
    pub fn refuse_identity(&self, field: &str, text: &str) -> Result<(), RunnableSecretRefusal> {
        redaction::refuse_identity(&self.catalog, field, text)
    }
}

impl std::fmt::Debug for RedactionContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RedactionContext")
            .field("catalog", &self.catalog)
            .finish()
    }
}

thread_local! {
    static CURRENT: RefCell<Option<RedactionContext>> = const { RefCell::new(None) };
}

fn registry() -> &'static Mutex<BTreeMap<String, RedactionContext>> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<String, RedactionContext>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn scope_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Install `catalog` as the active run scope, keyed by the workspace root and
/// events path so cross-thread `emit` calls can resolve it.
///
/// The thread-local current scope is what path-less snippet builders consult;
/// a provider worker thread must install its own captured context explicitly.
pub fn install_scope(
    catalog: SecretCatalog,
    workspace_root: Option<&Path>,
    events_path: Option<&Path>,
) -> RedactionContext {
    let context = RedactionContext::from_catalog(catalog);
    if let Some(root) = workspace_root {
        registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(scope_key(root), context.clone());
    }
    if let Some(path) = events_path {
        registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(scope_key(path), context.clone());
    }
    set_current(Some(context.clone()));
    context
}

/// Install a pre-built context as the thread-local current scope.
pub fn set_current(context: Option<RedactionContext>) {
    CURRENT.with(|current| *current.borrow_mut() = context);
}

/// The context active on this thread, if a run installed one.
pub fn current() -> Option<RedactionContext> {
    CURRENT.with(|current| current.borrow().clone())
}

/// The context registered for `path`, without any thread-local fallback. Used
/// by the run entry to reuse the scope that belongs to its own workspace.
pub fn registered_for(path: Option<&Path>) -> Option<RedactionContext> {
    let path = path?;
    let key = scope_key(path);
    registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&key)
        .cloned()
}

/// The context registered for `path` (or the workspace root), falling back to
/// [`current`]. This lets a product thread that never installed the scope find
/// it by the source it was handed.
pub fn active_for(path: Option<&Path>) -> Option<RedactionContext> {
    registered_for(path).or_else(current)
}

/// Scrub `text` with the active run scope when one exists.
pub fn scrub_active(text: &str) -> String {
    current()
        .map(|context| context.scrub_text(text))
        .unwrap_or_else(|| text.to_string())
}

/// The active run scope's marker, or the default when no scope is installed.
pub fn active_marker() -> String {
    current()
        .map(|context| context.marker().to_string())
        .unwrap_or_else(|| REDACTED.to_string())
}

/// Scrub with the staged config catalog and then the active run scope. Used
/// when a rejected config's error chain may embed a `${ENV}` expansion value.
pub fn scrub_everything(text: &str) -> String {
    scrub_active(&config_staging::scrub(text))
}

/// Remove every installed scope. Test-only; production never clears scopes.
#[doc(hidden)]
pub fn reset_scopes_for_tests() {
    registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    set_current(None);
}

/// Staging for config secrets registered before `Config` is validated.
///
/// The CLI begins staging at the top of config resolution, so a `${ENV}`
/// expansion value is protected before the same resolution can reject an
/// invalid `base_url`. Values are moved into an installed scope only when the
/// config is accepted.
pub(crate) mod config_staging {
    use super::{RegistrationRefusal, SecretCatalog};
    use std::cell::RefCell;

    thread_local! {
        static STAGING: RefCell<Option<SecretCatalog>> = const { RefCell::new(None) };
    }

    /// Begin staging, discarding any previous unfinished staging.
    pub fn begin() {
        STAGING.with(|staging| *staging.borrow_mut() = Some(SecretCatalog::new()));
    }

    /// Stage a credential value; a too-short or reserved value is refused.
    pub fn register_credential(name: &str, value: &str) -> Result<(), RegistrationRefusal> {
        STAGING.with(|staging| match staging.borrow_mut().as_mut() {
            Some(catalog) => catalog.register_credential(name, value),
            None => Ok(()),
        })
    }

    /// Stage a non-credential value under the plain policy.
    pub fn register_plain(value: &str) -> Result<(), RegistrationRefusal> {
        STAGING.with(|staging| match staging.borrow_mut().as_mut() {
            Some(catalog) => catalog.register_plain(value),
            None => Ok(()),
        })
    }

    /// Merge a whole catalog into staging, refusing on overflow.
    pub fn try_merge(catalog: &SecretCatalog) -> Result<(), RegistrationRefusal> {
        STAGING.with(|staging| match staging.borrow_mut().as_mut() {
            Some(staging) => staging.try_merge(catalog),
            None => Ok(()),
        })
    }

    /// Take the staged catalog out of staging.
    pub fn take() -> SecretCatalog {
        STAGING.with(|staging| staging.borrow_mut().take().unwrap_or_default())
    }

    /// Scrub `text` with the staged catalog without consuming it.
    pub fn scrub(text: &str) -> String {
        STAGING.with(|staging| match staging.borrow().as_ref() {
            Some(catalog) => catalog.scrub(text),
            None => text.to_string(),
        })
    }
}
