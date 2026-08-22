use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

const LOCK_SCHEMA_VERSION: u8 = 1;
const MAX_OWNER_BYTES: u64 = 4_096;

#[derive(Debug, Serialize, Deserialize)]
struct LockOwner {
    schema_version: u8,
    pid: u32,
    run_id: String,
}

#[derive(Debug)]
pub(crate) struct WorkspaceLock {
    file: File,
}

impl Drop for WorkspaceLock {
    fn drop(&mut self) {
        // SAFETY: `file` remains open for this call and flock only uses its fd.
        let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

pub(crate) fn acquire(
    workspace_root: &Path,
    events_path: Option<&Path>,
) -> anyhow::Result<WorkspaceLock> {
    let runtime_dir = crate::runtime_paths::workspace_dir(workspace_root);
    std::fs::create_dir_all(&runtime_dir)
        .with_context(|| format!("create runtime directory {}", runtime_dir.display()))?;
    let path = runtime_dir.join("lock");
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW);
    let mut file = options
        .open(&path)
        .with_context(|| format!("open workspace lock {}", path.display()))?;

    // SAFETY: `file` is an open fd owned by this guard. LOCK_NB guarantees that
    // acquisition returns immediately instead of waiting for another process.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        if matches!(error.raw_os_error(), Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN)
        {
            let owner = read_owner(&mut file);
            anyhow::bail!(
                "workspace is already locked by {owner}; lock file: {}. Wait for that run to finish and retry",
                path.display()
            );
        }
        return Err(error).with_context(|| format!("lock workspace at {}", path.display()));
    }

    let owner = LockOwner {
        schema_version: LOCK_SCHEMA_VERSION,
        pid: std::process::id(),
        run_id: run_id(events_path),
    };
    let mut encoded = serde_json::to_vec(&owner).context("serialize workspace lock owner")?;
    encoded.push(b'\n');
    file.set_len(0)
        .with_context(|| format!("truncate workspace lock {}", path.display()))?;
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("seek workspace lock {}", path.display()))?;
    file.write_all(&encoded)
        .with_context(|| format!("write workspace lock {}", path.display()))?;
    file.flush()
        .with_context(|| format!("flush workspace lock {}", path.display()))?;

    Ok(WorkspaceLock { file })
}

fn read_owner(file: &mut File) -> String {
    if file.seek(SeekFrom::Start(0)).is_err() {
        return "another commandagent process (owner metadata unavailable)".to_string();
    }
    let mut bytes = Vec::new();
    if file
        .take(MAX_OWNER_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_OWNER_BYTES
    {
        return "another commandagent process (owner metadata unavailable)".to_string();
    }
    match serde_json::from_slice::<LockOwner>(&bytes) {
        Ok(owner) if owner.schema_version == LOCK_SCHEMA_VERSION => {
            format!("commandagent pid {} (run {})", owner.pid, owner.run_id)
        }
        _ => "another commandagent process (owner metadata unavailable)".to_string(),
    }
}

fn run_id(events_path: Option<&Path>) -> String {
    events_path
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("not-recorded")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_records_owner_rejects_contention_and_reacquires_after_drop() {
        let root = tempfile::tempdir().unwrap();
        let events = root
            .path()
            .join(".commandagent/runs/issue-226-run/events.jsonl");
        let first = acquire(root.path(), Some(&events)).unwrap();
        let path = root.path().join(".commandagent/lock");
        let record: LockOwner =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(record.schema_version, LOCK_SCHEMA_VERSION);
        assert_eq!(record.pid, std::process::id());
        assert_eq!(record.run_id, "issue-226-run");

        let error = acquire(root.path(), Some(&events)).unwrap_err().to_string();
        assert!(
            error.contains(&format!("pid {}", std::process::id())),
            "{error}"
        );
        assert!(error.contains("run issue-226-run"), "{error}");
        assert!(error.contains("Wait for that run to finish"), "{error}");

        drop(first);
        let second = acquire(root.path(), Some(&events)).unwrap();
        drop(second);
        assert!(
            path.is_file(),
            "the stable lock inode is kept for safe reuse"
        );
    }

    #[test]
    fn run_id_fallback_never_invents_an_events_identity() {
        assert_eq!(run_id(None), "not-recorded");
        assert_eq!(run_id(Some(Path::new("runs/abc/events.jsonl"))), "abc");
    }
}
