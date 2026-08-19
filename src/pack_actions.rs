use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::cli::{Cli, IntentArg};
use crate::planner::pack::{PackIntent, PackProfile};

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
    let profile = crate::planner::profile_descriptor::descriptor_for_name(requested_profile)
        .and_then(|descriptor| descriptor.pack_profile)
        .or_else(|| PackProfile::parse(requested_profile))
        .with_context(|| format!("profile `{requested_profile}` does not support packs"))?;
    let intent = match cli.intent.context("--packs requires --intent")? {
        IntentArg::Create => PackIntent::Create,
        IntentArg::Fix => PackIntent::Fix,
        IntentArg::Investigate => PackIntent::Investigate,
    };

    println!("PACK\tHASH\tSOURCE");
    for pack in crate::planner::pack::catalog::compatible(profile.as_str(), intent.as_str()) {
        println!("{}@{}\t{}\tadmitted", pack.id, pack.version, pack.hash);
    }
    if let Some(extension_root) = &cli.extension_root {
        for directory in local_pack_directories(extension_root)? {
            let report =
                crate::planner::pack::conform_directory(&directory).with_context(|| {
                    format!(
                        "local pack conformance failed for `{}`",
                        directory.display()
                    )
                })?;
            if report.profile == profile.as_str() && report.intent == intent.as_str() {
                println!(
                    "{}@{}\t{}\tlocal",
                    report.pack_id, report.pack_version, report.exact_byte_hash
                );
            }
        }
    }
    Ok(())
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
    let compatibility_root = extension_root.join("packs");
    if compatibility_root.is_dir() {
        collect_pack_directories(&compatibility_root, false, &mut directories)?;
    }
    Ok(directories.into_iter().collect())
}

fn collect_pack_directories(
    root: &Path,
    skip_packs_directory: bool,
    directories: &mut BTreeSet<PathBuf>,
) -> anyhow::Result<()> {
    for id_entry in sorted_directories(root)? {
        if skip_packs_directory && id_entry.file_name() == "packs" {
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
