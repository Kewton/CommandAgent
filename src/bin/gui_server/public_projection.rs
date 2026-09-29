use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use commandagent::sensitive_data::{self, RedactionContext, SecretCatalog, SecretScrubError};
use commandagent::tui::boundary_shell::confirmation::ConfirmationIdentity;

pub(super) const EXECUTION_ROOT_LABEL: &str = "<execution-root>";

/// Built-in provider keys the run registers as credentials. Mirrors the shared
/// config sources so the GUI protects the same values a run would.
const BUILT_IN_PROVIDER_KEYS: [&str; 3] =
    ["OPENAI_API_KEY", "GEMINI_API_KEY", "LM_STUDIO_API_TOKEN"];

pub(super) fn identity(
    identity: &ConfirmationIdentity,
    execution_root: &Path,
) -> ConfirmationIdentity {
    let mut projected = identity.clone();
    projected.workspace = text(&identity.workspace, execution_root);
    projected
}

pub(super) fn text(value: impl Into<String>, execution_root: &Path) -> String {
    let execution_root = execution_root.to_string_lossy();
    let mut projected = value
        .into()
        .replace(execution_root.as_ref(), EXECUTION_ROOT_LABEL);
    if let Some(alias) = execution_root.strip_prefix("/private/") {
        projected = projected.replace(&format!("/{alias}"), EXECUTION_ROOT_LABEL);
    }
    projected
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

/// Replace the execution root and scrub every registered secret from free text.
pub(super) fn redact_text(value: impl Into<String>, root: &Path) -> String {
    let context = context(root);
    context.scrub_text(&text(value, root))
}

/// Replace the execution root and remove registered secrets from a document
/// body while keeping a fixed schema intact.
///
/// A JSON object/array keeps its fixed `event`/`schema_version`/`status`/
/// `verdict`/`type` identifiers and is refused honestly — naming the position,
/// never the value — when a dynamic key would expose a secret. A JSONL body
/// keeps its per-line schema. Any other body falls back to an exact-value text
/// scrub, which is idempotent and preserves non-secret content.
pub(super) fn redact_document(content: &str, root: &Path) -> Result<String, SecretScrubError> {
    let replaced = text(content, root);
    let context = context(root);
    if context.is_empty() {
        return Ok(replaced);
    }
    if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&replaced)
        && is_container(&value)
    {
        context.scrub_value(&mut value)?;
        return Ok(serde_json::to_string(&value).unwrap_or(replaced));
    }
    let lines = replaced.lines().collect::<Vec<_>>();
    let has_content = lines.iter().any(|line| !line.trim().is_empty());
    let json_lines = has_content
        && lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .all(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .is_ok_and(|value| is_container(&value))
            });
    if json_lines {
        let mut scrubbed = Vec::with_capacity(lines.len());
        for line in lines {
            if line.trim().is_empty() {
                scrubbed.push(line.to_string());
                continue;
            }
            let mut value: serde_json::Value =
                serde_json::from_str(line).expect("each non-empty line parsed as a JSON container");
            context.scrub_value(&mut value)?;
            scrubbed.push(serde_json::to_string(&value).unwrap_or_else(|_| line.to_string()));
        }
        let mut out = scrubbed.join("\n");
        if replaced.ends_with('\n') {
            out.push('\n');
        }
        return Ok(out);
    }
    Ok(context.scrub_text(&replaced))
}

fn is_container(value: &serde_json::Value) -> bool {
    matches!(
        value,
        serde_json::Value::Object(_) | serde_json::Value::Array(_)
    )
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
