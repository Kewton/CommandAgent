//! Opt-in source/config snapshot at the automatic Recovery treatment boundary.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use sha2::{Digest, Sha256};

const EXCLUDED_DIRECTORIES: &[&str] = &[
    ".anvil",
    ".cache",
    ".commandagent",
    ".commandagent-eval-home",
    ".commandagent-eval-tmp",
    ".commandagent-state",
    ".git",
    ".goal-verify-baseline",
    ".mypy_cache",
    ".next",
    ".nox",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".turbo",
    ".venv",
    "__pycache__",
    "build",
    "coverage",
    "dist",
    "htmlcov",
    "node_modules",
    "out",
    "target",
    "venv",
];
const SENSITIVE_NAMES: &[&str] = &[
    ".env",
    ".npmrc",
    ".pypirc",
    "credentials",
    "credentials.json",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryBoundarySnapshot {
    pub(crate) workspace_relative_path: String,
    pub(crate) file_count: usize,
    pub(crate) total_bytes: u64,
    pub(crate) snapshot_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryRestoreReport {
    pub(crate) restored_file_count: usize,
    pub(crate) removed_file_count: usize,
    pub(crate) snapshot_sha256: String,
}

pub(crate) fn capture_for_transaction(
    root: &Path,
    recovery_attempt: u8,
) -> anyhow::Result<RecoveryBoundarySnapshot> {
    capture(root, recovery_attempt)
}

pub(crate) fn current_source_sha256(root: &Path) -> anyhow::Result<String> {
    let root = root
        .canonicalize()
        .context("Recovery source workspace root is unavailable")?;
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files)?;
    content_sha256(&root, &files)
}

pub(crate) fn restore_transaction(
    root: &Path,
    snapshot: &RecoveryBoundarySnapshot,
) -> anyhow::Result<RecoveryRestoreReport> {
    let root = root
        .canonicalize()
        .context("Recovery transaction workspace root is unavailable")?;
    let snapshot_root = root.join(&snapshot.workspace_relative_path);
    let snapshot_root = snapshot_root
        .canonicalize()
        .context("Recovery transaction snapshot is unavailable")?;
    let transaction_root = root.join(".commandagent/recovery-boundaries");
    if !snapshot_root.starts_with(&transaction_root) {
        bail!("Recovery transaction snapshot escaped the evidence namespace");
    }
    sync_source_tree(&root, &snapshot_root)
}

pub(crate) fn prepare_treatment(
    root: &Path,
    snapshot: &RecoveryBoundarySnapshot,
    recovery_attempt: u8,
) -> anyhow::Result<PathBuf> {
    let root = root
        .canonicalize()
        .context("Recovery treatment workspace root is unavailable")?;
    let snapshot_root = root.join(&snapshot.workspace_relative_path);
    let relative = PathBuf::from(format!(
        ".commandagent/recovery-treatments/attempt-{recovery_attempt}/workspace"
    ));
    let treatment = root.join(&relative);
    if treatment.exists() {
        bail!("Recovery treatment workspace already exists");
    }
    copy_source_tree(&snapshot_root, &treatment)?;
    for runtime in [
        "node_modules",
        ".goal-verify-tools",
        ".venv",
        "venv",
        "target",
    ] {
        let source = root.join(runtime);
        if !source.exists() {
            continue;
        }
        let destination = treatment.join(runtime);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        symlink_directory(&source, &destination)?;
    }
    Ok(treatment)
}

pub(crate) fn promote_treatment(
    root: &Path,
    treatment: &Path,
) -> anyhow::Result<RecoveryRestoreReport> {
    let root = root
        .canonicalize()
        .context("Recovery promotion workspace root is unavailable")?;
    let treatment = treatment
        .canonicalize()
        .context("Recovery treatment workspace is unavailable")?;
    let treatment_namespace = root.join(".commandagent/recovery-treatments");
    if !treatment.starts_with(&treatment_namespace) {
        bail!("Recovery treatment workspace escaped the evidence namespace");
    }
    sync_source_tree(&root, &treatment)
}

pub(crate) fn retain_control(
    root: &Path,
    snapshot: &RecoveryBoundarySnapshot,
) -> anyhow::Result<RecoveryRestoreReport> {
    if current_source_sha256(root)? == snapshot.snapshot_sha256 {
        return Ok(RecoveryRestoreReport {
            restored_file_count: 0,
            removed_file_count: 0,
            snapshot_sha256: snapshot.snapshot_sha256.clone(),
        });
    }
    restore_transaction(root, snapshot)
}

fn sync_source_tree(
    destination_root: &Path,
    source_root: &Path,
) -> anyhow::Result<RecoveryRestoreReport> {
    let source_files = files_by_relative_path(source_root)?;
    let current_files = files_by_relative_path(destination_root)?;
    let mut removed_file_count = 0usize;
    for (relative, current) in &current_files {
        if source_files.contains_key(relative) {
            continue;
        }
        std::fs::remove_file(current)
            .with_context(|| format!("remove rejected Recovery artifact {}", relative.display()))?;
        removed_file_count += 1;
    }
    for (relative, source) in &source_files {
        let target = destination_root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(source, &target)
            .with_context(|| format!("restore Recovery control artifact {}", relative.display()))?;
        if let Ok(metadata) = std::fs::metadata(source) {
            let _ = std::fs::set_permissions(&target, metadata.permissions());
        }
    }
    let restored_files = files_by_relative_path(destination_root)?;
    let restored_paths = restored_files.values().cloned().collect::<Vec<_>>();
    let restored_sha256 = content_sha256(destination_root, &restored_paths)?;
    let source_paths = source_files.values().cloned().collect::<Vec<_>>();
    let source_sha256 = content_sha256(source_root, &source_paths)?;
    if restored_sha256 != source_sha256 {
        bail!("Recovery source synchronization hash mismatch");
    }
    Ok(RecoveryRestoreReport {
        restored_file_count: source_files.len(),
        removed_file_count,
        snapshot_sha256: restored_sha256,
    })
}

fn copy_source_tree(source_root: &Path, destination_root: &Path) -> anyhow::Result<()> {
    for (relative, source) in files_by_relative_path(source_root)? {
        let destination = destination_root.join(&relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&source, &destination)
            .with_context(|| format!("copy Recovery treatment artifact {}", relative.display()))?;
        if let Ok(metadata) = std::fs::metadata(source) {
            let _ = std::fs::set_permissions(&destination, metadata.permissions());
        }
    }
    Ok(())
}

#[cfg(unix)]
fn symlink_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, destination)
}

#[cfg(windows)]
fn symlink_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source, destination)
}

fn files_by_relative_path(root: &Path) -> anyhow::Result<BTreeMap<PathBuf, PathBuf>> {
    let mut paths = Vec::new();
    collect_files(root, root, &mut paths)?;
    paths
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(root)
                .context("Recovery transaction file escaped workspace")?
                .to_path_buf();
            Ok((relative, path))
        })
        .collect()
}

fn capture(root: &Path, recovery_attempt: u8) -> anyhow::Result<RecoveryBoundarySnapshot> {
    let root = root
        .canonicalize()
        .context("Recovery boundary workspace root is unavailable")?;
    let relative = PathBuf::from(format!(
        ".commandagent/recovery-boundaries/attempt-{recovery_attempt}/workspace"
    ));
    let destination = root.join(&relative);
    if destination.exists() {
        bail!("Recovery boundary snapshot destination already exists");
    }
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files)?;
    let mut total_bytes = 0u64;
    for source in &files {
        let rel = source
            .strip_prefix(&root)
            .context("Recovery boundary source escaped workspace")?;
        let target = destination.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = std::fs::copy(source, &target)
            .with_context(|| format!("copy Recovery boundary artifact {}", rel.display()))?;
        if let Ok(metadata) = std::fs::metadata(source) {
            let _ = std::fs::set_permissions(&target, metadata.permissions());
        }
        total_bytes = total_bytes.saturating_add(bytes);
    }
    Ok(RecoveryBoundarySnapshot {
        workspace_relative_path: relative.to_string_lossy().replace('\\', "/"),
        file_count: files.len(),
        total_bytes,
        snapshot_sha256: content_sha256(&root, &files)?,
    })
}

fn content_sha256(root: &Path, files: &[PathBuf]) -> anyhow::Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    for path in files {
        let relative = path
            .strip_prefix(root)
            .context("Recovery boundary hash source escaped workspace")?
            .to_string_lossy()
            .replace('\\', "/");
        let metadata = path.metadata()?;
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update(metadata.len().to_be_bytes());
        let mut file = std::fs::File::open(path)?;
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let mut entries = std::fs::read_dir(directory)
        .with_context(|| format!("read Recovery boundary directory {}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .context("Recovery boundary entry escaped workspace")?;
        if excluded(rel) {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn excluded(relative: &Path) -> bool {
    let mut components = relative
        .components()
        .filter_map(|part| part.as_os_str().to_str());
    if components
        .clone()
        .any(|part| EXCLUDED_DIRECTORIES.contains(&part))
    {
        return true;
    }
    let Some(name) = components.next_back() else {
        return true;
    };
    SENSITIVE_NAMES.contains(&name)
        || name.starts_with(".env.")
        || matches!(
            Path::new(name)
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("pyc" | "pyo" | "gcda" | "gcno" | "profraw")
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_source_and_excludes_runtime_cache_and_secrets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::create_dir_all(dir.path().join(".next/cache")).unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
        std::fs::write(dir.path().join("src/app.py"), "print('ok')\n").unwrap();
        std::fs::write(dir.path().join(".next/cache/large.bin"), vec![0; 4096]).unwrap();
        std::fs::write(dir.path().join("node_modules/pkg/index.js"), "cache\n").unwrap();
        std::fs::write(dir.path().join(".env"), "SECRET=value\n").unwrap();

        let captured = capture(dir.path(), 1).unwrap();
        let snapshot = dir.path().join(&captured.workspace_relative_path);

        assert_eq!(captured.file_count, 1);
        assert_eq!(captured.snapshot_sha256.len(), 64);
        assert!(snapshot.join("src/app.py").is_file());
        assert!(!snapshot.join(".next").exists());
        assert!(!snapshot.join("node_modules").exists());
        assert!(!snapshot.join(".env").exists());
    }

    #[test]
    fn existing_attempt_snapshot_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.py"), "pass\n").unwrap();
        capture(dir.path(), 1).unwrap();
        let error = capture(dir.path(), 1).unwrap_err();
        assert!(error.to_string().contains("already exists"), "{error:#}");
    }

    #[test]
    fn rejected_treatment_restores_control_and_preserves_runtime_evidence() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.py"), "print('fixed')\n").unwrap();
        let captured = capture_for_transaction(dir.path(), 1).unwrap();
        std::fs::write(dir.path().join("app.py"), "raise KeyError('regression')\n").unwrap();
        std::fs::write(dir.path().join("invented.txt"), "recovery output\n").unwrap();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(dir.path().join(".anvil/events.jsonl"), "evidence\n").unwrap();

        let report = restore_transaction(dir.path(), &captured).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join("app.py")).unwrap(),
            "print('fixed')\n"
        );
        assert!(!dir.path().join("invented.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".anvil/events.jsonl")).unwrap(),
            "evidence\n"
        );
        assert_eq!(report.restored_file_count, 1);
        assert_eq!(report.removed_file_count, 1);
        assert_eq!(report.snapshot_sha256, captured.snapshot_sha256);
    }

    #[test]
    fn isolated_treatment_changes_only_after_promotion() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.py"), "print('control')\n").unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
        std::fs::write(dir.path().join("node_modules/pkg/index.js"), "runtime\n").unwrap();
        let captured = capture_for_transaction(dir.path(), 1).unwrap();
        let treatment = prepare_treatment(dir.path(), &captured, 1).unwrap();

        assert!(treatment.join("node_modules").is_symlink());
        std::fs::write(treatment.join("app.py"), "print('treatment')\n").unwrap();
        std::fs::write(treatment.join("added.py"), "pass\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("app.py")).unwrap(),
            "print('control')\n"
        );

        let report = promote_treatment(dir.path(), &treatment).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join("app.py")).unwrap(),
            "print('treatment')\n"
        );
        assert!(dir.path().join("added.py").is_file());
        assert_eq!(report.restored_file_count, 2);
        let boundary = dir.path().join(&captured.workspace_relative_path);
        assert_eq!(
            std::fs::read_to_string(boundary.join("app.py")).unwrap(),
            "print('control')\n"
        );
    }

    #[test]
    fn snapshot_content_hash_has_a_cross_harness_fixed_vector() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "x\n").unwrap();
        assert_eq!(
            content_sha256(dir.path(), &[path]).unwrap(),
            "fca205d27f585d85835e310c03faf89448c6707cc889e2fda18085ae527122bf"
        );
    }
}
