use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use commandagent::sensitive_data::{self, RedactionContext, SecretCatalog, SecretScrubError};
use commandagent::tui::boundary_shell::confirmation::{ConfirmationIdentity, PackSelection};

pub(super) const EXECUTION_ROOT_LABEL: &str = "<execution-root>";

/// Built-in provider keys the run registers as credentials. Mirrors the shared
/// config sources so the GUI protects the same values a run would.
const BUILT_IN_PROVIDER_KEYS: [&str; 3] =
    ["OPENAI_API_KEY", "GEMINI_API_KEY", "LM_STUDIO_API_TOKEN"];

/// Project a confirmation identity for display.
///
/// The projected copy scrubs every field (including the nested pins/pack/manifest
/// strings), not just the workspace. It is a display copy only: it is never
/// re-hashed and never used to authorise a dispatch.
pub(super) fn identity(
    identity: &ConfirmationIdentity,
    execution_root: &Path,
) -> ConfirmationIdentity {
    let mut projected = identity.clone();
    let redact = |value: &mut String| *value = text(std::mem::take(value), execution_root);
    redact(&mut projected.request);
    redact(&mut projected.workspace);
    redact(&mut projected.profile);
    redact(&mut projected.intent);
    redact(&mut projected.task_family);
    projected.route_bases.iter_mut().for_each(redact);
    redact(&mut projected.contract_ref);
    projected.contract_checks.iter_mut().for_each(redact);
    redact(&mut projected.band_rate);
    redact(&mut projected.band_arm);
    redact(&mut projected.band_measurement);
    redact(&mut projected.band_source);
    redact(&mut projected.full_meaning);
    redact(&mut projected.pins.planner_provider);
    redact(&mut projected.pins.planner_model);
    redact(&mut projected.pins.executor_provider);
    redact(&mut projected.pins.executor_model);
    redact(&mut projected.pins.preset);
    if let PackSelection::Pinned {
        id,
        version,
        hash,
        point,
        ..
    } = &mut projected.pack
    {
        redact(id);
        redact(version);
        redact(hash);
        redact(point);
    }
    if let Some(manifest) = projected.draft_manifest.as_mut() {
        redact(&mut manifest.source);
        redact(&mut manifest.path);
        redact(&mut manifest.hash);
        redact(&mut manifest.assurance_ceiling);
        if let Some(base) = manifest.base_profile.as_mut() {
            redact(base);
        }
    }
    projected
}

/// Replace the execution root with its fixed label and scrub every registered
/// secret from the surrounding free text.
///
/// The label itself is a fixed marker: it is split out before the scrub, so a
/// registered value that is a substring of the label cannot corrupt it, and a
/// root path that itself contains a secret is replaced first so no fragment of
/// it survives.
pub(super) fn text(value: impl Into<String>, execution_root: &Path) -> String {
    let replaced = replace_execution_root(&value.into(), execution_root);
    scrub_around_label(&context(execution_root), &replaced)
}

fn replace_execution_root(value: &str, execution_root: &Path) -> String {
    let execution_root = execution_root.to_string_lossy();
    let mut projected = value.replace(execution_root.as_ref(), EXECUTION_ROOT_LABEL);
    if let Some(alias) = execution_root.strip_prefix("/private/") {
        projected = projected.replace(&format!("/{alias}"), EXECUTION_ROOT_LABEL);
    }
    projected
}

fn scrub_around_label(context: &RedactionContext, text: &str) -> String {
    if context.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find(EXECUTION_ROOT_LABEL) {
        out.push_str(&context.scrub_text(&rest[..index]));
        out.push_str(EXECUTION_ROOT_LABEL);
        rest = &rest[index + EXECUTION_ROOT_LABEL.len()..];
    }
    out.push_str(&context.scrub_text(rest));
    out
}

fn contexts() -> &'static Mutex<BTreeMap<PathBuf, RedactionContext>> {
    static CONTEXTS: OnceLock<Mutex<BTreeMap<PathBuf, RedactionContext>>> = OnceLock::new();
    CONTEXTS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// The secret catalog that belongs to `root`.
///
/// An explicitly registered scope for exactly this root wins (tests and any
/// future in-process wiring install one); otherwise the catalog is derived from
/// the same sources a run uses: the bounded root dotenv and the built-in
/// provider keys. The lookup is keyed by the root path and never falls back to
/// a thread-local scope, so concurrent requests for two workspaces keep their
/// own catalogs and never mix them.
pub(super) fn context(root: &Path) -> RedactionContext {
    if let Some(context) = sensitive_data::registered_for(Some(root)) {
        return context;
    }
    {
        let cached = contexts()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(context) = cached.get(root) {
            return context.clone();
        }
    }
    let context = RedactionContext::from_catalog(build_catalog(root));
    contexts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(root.to_path_buf(), context.clone());
    context
}

fn build_catalog(root: &Path) -> SecretCatalog {
    let mut catalog = SecretCatalog::new();
    match sensitive_data::collect_scoped_dotenv(root) {
        Ok(collection) => catalog.merge(&collection.catalog),
        Err(failure) => catalog.merge(&failure.catalog),
    }
    for name in BUILT_IN_PROVIDER_KEYS {
        if let Ok(value) = commandagent::config::load_api_key(root, name) {
            let _ = catalog.register_credential(name, &value);
        }
    }
    catalog
}

/// Replace the execution root and scrub every registered secret from free text
/// (a document id/path or a short status string).
pub(super) fn redact_text(value: impl Into<String>, root: &Path) -> String {
    text(value, root)
}

/// Replace the execution root and remove registered secrets from a stored
/// document body while keeping its fixed schema intact.
///
/// A JSON object/array keeps its fixed `event`/`schema_version`/`status`/
/// `verdict`/`type` identifiers; a JSONL body keeps its per-line schema. A
/// dynamic key that would expose a secret is refused honestly (naming the
/// position, never the value) so a caller that cannot project the body safely
/// fails instead of emitting a corrupt document.
pub(super) fn redact_document(content: &str, root: &Path) -> Result<String, SecretScrubError> {
    let replaced = replace_execution_root(content, root);
    let context = context(root);
    if context.is_empty() {
        return Ok(replaced);
    }
    if let Some(mut value) = parse_container(&replaced) {
        context.scrub_value(&mut value)?;
        return Ok(serde_json::to_string(&value).unwrap_or(replaced));
    }
    if let Some(lines) = parse_jsonl_containers(&replaced) {
        let mut scrubbed = Vec::with_capacity(lines.len());
        for line in lines {
            let Some(mut value) = line.value else {
                scrubbed.push(line.raw);
                continue;
            };
            context.scrub_value(&mut value)?;
            scrubbed.push(serde_json::to_string(&value).unwrap_or(line.raw));
        }
        return Ok(join_lines(&replaced, scrubbed));
    }
    Ok(context.scrub_text(&replaced))
}

/// Like [`redact_document`] but never fails: a dynamic key is projected to a
/// distinct safe key. Used at boundaries that cannot report an error.
pub(super) fn redact_document_lenient(content: &str, root: &Path) -> String {
    let replaced = replace_execution_root(content, root);
    let context = context(root);
    if context.is_empty() {
        return replaced;
    }
    if let Some(mut value) = parse_container(&replaced) {
        context.scrub_value_lenient(&mut value);
        return serde_json::to_string(&value).unwrap_or(replaced);
    }
    if let Some(lines) = parse_jsonl_containers(&replaced) {
        let scrubbed = lines
            .into_iter()
            .map(|line| match line.value {
                Some(mut value) => {
                    context.scrub_value_lenient(&mut value);
                    serde_json::to_string(&value).unwrap_or(line.raw)
                }
                None => line.raw,
            })
            .collect();
        return join_lines(&replaced, scrubbed);
    }
    context.scrub_text(&replaced)
}

/// The display projection for a session event tail or artifact body.
///
/// Key names are preserved for a fixed schema key (so `event`/`status`/`verdict`
/// are never renamed) and a key that contains a secret is projected to a
/// distinct safe key; unlike the stored-document scrub it also removes a secret
/// that sits in a fixed key's free text (`"status":"failed token=<secret>"`),
/// which the schema-preserving scrub deliberately leaves alone.
pub(super) fn redact_display(content: &str, root: &Path) -> String {
    let replaced = replace_execution_root(content, root);
    let context = context(root);
    if context.is_empty() {
        return replaced;
    }
    if let Some(mut value) = parse_container(&replaced) {
        scrub_display_value(&context, &mut value);
        return serde_json::to_string(&value).unwrap_or(replaced);
    }
    if let Some(lines) = parse_jsonl_containers(&replaced) {
        let scrubbed = lines
            .into_iter()
            .map(|line| match line.value {
                Some(mut value) => {
                    scrub_display_value(&context, &mut value);
                    serde_json::to_string(&value).unwrap_or(line.raw)
                }
                None => context.scrub_text(&line.raw),
            })
            .collect();
        return join_lines(&replaced, scrubbed);
    }
    context.scrub_text(&replaced)
}

fn scrub_display_value(context: &RedactionContext, value: &mut serde_json::Value) {
    // Key names: a fixed schema key keeps its name, a key that contains a secret
    // is projected to a distinct safe key.
    context.scrub_value_lenient(value);
    // Values: every string is scrubbed, including the free text that sits in a
    // fixed schema key.
    scrub_all_strings(context, value);
}

fn scrub_all_strings(context: &RedactionContext, value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => *text = context.scrub_text(text),
        serde_json::Value::Array(items) => {
            items
                .iter_mut()
                .for_each(|item| scrub_all_strings(context, item));
        }
        serde_json::Value::Object(map) => {
            map.values_mut()
                .for_each(|item| scrub_all_strings(context, item));
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn is_container(value: &serde_json::Value) -> bool {
    matches!(
        value,
        serde_json::Value::Object(_) | serde_json::Value::Array(_)
    )
}

fn parse_container(text: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .filter(is_container)
}

struct JsonLine {
    raw: String,
    value: Option<serde_json::Value>,
}

/// Parse a JSONL body when every non-empty line is a JSON container. Returns the
/// original line text alongside the parsed value so a line that cannot be parsed
/// is still scrubbed as text without losing the original bytes.
fn parse_jsonl_containers(text: &str) -> Option<Vec<JsonLine>> {
    let lines = text.lines().collect::<Vec<_>>();
    if !lines.iter().any(|line| !line.trim().is_empty()) {
        return None;
    }
    let mut parsed = Vec::with_capacity(lines.len());
    for line in lines {
        if line.trim().is_empty() {
            parsed.push(JsonLine {
                raw: line.to_string(),
                value: None,
            });
            continue;
        }
        let value = serde_json::from_str::<serde_json::Value>(line).ok();
        if value.as_ref().is_none_or(|value| !is_container(value)) {
            return None;
        }
        parsed.push(JsonLine {
            raw: line.to_string(),
            value,
        });
    }
    Some(parsed)
}

fn join_lines(original: &str, lines: Vec<String>) -> String {
    let mut out = lines.join("\n");
    if original.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANARY: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";

    fn scope(root: &Path) -> RedactionContext {
        let mut catalog = SecretCatalog::new();
        catalog.register(CANARY);
        sensitive_data::install_scope(catalog, Some(root), None)
    }

    #[test]
    fn redacts_every_execution_root_occurrence() {
        let root = Path::new("/private/tmp/trial-root");

        assert_eq!(
            text(
                "workspace=/private/tmp/trial-root; events=/private/tmp/trial-root/.commandagent/runs/one/events.jsonl",
                root,
            ),
            "workspace=<execution-root>; events=<execution-root>/.commandagent/runs/one/events.jsonl"
        );
    }

    #[test]
    fn redacts_the_macos_private_path_alias() {
        assert_eq!(
            text(
                "workspace=/var/folders/example/trial-root",
                Path::new("/private/var/folders/example/trial-root"),
            ),
            "workspace=<execution-root>"
        );
    }

    #[test]
    fn json_document_keeps_fixed_schema_and_scrubs_free_fields() {
        let dir = tempfile::tempdir().unwrap();
        scope(dir.path());
        let document = format!(
            "{{\"event\":\"run_stop\",\"status\":\"completed\",\"verdict\":\"pass\",\"message\":\"{CANARY}\"}}"
        );

        let redacted = redact_document(&document, dir.path()).unwrap();

        assert!(!redacted.contains(CANARY), "{redacted}");
        let value: serde_json::Value = serde_json::from_str(&redacted).unwrap();
        assert_eq!(value["event"], "run_stop");
        assert_eq!(value["status"], "completed");
        assert_eq!(value["verdict"], "pass");
        assert_eq!(value["message"], "<redacted>");
    }

    #[test]
    fn jsonl_document_scrubs_each_line_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        scope(dir.path());
        let document = format!(
            "{{\"event\":\"run_start\",\"note\":\"{CANARY}\"}}\n{{\"event\":\"run_stop\",\"status\":\"completed\"}}\n"
        );

        let redacted = redact_document(&document, dir.path()).unwrap();
        assert!(!redacted.contains(CANARY), "{redacted}");
        assert!(redacted.contains("\"status\":\"completed\""), "{redacted}");
        assert_eq!(redact_document(&redacted, dir.path()).unwrap(), redacted);
    }

    #[test]
    fn a_dynamic_key_that_would_expose_a_secret_is_refused_without_leaking() {
        let dir = tempfile::tempdir().unwrap();
        scope(dir.path());
        let document = format!("{{\"event\":\"x\",\"{CANARY}\":1}}");

        let error = redact_document(&document, dir.path()).unwrap_err();
        assert!(!error.to_string().contains(CANARY), "{error}");
    }

    #[test]
    fn two_workspaces_keep_separate_catalogs() {
        let alpha = tempfile::tempdir().unwrap();
        let beta = tempfile::tempdir().unwrap();
        let mut alpha_catalog = SecretCatalog::new();
        alpha_catalog.register("alpha-only-secret");
        sensitive_data::install_scope(alpha_catalog, Some(alpha.path()), None);
        let mut beta_catalog = SecretCatalog::new();
        beta_catalog.register("beta-only-secret");
        sensitive_data::install_scope(beta_catalog, Some(beta.path()), None);

        assert!(context(alpha.path()).contains("alpha-only-secret"));
        assert!(!context(alpha.path()).contains("beta-only-secret"));
        assert!(context(beta.path()).contains("beta-only-secret"));
        assert!(!context(beta.path()).contains("alpha-only-secret"));
    }
}
