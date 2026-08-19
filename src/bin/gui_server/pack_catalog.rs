use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use commandagent::planner::pack::catalog::{PackSource, admitted_packs};
use serde::Serialize;

const PACK_PIN_FILE: &str = "pack.sha256";
const RETIRED_FILE: &str = "RETIRED";

#[derive(Debug, Serialize)]
pub struct PackSummary {
    id: String,
    version: String,
    path: String,
    profile: Option<String>,
    intent: Option<String>,
    source: PackSource,
    source_label: &'static str,
    pin: String,
    expected_hash: Option<String>,
    observed_hash: Option<String>,
    hash_matches_pin: bool,
    has_assist: bool,
    has_eval: bool,
    retired: bool,
    shadowing_repository: bool,
    trial_eligible: bool,
    warning: Option<String>,
}

pub async fn list(
    repository_root: PathBuf,
    extension_root: Option<PathBuf>,
) -> Result<Vec<PackSummary>, String> {
    tokio::task::spawn_blocking(move || list_sync(&repository_root, extension_root.as_deref()))
        .await
        .map_err(|error| format!("join pack catalog task: {error}"))?
}

fn list_sync(
    repository_root: &Path,
    extension_root: Option<&Path>,
) -> Result<Vec<PackSummary>, String> {
    let mut resolved = BTreeMap::new();
    for mut pack in discover(repository_root, PackSource::Repository, "packs")? {
        classify_repository(&mut pack);
        resolved.insert((pack.id.clone(), pack.version.clone()), pack);
    }
    if let Some(extension_root) = extension_root {
        for mut pack in discover(extension_root, PackSource::Local, "extension-root/packs")? {
            let key = (pack.id.clone(), pack.version.clone());
            pack.shadowing_repository = resolved.contains_key(&key);
            finalize_warning(&mut pack);
            resolved.insert(key, pack);
        }
    }
    Ok(resolved.into_values().collect())
}

fn discover(
    root: &Path,
    source: PackSource,
    display_root: &str,
) -> Result<Vec<PackSummary>, String> {
    let packs_root = root.join("packs");
    if !packs_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    for id_entry in directory_entries(&packs_root)? {
        if !is_directory(&id_entry)? {
            continue;
        }
        let id = file_name(&id_entry)?;
        for version_entry in directory_entries(&id_entry)? {
            if !is_directory(&version_entry)? {
                continue;
            }
            let version = file_name(&version_entry)?;
            rows.push(inspect(
                &version_entry,
                id.clone(),
                version,
                source,
                display_root,
            ));
        }
    }
    Ok(rows)
}

fn inspect(
    directory: &Path,
    id: String,
    version: String,
    source: PackSource,
    display_root: &str,
) -> PackSummary {
    let expected_hash = std::fs::read_to_string(directory.join(PACK_PIN_FILE))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let loaded = commandagent::planner::pack::load_directory(directory).ok();
    let observed_hash = loaded.as_ref().map(|pack| pack.hash.clone());
    let identity_matches_path = loaded
        .as_ref()
        .is_some_and(|pack| pack.id() == id && pack.identity.version == version);
    let hash_matches_pin = identity_matches_path
        && expected_hash
            .as_ref()
            .zip(observed_hash.as_ref())
            .is_some_and(|(expected, observed)| expected == observed);
    let local_trial_eligible = source == PackSource::Local
        && hash_matches_pin
        && loaded
            .as_ref()
            .is_some_and(|pack| commandagent::planner::pack::conform(pack).is_ok());
    let mut row = PackSummary {
        path: format!("{display_root}/{id}/{version}"),
        profile: loaded
            .as_ref()
            .map(|pack| pack.identity.profile.as_str().to_string()),
        intent: loaded
            .as_ref()
            .map(|pack| pack.identity.intent.as_str().to_string()),
        id,
        version,
        source,
        source_label: source.japanese_label(),
        pin: expected_hash.clone().unwrap_or_default(),
        expected_hash,
        observed_hash,
        hash_matches_pin,
        has_assist: directory
            .join(commandagent::planner::pack::ASSIST_FILE)
            .is_file(),
        has_eval: directory
            .join(commandagent::planner::pack::EVAL_FILE)
            .is_file(),
        retired: directory.join(RETIRED_FILE).is_file(),
        shadowing_repository: false,
        trial_eligible: local_trial_eligible,
        warning: None,
    };
    if loaded.is_none() {
        row.warning = Some("pack の内容を解析できません。".to_string());
    } else if !identity_matches_path {
        row.warning = Some("ディレクトリ名と pack の識別子が一致しません。".to_string());
    }
    finalize_warning(&mut row);
    row
}

fn classify_repository(pack: &mut PackSummary) {
    let admitted = pack.hash_matches_pin
        && admitted_packs().iter().any(|entry| {
            entry.id == pack.id
                && entry.version == pack.version
                && Some(entry.profile) == pack.profile.as_deref()
                && Some(entry.intent) == pack.intent.as_deref()
                && Some(entry.hash) == pack.observed_hash.as_deref()
        });
    if admitted {
        pack.source = PackSource::Admitted;
        pack.source_label = PackSource::Admitted.japanese_label();
        pack.trial_eligible = !pack.retired;
    }
    finalize_warning(pack);
}

fn finalize_warning(pack: &mut PackSummary) {
    let mut warnings = pack.warning.take().into_iter().collect::<Vec<_>>();
    if pack.expected_hash.is_none() {
        warnings.push("pack.sha256 がありません。".to_string());
    } else if pack.observed_hash.is_some() && !pack.hash_matches_pin {
        warnings.push("hash と pin が一致しません。".to_string());
    }
    if pack.retired {
        warnings.push("この pack は廃止済みです。".to_string());
        pack.trial_eligible = false;
    }
    if pack.shadowing_repository {
        warnings.push("ローカル優先: 同名のリポジトリ pack より拡張ルートを優先".to_string());
    }
    warnings.sort();
    warnings.dedup();
    pack.warning = (!warnings.is_empty()).then(|| warnings.join(" "));
}

fn directory_entries(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut entries = std::fs::read_dir(directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("read {} entry: {error}", directory.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    Ok(entries)
}

fn is_directory(path: &Path) -> Result<bool, String> {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .map_err(|error| format!("inspect {}: {error}", path.display()))
}

fn file_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| format!("{} has no UTF-8 file name", path.display()))
}
