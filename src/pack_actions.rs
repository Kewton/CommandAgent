use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::cli::{Cli, IntentArg};
use crate::planner::pack::{PACKS_DIRECTORY, PackIntent, PackProfile};
use crate::planner::profile_manifest::source::EXTENSION_PROFILES_DIRECTORY;

const PACK_PIN_FILE: &str = "pack.sha256";

pub(crate) fn run_if_requested(cli: &Cli) -> anyhow::Result<bool> {
    if cli.packs {
        list(cli)?;
        return Ok(true);
    }
    if let Some(directory) = &cli.pack_verify {
        print_report(directory)?;
        return Ok(true);
    }
    if let Some(directory) = &cli.pack_pin {
        pin(directory)?;
        return Ok(true);
    }
    Ok(false)
}

fn list(cli: &Cli) -> anyhow::Result<()> {
    let requested_profile = cli
        .profile
        .as_deref()
        .context("--packs requires --profile")?;
    let intent = match cli.intent.context("--packs requires --intent")? {
        IntentArg::Create => PackIntent::Create,
        IntentArg::Fix => PackIntent::Fix,
        IntentArg::Investigate => PackIntent::Investigate,
    };
    print!(
        "{}",
        render_list(
            requested_profile,
            intent.as_str(),
            cli.extension_root.as_deref()
        )?
    );
    Ok(())
}

pub(crate) fn render_list(
    requested_profile: &str,
    requested_intent: &str,
    extension_root: Option<&Path>,
) -> anyhow::Result<String> {
    let profile = crate::planner::profile_descriptor::descriptor_for_name(requested_profile)
        .and_then(|descriptor| descriptor.pack_profile)
        .or_else(|| PackProfile::parse(requested_profile))
        .with_context(|| format!("profile `{requested_profile}` does not support packs"))?;
    let intent = PackIntent::parse(requested_intent)
        .with_context(|| format!("intent `{requested_intent}` does not support packs"))?;
    let mut lines = vec!["PACK\tHASH\tSOURCE".to_string()];
    for pack in crate::planner::pack::catalog::compatible(profile.as_str(), intent.as_str()) {
        lines.push(format!(
            "{}@{}\t{}\tadmitted",
            pack.id, pack.version, pack.hash
        ));
    }
    if let Some(extension_root) = extension_root {
        for directory in local_pack_directories(extension_root)? {
            let report = match crate::planner::pack::conform_directory(&directory) {
                Ok(report) => report,
                Err(error) => {
                    eprintln!(
                        "warning: skipping invalid local pack `{}`: {error:#}",
                        directory.display()
                    );
                    continue;
                }
            };
            if report.profile == profile.as_str() && report.intent == intent.as_str() {
                // A retired pack stays listed for audit but is never selectable,
                // so the source cell says so instead of reading as available.
                let source = if crate::planner::pack::catalog::is_retired(&directory) {
                    "local (retired)"
                } else {
                    "local"
                };
                lines.push(format!(
                    "{}@{}\t{}\t{source}",
                    report.pack_id, report.pack_version, report.exact_byte_hash
                ));
            }
        }
    }
    Ok(format!("{}\n", lines.join("\n")))
}

fn local_pack_directories(extension_root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    if !extension_root.is_dir() {
        bail!(
            "extension root `{}` is not a directory",
            extension_root.display()
        );
    }
    let mut directories = BTreeSet::new();
    collect_pack_directories(extension_root, true, &mut directories)?;
    let compatibility_root = extension_root.join(PACKS_DIRECTORY);
    if compatibility_root.is_dir() {
        collect_pack_directories(&compatibility_root, false, &mut directories)?;
    }
    Ok(directories.into_iter().collect())
}

fn collect_pack_directories(
    root: &Path,
    skip_extension_namespaces: bool,
    directories: &mut BTreeSet<PathBuf>,
) -> anyhow::Result<()> {
    for id_entry in sorted_directories(root)? {
        if skip_extension_namespaces
            && (id_entry.file_name() == PACKS_DIRECTORY
                || id_entry.file_name() == EXTENSION_PROFILES_DIRECTORY)
        {
            continue;
        }
        for version_entry in sorted_directories(&id_entry.path())? {
            directories.insert(version_entry.path());
        }
    }
    Ok(())
}

fn sorted_directories(root: &Path) -> anyhow::Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(root)
        .with_context(|| format!("read extension pack directory `{}`", root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("read entries under `{}`", root.display()))?;
    entries.retain(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()));
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn print_report(directory: &Path) -> anyhow::Result<()> {
    let report = crate::planner::pack::conform_directory(directory)
        .with_context(|| format!("pack conformance failed for `{}`", directory.display()))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn pin(directory: &Path) -> anyhow::Result<()> {
    let report = crate::planner::pack::conform_directory(directory)
        .with_context(|| format!("pack conformance failed for `{}`", directory.display()))?;
    let pin_path = directory.join(PACK_PIN_FILE);
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pin_path)
    {
        Ok(mut file) => {
            writeln!(file, "{}", report.exact_byte_hash)
                .with_context(|| format!("write pack pin `{}`", pin_path.display()))?;
            println!("created {}", pin_path.display());
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            validate_existing_pin(&pin_path, &report.exact_byte_hash)?;
            println!("unchanged {}", pin_path.display());
            Ok(())
        }
        Err(error) => {
            Err(error).with_context(|| format!("create pack pin `{}`", pin_path.display()))
        }
    }
}

fn validate_existing_pin(pin_path: &Path, observed_hash: &str) -> anyhow::Result<()> {
    let pinned_hash = fs::read_to_string(pin_path)
        .with_context(|| format!("read pack pin `{}`", pin_path.display()))?;
    if pinned_hash.trim() != observed_hash {
        bail!(
            "pack hash mismatch: pack.sha256 contains `{}`, observed `{observed_hash}`",
            pinned_hash.trim()
        );
    }
    Ok(())
}
