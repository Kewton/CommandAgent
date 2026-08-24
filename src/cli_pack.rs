use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;
use thiserror::Error;

use crate::cli::Cli;
use crate::config::Config;
use crate::planner::pack::PackIntent;
use crate::planner::pack::catalog::{self, PackSource};

const PACK_DIRECTORY_ENV: &str = "COMMANDAGENT_PACK_DIRECTORY";
const PACK_ID_ENV: &str = "COMMANDAGENT_PACK_ID";
const PACK_VERSION_ENV: &str = "COMMANDAGENT_PACK_VERSION";
const PACK_HASH_ENV: &str = "COMMANDAGENT_PACK_HASH";
const PACK_PIN_FILE: &str = "pack.sha256";
const PACK_ENV_KEYS: [&str; 4] = [
    PACK_DIRECTORY_ENV,
    PACK_ID_ENV,
    PACK_VERSION_ENV,
    PACK_HASH_ENV,
];
static PACK_ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Error)]
#[error("{message}")]
pub(crate) struct PackCliError {
    message: String,
}

impl PackCliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SelectionSource {
    ExtensionRoot,
    Repository,
}

impl SelectionSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ExtensionRoot => "extension_root",
            Self::Repository => "repository",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ResolvedPack {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) hash: String,
    pub(crate) source: SelectionSource,
    #[serde(skip)]
    pub(crate) directory: PathBuf,
}

pub(crate) fn resolve(cli: &Cli, config: &Config) -> anyhow::Result<Option<ResolvedPack>> {
    resolve_inner(cli, config).map_err(Into::into)
}

pub(crate) fn resolve_for_doctor(
    cli: &Cli,
    config: &Config,
) -> Result<Option<ResolvedPack>, String> {
    resolve_inner(cli, config).map_err(|error| error.to_string())
}

fn resolve_inner(cli: &Cli, config: &Config) -> Result<Option<ResolvedPack>, PackCliError> {
    let preset_pack =
        crate::config::selected_preset_pack(&config.workspace_root, cli.preset.as_deref())
            .map_err(|error| PackCliError::new(format!("resolve preset pack: {error:#}")))?;
    if let (Some(flag), Some(preset)) = (cli.pack.as_deref(), preset_pack.as_deref())
        && flag.trim() != preset.trim()
    {
        return Err(PackCliError::new(format!(
            "--pack `{}` contradicts preset pack `{}`",
            flag.trim(),
            preset.trim()
        )));
    }
    let selector = cli
        .pack
        .as_deref()
        .or(preset_pack.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(selector) = selector else {
        if cli.pack_hash.is_some() {
            return Err(PackCliError::new("--pack-hash requires --pack"));
        }
        return Ok(None);
    };
    let (id, version) = parse_selector(selector)?;
    let configured_extension_root =
        crate::config::configured_extension_root(&config.workspace_root)
            .map_err(|error| PackCliError::new(format!("resolve extension_root: {error:#}")))?;
    let extension_root = cli.extension_root.clone().or(configured_extension_root);
    let (directory, source) = locate(
        &config.workspace_root,
        extension_root.as_deref(),
        id,
        version,
    )?;
    if crate::planner::pack::catalog::is_retired(&directory) {
        return Err(PackCliError::new(format!(
            "pack `{id}@{version}` is retired and cannot be selected"
        )));
    }
    let loaded = crate::planner::pack::load_directory(&directory).map_err(|error| {
        PackCliError::new(format!(
            "load selected pack {}: {error}",
            directory.display()
        ))
    })?;
    if loaded.id() != id || loaded.identity.version != version {
        return Err(PackCliError::new(format!(
            "selected directory identity is {}@{}, not {id}@{version}",
            loaded.id(),
            loaded.identity.version
        )));
    }
    let pin_path = directory.join(PACK_PIN_FILE);
    let pin = std::fs::read_to_string(&pin_path).map_err(|error| {
        PackCliError::new(format!(
            "selected pack has no readable pin {}: {error}",
            pin_path.display()
        ))
    })?;
    if pin.trim() != loaded.hash {
        return Err(PackCliError::new(format!(
            "selected pack hash mismatch: pack.sha256 contains `{}`, observed `{}`",
            pin.trim(),
            loaded.hash
        )));
    }
    if let Some(expected) = cli.pack_hash.as_deref()
        && expected.trim() != loaded.hash
    {
        return Err(PackCliError::new(format!(
            "--pack-hash mismatch: expected `{}`, observed `{}`",
            expected.trim(),
            loaded.hash
        )));
    }
    validate_compatibility(config, &loaded, source)?;
    crate::planner::pack::conform(&loaded)
        .map_err(|error| PackCliError::new(format!("selected pack conformance failed: {error}")))?;
    Ok(Some(ResolvedPack {
        id: id.to_string(),
        version: version.to_string(),
        hash: loaded.hash,
        source,
        directory,
    }))
}

fn parse_selector(selector: &str) -> Result<(&str, &str), PackCliError> {
    let Some((id, version)) = selector.split_once('@') else {
        return Err(PackCliError::new(format!(
            "pack selector `{selector}` must pin id@MAJOR.MINOR.PATCH"
        )));
    };
    if id.is_empty() || version.is_empty() || version.contains('@') {
        return Err(PackCliError::new(format!(
            "pack selector `{selector}` must pin one id@MAJOR.MINOR.PATCH"
        )));
    }
    if id.len() > 64
        || !id.as_bytes()[0].is_ascii_lowercase()
        || id.split('-').any(|segment| {
            segment.is_empty()
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
    {
        return Err(PackCliError::new(format!(
            "pack selector `{selector}` has an invalid pack id"
        )));
    }
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return Err(PackCliError::new(format!(
            "pack selector `{selector}` must pin id@MAJOR.MINOR.PATCH"
        )));
    }
    Ok((id, version))
}

fn locate(
    repository_root: &Path,
    extension_root: Option<&Path>,
    id: &str,
    version: &str,
) -> Result<(PathBuf, SelectionSource), PackCliError> {
    if let Some(root) = extension_root {
        for candidate in [
            root.join(id).join(version),
            root.join("packs").join(id).join(version),
        ] {
            if candidate.is_dir() {
                return Ok((candidate, SelectionSource::ExtensionRoot));
            }
        }
    }
    let repository = repository_root.join("packs").join(id).join(version);
    if repository.is_dir() {
        return Ok((repository, SelectionSource::Repository));
    }
    Err(PackCliError::new(format!(
        "pack `{id}@{version}` was not found in the extension root or repository"
    )))
}

fn validate_compatibility(
    config: &Config,
    loaded: &crate::planner::pack::LoadedPack,
    source: SelectionSource,
) -> Result<(), PackCliError> {
    let expected_profile = crate::planner::profile_descriptor::pack_profile_for_name(
        &config.profile,
    )
    .ok_or_else(|| {
        PackCliError::new(format!(
            "profile `{}` cannot activate an assist/eval pack",
            config.profile
        ))
    })?;
    let intent = config.resolved_run_intent().as_str();
    let expected_intent = PackIntent::parse(intent)
        .ok_or_else(|| PackCliError::new(format!("intent `{intent}` cannot activate a pack")))?;
    let source = match source {
        SelectionSource::ExtensionRoot => PackSource::Local,
        SelectionSource::Repository => PackSource::Repository,
    };
    if !catalog::profile_is_compatible(source, &config.profile, loaded.identity.profile)
        || loaded.identity.intent != expected_intent
    {
        return Err(PackCliError::new(format!(
            "selected pack is for {} × {}, not {} × {}",
            loaded.identity.profile, loaded.identity.intent, expected_profile, expected_intent
        )));
    }
    Ok(())
}

pub(crate) struct RuntimeEnvironmentGuard {
    _lock: Option<MutexGuard<'static, ()>>,
    previous: [Option<OsString>; 4],
}

impl RuntimeEnvironmentGuard {
    pub(crate) fn install(selection: Option<&ResolvedPack>) -> anyhow::Result<Self> {
        let Some(selection) = selection else {
            return Ok(Self {
                _lock: None,
                previous: [None, None, None, None],
            });
        };
        let lock = PACK_ENVIRONMENT_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("pack runtime environment lock is poisoned"))?;
        let previous = PACK_ENV_KEYS.map(std::env::var_os);
        let values = [
            selection.directory.as_os_str().to_os_string(),
            OsString::from(&selection.id),
            OsString::from(&selection.version),
            OsString::from(&selection.hash),
        ];
        // SAFETY: the process-wide lock serializes this scoped mutation, and the
        // guard remains alive for the entire command before restoring all keys.
        unsafe {
            for (key, value) in PACK_ENV_KEYS.into_iter().zip(values.iter()) {
                std::env::set_var(key, value);
            }
        }
        Ok(Self {
            _lock: Some(lock),
            previous,
        })
    }
}

impl Drop for RuntimeEnvironmentGuard {
    fn drop(&mut self) {
        if self._lock.is_none() {
            return;
        }
        // SAFETY: this guard still owns the process-wide mutation lock.
        unsafe {
            for (key, previous) in PACK_ENV_KEYS.into_iter().zip(self.previous.iter()) {
                if let Some(value) = previous {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_requires_an_exact_core_version() {
        assert_eq!(
            parse_selector("nextjs-acme@1.0.0").unwrap(),
            ("nextjs-acme", "1.0.0")
        );
        for invalid in [
            "nextjs-acme",
            "nextjs-acme@1",
            "nextjs-acme@1.0",
            "nextjs-acme@01.0.0",
            "../nextjs-acme@1.0.0",
        ] {
            assert!(parse_selector(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn extension_root_precedes_repository_for_the_same_identity() {
        let temp = tempfile::tempdir().unwrap();
        let repository = temp.path().join("repository");
        let extension = temp.path().join("extension");
        std::fs::create_dir_all(repository.join("packs/acme/1.0.0")).unwrap();
        std::fs::create_dir_all(extension.join("acme/1.0.0")).unwrap();
        let (path, source) = locate(&repository, Some(&extension), "acme", "1.0.0").unwrap();
        assert_eq!(path, extension.join("acme/1.0.0"));
        assert_eq!(source, SelectionSource::ExtensionRoot);
    }
}
