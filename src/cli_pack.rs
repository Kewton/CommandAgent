use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;
use thiserror::Error;

use crate::cli::Cli;
use crate::config::Config;
use crate::planner::pack::PackIntent;
use crate::planner::pack::catalog::{self, PackSource};
use crate::tools::profile_tools::selection::{
    InstalledDeclaration, PackSelection, ResolvedDeclaration, install_declaration,
};

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
    /// What the verified bytes declare. `None` is unknown: admission reads them.
    #[serde(skip)]
    pub(crate) declared: Option<ResolvedDeclaration>,
}

impl ResolvedPack {
    pub(crate) fn selection(&self) -> PackSelection {
        PackSelection {
            directory: self.directory.clone(),
            id: self.id.clone(),
            version: self.version.clone(),
            hash: self.hash.clone(),
        }
    }
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
    let declared = crate::planner::pack::load_declared_directory(&directory).map_err(|error| {
        PackCliError::new(format!(
            "load selected pack {}: {error}",
            directory.display()
        ))
    })?;
    let tools = declared.profile_tools;
    let loaded = declared.pack;
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
        declared: Some(ResolvedDeclaration {
            pack_profile: loaded.identity.profile,
            tools,
        }),
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
    // Declared before the lock: it is restored while the lock is still held.
    _declared: Option<InstalledDeclaration>,
    _lock: Option<MutexGuard<'static, ()>>,
    previous: [Option<OsString>; 4],
}

impl RuntimeEnvironmentGuard {
    pub(crate) fn install(selection: Option<&ResolvedPack>) -> anyhow::Result<Self> {
        let Some(selection) = selection else {
            return Ok(Self {
                _declared: None,
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
            _declared: Some(install_declaration(
                selection.selection(),
                selection.declared.clone(),
            )),
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
pub mod tests {
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

    /// These install process-wide variables, so each runs alone in its own process.
    fn in_own_process(test: &str, body: fn()) {
        own_process::run(test, body);
    }

    /// Runs one test's body in a test process of its own, so that what the body sets in the
    /// process environment (the pack selection above all) never reaches the tests that
    /// share the parent's process. The parent itself changes nothing in its environment.
    ///
    /// The parent passes only when the child exits successfully *and* the body ran to its
    /// end: a child that ran no test (a name that `--exact` does not match) is a failure.
    pub(crate) mod own_process {
        use std::io::{BufRead, BufReader, Write};
        use std::process::{Child, Command, Stdio};

        /// Set in the child: where it writes the test name once the body has returned.
        const CHILD_ENV: &str = "COMMANDAGENT_TEST_OWN_PROCESS_RAN";
        /// Set in a child whose parent waits on its hold.
        const HOLD_ENV: &str = "COMMANDAGENT_TEST_OWN_PROCESS_HOLD";
        const HOLDING: &str = "commandagent-own-process: holding";
        const RELEASE: &str = "commandagent-own-process: release";

        /// Whether this process is a child that `run` or `run_holding` started.
        pub(crate) fn is_child() -> bool {
            std::env::var_os(CHILD_ENV).is_some()
        }

        /// In the parent, runs `test` alone in a child and fails with it; in the child,
        /// runs `body`.
        pub(crate) fn run(test: &str, body: fn()) {
            if is_child() {
                run_body(test, body);
                return;
            }
            if let Err(error) = spawn(test, None) {
                panic!("{error}");
            }
        }

        /// As `run`, and the body calls `hold` exactly once: the parent runs `while_held`
        /// while the child is stopped there, then lets it go on.
        pub(crate) fn run_holding(test: &str, body: fn(), while_held: impl FnOnce()) {
            if is_child() {
                run_body(test, body);
                return;
            }
            if let Err(error) = spawn(test, Some(Box::new(while_held))) {
                panic!("{error}");
            }
        }

        /// In a child that `run_holding` started, stops here until the parent has run its
        /// check.
        pub(crate) fn hold() {
            assert!(
                std::env::var_os(HOLD_ENV).is_some(),
                "hold() runs only in a child that run_holding started"
            );
            let mut out = std::io::stdout().lock();
            writeln!(out).unwrap();
            writeln!(out, "{HOLDING}").unwrap();
            out.flush().unwrap();
            drop(out);
            let mut line = String::new();
            std::io::stdin().read_line(&mut line).unwrap();
            assert_eq!(
                line.trim_end(),
                RELEASE,
                "the parent did not release the hold"
            );
        }

        fn run_body(test: &str, body: fn()) {
            body();
            let ran = std::env::var_os(CHILD_ENV).unwrap();
            std::fs::write(ran, test).unwrap();
        }

        /// Kills a child the parent stops waiting for, so a failed check does not hang.
        struct Running(Child);

        impl Drop for Running {
            fn drop(&mut self) {
                if let Ok(None) = self.0.try_wait() {
                    let _ = self.0.kill();
                    let _ = self.0.wait();
                }
            }
        }

        pub(crate) fn spawn(
            test: &str,
            while_held: Option<Box<dyn FnOnce() + '_>>,
        ) -> Result<(), String> {
            let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
            let ran = scratch.path().join("ran");
            let home = scratch.path().join("home");
            std::fs::create_dir(&home).map_err(|error| error.to_string())?;
            let executable = std::env::current_exe().map_err(|error| error.to_string())?;
            let mut command = Command::new(executable);
            command
                .args(["--exact", test, "--nocapture"])
                .env(CHILD_ENV, &ran)
                .env("HOME", &home)
                .env_remove("COMMANDAGENT_ADAPTER_CONFIG_ROOT")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped());
            for key in super::PACK_ENV_KEYS {
                command.env_remove(key);
            }
            let expects_hold = while_held.is_some();
            if expects_hold {
                command.env(HOLD_ENV, "1");
            }
            let mut child = Running(
                command
                    .spawn()
                    .map_err(|error| format!("{test}: cannot start its process: {error}"))?,
            );
            let stdout = child.0.stdout.take().unwrap();
            let mut stdin = child.0.stdin.take();
            let mut while_held = while_held;
            let mut holds = 0;
            for line in BufReader::new(stdout).lines() {
                let line = line.map_err(|error| error.to_string())?;
                if line != HOLDING {
                    println!("{line}");
                    continue;
                }
                holds += 1;
                let Some(check) = while_held.take() else {
                    return Err(format!(
                        "{test}: the child held {holds} time(s); the parent checks once"
                    ));
                };
                // A failing check unwinds from here, and `Running` stops the child.
                check();
                let input = stdin.as_mut().unwrap();
                writeln!(input, "{RELEASE}").map_err(|error| error.to_string())?;
                input.flush().map_err(|error| error.to_string())?;
            }
            drop(stdin);
            let status = child.0.wait().map_err(|error| error.to_string())?;
            if !status.success() {
                return Err(format!("{test} failed in its own process: {status}"));
            }
            if expects_hold && holds != 1 {
                return Err(format!(
                    "{test}: the parent expected one hold in its process, observed {holds}"
                ));
            }
            match std::fs::read_to_string(&ran) {
                Ok(name) if name == test => Ok(()),
                Ok(name) => Err(format!("{test}: its process ran `{name}` instead")),
                Err(_) => Err(format!(
                    "{test}: its process exited without running the test body"
                )),
            }
        }
    }

    #[test]
    fn an_own_process_child_that_runs_no_test_fails_the_parent() {
        let error = own_process::spawn("cli_pack::tests::no_test_has_this_name", None).unwrap_err();
        assert!(error.contains("without running the test body"), "{error}");
    }

    #[test]
    fn an_own_process_child_that_fails_fails_the_parent() {
        const TEST: &str = "cli_pack::tests::an_own_process_child_that_fails_fails_the_parent";
        if own_process::is_child() {
            panic!("this child fails on purpose");
        }
        let error = own_process::spawn(TEST, None).unwrap_err();
        assert!(error.contains("failed in its own process"), "{error}");
    }

    #[test]
    fn an_own_process_hold_runs_the_parent_check_while_the_child_waits() {
        const TEST: &str =
            "cli_pack::tests::an_own_process_hold_runs_the_parent_check_while_the_child_waits";
        let checked = std::cell::Cell::new(false);
        own_process::run_holding(
            TEST,
            || {
                own_process::hold();
            },
            || checked.set(true),
        );
        assert!(own_process::is_child() || checked.get());
        // A parent check that fails fails the parent, and the child is stopped.
        if !own_process::is_child() {
            let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                own_process::spawn(TEST, Some(Box::new(|| panic!("the parent check fails"))))
            }));
            assert!(failed.is_err());
        }
    }

    fn copy_directory(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_directory(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    /// `data-assist@1.0.0` of this repository, as a selection in a private workspace.
    fn resolved_data_pack(root: &Path) -> (Config, ResolvedPack, PathBuf) {
        let directory = root.join("packs/data-assist/1.0.0");
        copy_directory(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("packs/data-assist/1.0.0"),
            &directory,
        );
        resolved(root, "data-assist@1.0.0", directory)
    }

    fn resolved(
        root: &Path,
        selector: &str,
        directory: PathBuf,
    ) -> (Config, ResolvedPack, PathBuf) {
        use clap::Parser;
        let cli = Cli::parse_from([
            "commandagent",
            "--profile",
            "data",
            "--intent",
            "create",
            "--pack",
            selector,
            "--prompt",
            "hi",
        ]);
        let mut config = Config::from_cli(cli.clone()).unwrap();
        config.workspace_root = root.to_path_buf();
        let pack = resolve(&cli, &config).unwrap().unwrap();
        (config, pack, directory)
    }

    fn admission_error(root: &Path, profile: &str) -> String {
        crate::tools::profile_tools::admit_selected(root, profile, None)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn an_installed_selection_is_admitted_without_reading_the_pack() {
        in_own_process(
            "cli_pack::tests::an_installed_selection_is_admitted_without_reading_the_pack",
            || {
                let temp = tempfile::tempdir().unwrap();
                let root = std::fs::canonicalize(temp.path()).unwrap();
                let (config, pack, directory) = resolved_data_pack(&root);
                assert_eq!(pack.declared.as_ref().unwrap().tools, None);
                let moved = root.join("moved");

                // The installed value answers with the pack gone.
                let guard = RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                std::fs::rename(&directory, &moved).unwrap();
                assert!(
                    crate::tools::profile_tools::admit_selected(&root, "data", None)
                        .map_err(|error| error.to_string())
                        .unwrap()
                        .is_none()
                );
                crate::tools::profile_tools::pre_run::refuse_before_workflow_node(&config).unwrap();
                // Another profile does not take it: admission reads, and the pack is gone.
                let error = admission_error(&root, "nextjs");
                assert!(error.contains("selected_pack_invalid"), "{error}");
                drop(guard);
                assert_eq!(std::env::var_os(PACK_DIRECTORY_ENV), None);

                // The same selection with nothing decoded, as a TUI confirmation installs
                // it, reads: this is what the value above saved.
                let unknown = ResolvedPack {
                    declared: None,
                    ..pack
                };
                let guard = RuntimeEnvironmentGuard::install(Some(&unknown)).unwrap();
                let error = admission_error(&root, "data");
                assert!(error.contains("selected_pack_invalid"), "{error}");
                std::fs::rename(&moved, &directory).unwrap();
                assert!(
                    crate::tools::profile_tools::admit_selected(&root, "data", None)
                        .map_err(|error| error.to_string())
                        .unwrap()
                        .is_none()
                );
                drop(guard);
            },
        );
    }

    #[test]
    fn a_resolved_declaration_is_enforced_from_its_value_with_the_pack_gone() {
        in_own_process(
            "cli_pack::tests::a_resolved_declaration_is_enforced_from_its_value_with_the_pack_gone",
            || {
                let temp = tempfile::tempdir().unwrap();
                let root = std::fs::canonicalize(temp.path()).unwrap();
                let directory = root.join("packs/data-tools/1.0.0");
                std::fs::create_dir_all(&directory).unwrap();
                let eval = "schema_version: commandagent.pack.eval/v0\npack:\n  id: data-tools\n  version: 1.0.0\n  profile: data\n  intent: create\nprofile_tools:\n  schema_version: profile-tool.declaration/v1\n  builtin_tools: [Read]\n  tools:\n    - id: some-tool\n      effect: read\n";
                std::fs::write(directory.join("eval.yaml"), eval).unwrap();
                let hash = crate::planner::pack::exact_byte_hash(None, Some(eval.as_bytes()));
                std::fs::write(directory.join("pack.sha256"), format!("{hash}\n")).unwrap();
                let (config, pack, directory) = resolved(&root, "data-tools@1.0.0", directory);
                let moved = root.join("moved");
                std::fs::rename(&directory, &moved).unwrap();

                // The operator root is unset: the declaration is what is refused, not
                // a read of the pack, at the pre-run gate and after the guard installs.
                let selected = Ok(Some(pack.clone()));
                let error =
                    crate::tools::profile_tools::pre_run::refuse_before_cli_run(&config, &selected)
                        .unwrap_err()
                        .to_string();
                assert!(error.contains("registry_unavailable"), "{error}");
                let _guard = RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                let error = admission_error(&root, "data");
                assert!(error.contains("registry_unavailable"), "{error}");
            },
        );
    }

    #[test]
    fn the_guard_restores_the_previous_value_and_only_the_exact_selection_takes_one() {
        in_own_process(
            "cli_pack::tests::the_guard_restores_the_previous_value_and_only_the_exact_selection_takes_one",
            || {
                let temp = tempfile::tempdir().unwrap();
                let root = std::fs::canonicalize(temp.path()).unwrap();
                let (_config, pack, directory) = resolved_data_pack(&root);
                std::fs::rename(&directory, root.join("moved")).unwrap();
                let guard = RuntimeEnvironmentGuard::install(Some(&pack)).unwrap();
                // A different hash in the environment is another selection: it is read.
                // SAFETY: this test runs alone in its own process.
                unsafe { std::env::set_var(PACK_HASH_ENV, "0".repeat(64)) };
                let error = admission_error(&root, "data");
                assert!(error.contains("selected_pack_invalid"), "{error}");
                drop(guard);

                // After the guard the slot is empty: the same variables, set directly, read.
                // SAFETY: this test runs alone in its own process.
                unsafe {
                    std::env::set_var(PACK_DIRECTORY_ENV, &pack.directory);
                    std::env::set_var(PACK_ID_ENV, &pack.id);
                    std::env::set_var(PACK_VERSION_ENV, &pack.version);
                    std::env::set_var(PACK_HASH_ENV, &pack.hash);
                }
                let error = admission_error(&root, "data");
                assert!(error.contains("selected_pack_invalid"), "{error}");
            },
        );
    }
}
