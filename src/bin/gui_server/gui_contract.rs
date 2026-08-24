use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context, bail};
use serde::Deserialize;

pub(super) const MANIFEST_FILE: &str = "commandagent-gui-contract.json";
const MANIFEST_SCHEMA: &str = "commandagent.gui-contract/v1";
const COMPILED_MANIFEST: &str = include_str!("../../../gui/public/commandagent-gui-contract.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    contract_version: String,
}

pub(super) fn server_contract_version() -> &'static str {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    &MANIFEST
        .get_or_init(|| {
            parse_manifest(COMPILED_MANIFEST)
                .expect("the compiled GUI contract manifest must be valid")
        })
        .contract_version
}

pub(super) fn export_contract_version(static_root: &Path) -> anyhow::Result<String> {
    let path = static_root.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read GUI contract manifest {}", path.display()))?;
    parse_manifest(&text)
        .with_context(|| format!("parse GUI contract manifest {}", path.display()))
        .map(|manifest| manifest.contract_version)
}

fn parse_manifest(text: &str) -> anyhow::Result<Manifest> {
    let manifest: Manifest = serde_json::from_str(text)?;
    if manifest.schema_version != MANIFEST_SCHEMA {
        bail!(
            "unsupported GUI contract schema {:?}; expected {MANIFEST_SCHEMA}",
            manifest.schema_version
        );
    }
    if manifest.contract_version.trim().is_empty() {
        bail!("GUI contract version must not be empty");
    }
    Ok(manifest)
}
