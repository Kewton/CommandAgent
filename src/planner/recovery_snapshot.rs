//! Opt-in source/config snapshot at the automatic Recovery treatment boundary.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use sha2::{Digest, Sha256};

pub(crate) const CAPTURE_ENV: &str = "COMMANDAGENT_CAPTURE_RECOVERY_BOUNDARY";

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

pub(crate) fn capture_if_enabled(
    root: &Path,
    recovery_attempt: u8,
) -> anyhow::Result<Option<RecoveryBoundarySnapshot>> {
    if crate::env_compat::var_os(CAPTURE_ENV).as_deref() != Some(std::ffi::OsStr::new("1")) {
        return Ok(None);
    }
    capture(root, recovery_attempt).map(Some)
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
