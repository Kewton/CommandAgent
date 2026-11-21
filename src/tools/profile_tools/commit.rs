//! The single host commit of one accepted candidate (Issue #974, G3).
//!
//! Everything happens on a parent directory descriptor reached by a no-follow walk from
//! the root descriptor held since admission: an exclusive temporary name in that directory,
//! write, fsync, `renameat`, then fsync of the directory. **The linearization point is the
//! successful `renameat`.** Before it, nothing a consumer can see has changed and a failure
//! or a cancel leaves the old bytes and removes the temporary. After it, the commit stands:
//! a later cancel or a failed fsync is recorded on the receipt, never turned into "nothing
//! was saved".
//!
//! This leaf decides the receipt. The caller records from it, not from a tool name.

use std::fs::File;
use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::protocol::{Binding, Diagnostic, RECEIPT_PROTOCOL};
use super::{ProfileToolRefusal, RefusalKind};

/// The smallest record that ties the raw result to what was adopted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitReceipt {
    pub protocol: String,
    pub request_id: String,
    pub tool_id: String,
    pub output_id: String,
    pub path: String,
    /// True on every receipt: there is no receipt for a commit that did not stand.
    pub committed: bool,
    /// False when the rename stood but the directory's fsync failed.
    pub durable: bool,
    pub cancelled_after_commit: bool,
    pub runtime_sha256: String,
    pub input_revision: String,
    pub schema_sha256: String,
    pub result_sha256: String,
    pub committed_sha256: String,
    pub committed_bytes: usize,
    pub diagnostics: Vec<Diagnostic>,
}

/// One candidate and what must hold right up to its adoption.
#[derive(Debug)]
pub(super) struct CommitPlan<'a> {
    pub(super) root: &'a File,
    pub(super) root_display: &'a Path,
    pub(super) relative: &'a str,
    pub(super) bytes: &'a [u8],
    pub(super) committed_sha256: &'a str,
    /// Workspace-relative paths the destination must not be the same file as.
    pub(super) forbidden: &'a [String],
    pub(super) binding: &'a Binding,
    pub(super) output_id: &'a str,
    pub(super) result_sha256: &'a str,
    pub(super) diagnostics: &'a [Diagnostic],
}

fn refused(reason: impl Into<String>) -> ProfileToolRefusal {
    ProfileToolRefusal::new(RefusalKind::CommitFailed, reason)
}

fn path_refused(reason: impl Into<String>) -> ProfileToolRefusal {
    ProfileToolRefusal::new(RefusalKind::EffectPathRefused, reason)
}

#[cfg(test)]
pub(super) mod test_hook {
    use std::cell::RefCell;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(in super::super) enum Stage {
        AfterTempWrite,
        BeforeRename,
        AfterRename,
    }

    type Hook = Box<dyn FnMut(Stage)>;

    thread_local! {
        static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
        static FAIL_DIRECTORY_SYNC: RefCell<bool> = const { RefCell::new(false) };
    }

    pub(in super::super) struct Reset;

    impl Drop for Reset {
        fn drop(&mut self) {
            HOOK.with(|slot| slot.borrow_mut().take());
            FAIL_DIRECTORY_SYNC.with(|slot| *slot.borrow_mut() = false);
        }
    }

    pub(in super::super) fn install(hook: impl FnMut(Stage) + 'static) -> Reset {
        HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
        Reset
    }

    pub(in super::super) fn fail_directory_sync() {
        FAIL_DIRECTORY_SYNC.with(|slot| *slot.borrow_mut() = true);
    }

    pub(super) fn fire(stage: Stage) {
        let taken = HOOK.with(|slot| slot.borrow_mut().take());
        if let Some(mut hook) = taken {
            hook(stage);
            HOOK.with(|slot| *slot.borrow_mut() = Some(hook));
        }
    }

    pub(super) fn directory_sync_fails() -> bool {
        FAIL_DIRECTORY_SYNC.with(|slot| *slot.borrow())
    }
}

/// `(device, inode)` of what is at a workspace-relative path now, reached with no symlink.
/// `None` only when nothing is there. A path that cannot be identified, such as a symlink, is
/// a refusal: left out of the comparison, it would protect nothing.
#[cfg(unix)]
fn identity_below(
    root: &File,
    display: &Path,
    relative: &str,
) -> Result<Option<(u64, u64)>, ProfileToolRefusal> {
    use std::os::unix::fs::MetadataExt as _;
    let unidentified = || path_refused("a protected path cannot be identified without a symlink");
    let parent =
        match crate::tools::dir_fd::open_parent_at(root, display, Path::new(relative), false) {
            Ok(parent) => parent,
            Err(error) if names_nothing(&error) => return Ok(None),
            Err(_) => return Err(unidentified()),
        };
    let file = match parent.open_existing_leaf() {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(unidentified()),
    };
    let metadata = file.metadata().map_err(|_| unidentified())?;
    Ok(Some((metadata.dev(), metadata.ino())))
}

#[cfg(unix)]
fn names_nothing(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
}

#[cfg(unix)]
fn identities_below(
    root: &File,
    display: &Path,
    paths: &[String],
) -> Result<Vec<(u64, u64)>, ProfileToolRefusal> {
    let mut identities = Vec::with_capacity(paths.len());
    for path in paths {
        if let Some(identity) = identity_below(root, display, path)? {
            identities.push(identity);
        }
    }
    Ok(identities)
}

/// What is at the destination may be replaced only when it is one regular file with one
/// name, and not another name of a protected input.
#[cfg(unix)]
fn check_existing(
    parent: &crate::tools::dir_fd::ParentDir,
    forbidden: &[(u64, u64)],
) -> Result<Option<u32>, ProfileToolRefusal> {
    use std::os::unix::fs::MetadataExt as _;
    let file = match parent.open_existing_leaf() {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(path_refused(
                "the destination is a symlink or cannot be opened as a file",
            ));
        }
    };
    let metadata = file
        .metadata()
        .map_err(|_| path_refused("the destination cannot be inspected"))?;
    if !metadata.is_file() {
        return Err(path_refused("the destination is not a regular file"));
    }
    if metadata.nlink() != 1 {
        return Err(path_refused("the destination has another hard link"));
    }
    if forbidden.contains(&(metadata.dev(), metadata.ino())) {
        return Err(path_refused(
            "the destination is the same file as a protected input",
        ));
    }
    // Only the permission bits move to the new file: never setuid, setgid, or sticky.
    Ok(Some(metadata.mode() & 0o777))
}

/// The entry under this name is the file this descriptor holds, with one name, and that file
/// holds exactly the checked bytes. A name alone says nothing: anything that can write in
/// the directory can replace what it points at, or write through it.
#[cfg(unix)]
fn entry_holds_candidate(
    parent: &crate::tools::dir_fd::ParentDir,
    name: &std::ffi::CStr,
    held: &File,
    expected_len: usize,
    expected_sha256: &str,
) -> bool {
    use std::os::unix::fs::{FileExt as _, MetadataExt as _};
    let (Ok(entry), Ok(metadata)) = (parent.stat_name(name), held.metadata()) else {
        return false;
    };
    if !entry.regular
        || entry.links != 1
        || (entry.device, entry.inode) != (metadata.dev(), metadata.ino())
        || metadata.len() != expected_len as u64
    {
        return false;
    }
    let mut bytes = vec![0u8; expected_len];
    held.read_exact_at(&mut bytes, 0).is_ok()
        && format!("{:x}", Sha256::digest(&bytes)) == expected_sha256
}

/// Before the start: what is at the destination now must be replaceable. A parent that does
/// not exist yet is fine, because the commit makes it below the held root. The commit
/// repeats this on its own descriptors; this only refuses early what is already wrong.
#[cfg(unix)]
pub(super) fn preflight_destination(
    root: &File,
    root_display: &Path,
    relative: &str,
    forbidden: &[String],
) -> Result<(), ProfileToolRefusal> {
    let identities = identities_below(root, root_display, forbidden)?;
    let parent =
        crate::tools::dir_fd::open_parent_at(root, root_display, Path::new(relative), false);
    match parent {
        Ok(parent) => check_existing(&parent, &identities).map(|_| ()),
        Err(error) if names_nothing(&error) => Ok(()),
        Err(_) => Err(path_refused(
            "the destination's parent is not reached without a symlink",
        )),
    }
}

#[cfg(not(unix))]
pub(super) fn preflight_destination(
    _root: &File,
    _root_display: &Path,
    _relative: &str,
    _forbidden: &[String],
) -> Result<(), ProfileToolRefusal> {
    Err(ProfileToolRefusal::new(
        RefusalKind::EffectUnavailable,
        "no host commit on this platform",
    ))
}

/// Removes the temporary on every way out before the rename.
#[cfg(unix)]
struct TemporaryName<'a> {
    parent: &'a crate::tools::dir_fd::ParentDir,
    name: std::ffi::CString,
    adopted: bool,
}

#[cfg(unix)]
impl Drop for TemporaryName<'_> {
    fn drop(&mut self) {
        if !self.adopted {
            let _ = self.parent.unlink(&self.name);
        }
    }
}

/// Adopt the candidate. `final_check` is the caller's judgement with the values in force
/// now, and runs last before the rename, as does `is_cancelled`.
#[cfg(unix)]
pub(super) fn commit(
    plan: &CommitPlan<'_>,
    final_check: &dyn Fn() -> Result<(), ProfileToolRefusal>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CommitReceipt, ProfileToolRefusal> {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;

    if format!("{:x}", Sha256::digest(plan.bytes)) != plan.committed_sha256 {
        return Err(refused("the candidate bytes differ from their digest"));
    }
    let cancelled = || refused("cancelled before the commit");
    if is_cancelled() {
        return Err(cancelled());
    }
    final_check()?;
    let forbidden = identities_below(plan.root, plan.root_display, plan.forbidden)?;
    let relative = Path::new(plan.relative);
    if let Ok(parent) =
        crate::tools::dir_fd::open_parent_at(plan.root, plan.root_display, relative, false)
    {
        check_existing(&parent, &forbidden)?;
    }
    let parent = crate::tools::dir_fd::open_parent_at(plan.root, plan.root_display, relative, true)
        .map_err(|_| path_refused("the destination's parent is not reached without a symlink"))?;
    check_existing(&parent, &forbidden)?;
    let temporary_name = std::ffi::CString::new(format!(
        ".profile-tool-{}.tmp",
        uuid::Uuid::now_v7().simple()
    ))
    .map_err(|_| refused("the temporary name is not valid"))?;
    let mut file = parent
        .create_exclusive(&temporary_name)
        .map_err(|_| refused("the temporary file cannot be created"))?;
    let mut temporary = TemporaryName {
        parent: &parent,
        name: temporary_name,
        adopted: false,
    };
    file.write_all(plan.bytes)
        .map_err(|_| refused("the temporary file cannot be written"))?;
    #[cfg(test)]
    test_hook::fire(test_hook::Stage::AfterTempWrite);
    // The existing permission bits stay. A new file stays private to this user.
    if let Some(mode) = check_existing(&parent, &forbidden)? {
        file.set_permissions(std::fs::Permissions::from_mode(mode))
            .map_err(|_| refused("the temporary file's mode cannot be set"))?;
    }
    file.sync_all()
        .map_err(|_| refused("the temporary file cannot be synced"))?;
    final_check()?;
    #[cfg(test)]
    test_hook::fire(test_hook::Stage::BeforeRename);
    if is_cancelled() {
        return Err(cancelled());
    }
    if !entry_holds_candidate(
        &parent,
        &temporary.name,
        &file,
        plan.bytes.len(),
        plan.committed_sha256,
    ) {
        return Err(refused(
            "the temporary file was changed or replaced before the rename",
        ));
    }
    parent
        .rename_to_leaf(&temporary.name)
        .map_err(|_| refused("the rename onto the destination failed"))?;
    // The linearization point has passed. Nothing below can undo it or report it undone.
    temporary.adopted = true;
    #[cfg(test)]
    test_hook::fire(test_hook::Stage::AfterRename);
    // What the name now holds decides what is reported. A writer in that instant has replaced
    // the old bytes with something this host never checked, and no receipt says otherwise.
    if !entry_holds_candidate(
        &parent,
        parent.leaf_name(),
        &file,
        plan.bytes.len(),
        plan.committed_sha256,
    ) {
        return Err(refused(
            "the rename stood, but the destination no longer holds the checked bytes",
        ));
    }
    #[cfg(test)]
    let synced = !test_hook::directory_sync_fails() && parent.sync_directory().is_ok();
    #[cfg(not(test))]
    let synced = parent.sync_directory().is_ok();
    Ok(CommitReceipt {
        protocol: RECEIPT_PROTOCOL.to_string(),
        request_id: plan.binding.request_id.clone(),
        tool_id: plan.binding.tool_id.clone(),
        output_id: plan.output_id.to_string(),
        path: plan.relative.to_string(),
        committed: true,
        durable: synced,
        cancelled_after_commit: is_cancelled(),
        runtime_sha256: plan.binding.runtime_sha256.clone(),
        input_revision: plan.binding.input_revision.clone(),
        schema_sha256: plan.binding.schema_sha256.clone(),
        result_sha256: plan.result_sha256.to_string(),
        committed_sha256: plan.committed_sha256.to_string(),
        committed_bytes: plan.bytes.len(),
        diagnostics: plan.diagnostics.to_vec(),
    })
}

#[cfg(not(unix))]
pub(super) fn commit(
    _plan: &CommitPlan<'_>,
    _final_check: &dyn Fn() -> Result<(), ProfileToolRefusal>,
    _is_cancelled: &dyn Fn() -> bool,
) -> Result<CommitReceipt, ProfileToolRefusal> {
    Err(ProfileToolRefusal::new(
        RefusalKind::EffectUnavailable,
        "no host commit on this platform",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use std::cell::Cell;
    use std::fs::File;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use sha2::{Digest, Sha256};

    use super::test_hook::{self, Stage};
    use super::{CommitPlan, CommitReceipt, commit};
    use crate::tools::profile_tools::protocol::Binding;
    use crate::tools::profile_tools::{ProfileToolRefusal, RefusalKind};

    const OLD: &[u8] = b"old bytes\n";
    const NEW: &[u8] = b"new bytes\n";

    struct Stage2 {
        _temp: tempfile::TempDir,
        root: PathBuf,
        outside: PathBuf,
        file: File,
    }

    fn stage() -> Stage2 {
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let root = base.join("workspace");
        let outside = base.join("outside");
        std::fs::create_dir_all(root.join("out")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let file = File::open(&root).unwrap();
        Stage2 {
            _temp: temp,
            root,
            outside,
            file,
        }
    }

    fn binding() -> Binding {
        Binding {
            request_id: "request-1".to_string(),
            tool_id: "tool-1".to_string(),
            tool_version: "1.0.0".to_string(),
            descriptor_sha256: "d".repeat(64),
            runtime_sha256: "a".repeat(64),
            input_revision: "b".repeat(64),
            schema_sha256: "c".repeat(64),
        }
    }

    fn attempt_with(
        stage: &Stage2,
        relative: &str,
        forbidden: &[String],
        final_check: &dyn Fn() -> Result<(), ProfileToolRefusal>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<CommitReceipt, ProfileToolRefusal> {
        let digest = format!("{:x}", Sha256::digest(NEW));
        let binding = binding();
        commit(
            &CommitPlan {
                root: &stage.file,
                root_display: &stage.root,
                relative,
                bytes: NEW,
                committed_sha256: &digest,
                forbidden,
                binding: &binding,
                output_id: "artifact",
                result_sha256: &"e".repeat(64),
                diagnostics: &[],
            },
            final_check,
            is_cancelled,
        )
    }

    fn attempt(stage: &Stage2, relative: &str) -> Result<CommitReceipt, ProfileToolRefusal> {
        attempt_with(stage, relative, &[], &|| Ok(()), &|| false)
    }

    fn leftovers(directory: &Path) -> Vec<String> {
        std::fs::read_dir(directory)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".profile-tool-"))
            .collect()
    }

    #[test]
    fn a_new_file_is_written_whole_with_a_receipt_of_its_bytes() {
        let stage = stage();
        let receipt = attempt(&stage, "out/settings.json").unwrap();
        assert_eq!(
            std::fs::read(stage.root.join("out/settings.json")).unwrap(),
            NEW
        );
        assert!(receipt.committed && receipt.durable && !receipt.cancelled_after_commit);
        assert_eq!(
            receipt.committed_sha256,
            format!("{:x}", Sha256::digest(NEW))
        );
        assert_eq!(receipt.committed_bytes, NEW.len());
        assert_eq!(receipt.result_sha256, "e".repeat(64));
        assert!(leftovers(&stage.root.join("out")).is_empty());
    }

    #[test]
    fn missing_parents_are_created_below_the_held_root() {
        let stage = stage();
        attempt(&stage, "a/b/c/settings.json").unwrap();
        assert_eq!(
            std::fs::read(stage.root.join("a/b/c/settings.json")).unwrap(),
            NEW
        );
    }

    #[test]
    fn an_existing_file_is_replaced_by_a_new_inode_and_keeps_its_mode() {
        let stage = stage();
        let target = stage.root.join("out/settings.json");
        std::fs::write(&target, OLD).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
        let before = std::fs::metadata(&target).unwrap();
        let open_old = File::open(&target).unwrap();
        attempt(&stage, "out/settings.json").unwrap();
        let after = std::fs::metadata(&target).unwrap();
        assert_ne!(before.ino(), after.ino());
        assert_eq!(after.mode() & 0o7777, 0o640);
        assert_eq!(std::fs::read(&target).unwrap(), NEW);
        assert_eq!(std::io::read_to_string(open_old).unwrap().as_bytes(), OLD);
    }

    fn assert_refused_unchanged(
        result: &Result<CommitReceipt, ProfileToolRefusal>,
        stage: &Stage2,
    ) {
        let refusal = result.as_ref().unwrap_err();
        assert!(
            matches!(
                refusal.kind,
                RefusalKind::EffectPathRefused | RefusalKind::CommitFailed
            ),
            "{refusal}"
        );
        assert!(leftovers(&stage.root.join("out")).is_empty());
    }

    #[test]
    fn a_symlink_leaf_is_refused_and_its_target_is_untouched() {
        let stage = stage();
        let target = stage.outside.join("secret");
        std::fs::write(&target, OLD).unwrap();
        symlink(&target, stage.root.join("out/settings.json")).unwrap();
        let result = attempt(&stage, "out/settings.json");
        assert_refused_unchanged(&result, &stage);
        assert_eq!(std::fs::read(&target).unwrap(), OLD);
    }

    #[test]
    fn a_symlink_parent_is_refused_even_pointing_inside() {
        let stage = stage();
        symlink(stage.root.join("out"), stage.root.join("alias")).unwrap();
        symlink(&stage.outside, stage.root.join("away")).unwrap();
        for path in ["alias/settings.json", "away/settings.json"] {
            assert_refused_unchanged(&attempt(&stage, path), &stage);
        }
        assert!(!stage.outside.join("settings.json").exists());
    }

    #[test]
    fn a_hard_link_is_refused_and_its_other_name_is_untouched() {
        let stage = stage();
        let other = stage.outside.join("other-name");
        std::fs::write(&other, OLD).unwrap();
        std::fs::hard_link(&other, stage.root.join("out/settings.json")).unwrap();
        assert_refused_unchanged(&attempt(&stage, "out/settings.json"), &stage);
        assert_eq!(std::fs::read(&other).unwrap(), OLD);
        assert_eq!(
            std::fs::read(stage.root.join("out/settings.json")).unwrap(),
            OLD
        );
    }

    #[test]
    fn a_directory_fifo_and_socket_are_refused_without_blocking() {
        let stage = stage();
        std::fs::create_dir(stage.root.join("out/dir")).unwrap();
        let fifo = stage.root.join("out/fifo");
        let name = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        // SAFETY: `name` is a NUL-terminated path that outlives the call.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let socket_path = stage.root.join("out/sock");
        let _socket = std::os::unix::net::UnixListener::bind(&socket_path);
        for path in ["out/dir", "out/fifo", "out/sock"] {
            if stage.root.join(path).exists() {
                assert_refused_unchanged(&attempt(&stage, path), &stage);
            }
        }
        assert!(stage.root.join("out/dir").is_dir());
        assert!(fifo.exists());
    }

    #[test]
    fn a_device_is_refused() {
        let stage = stage();
        // A device cannot be made here, so the check runs on one through a parent that is
        // not the workspace: `/dev` as the held root.
        let root = File::open("/dev").unwrap();
        let digest = format!("{:x}", Sha256::digest(NEW));
        let binding = binding();
        let result = commit(
            &CommitPlan {
                root: &root,
                root_display: Path::new("/dev"),
                relative: "null",
                bytes: NEW,
                committed_sha256: &digest,
                forbidden: &[],
                binding: &binding,
                output_id: "artifact",
                result_sha256: &"e".repeat(64),
                diagnostics: &[],
            },
            &|| Ok(()),
            &|| false,
        );
        assert_refused_unchanged(&result, &stage);
    }

    #[test]
    fn the_same_inode_as_a_protected_input_is_refused() {
        let stage = stage();
        std::fs::create_dir(stage.root.join("frozen")).unwrap();
        std::fs::write(stage.root.join("frozen/input.txt"), OLD).unwrap();
        let result = attempt_with(
            &stage,
            "frozen/input.txt",
            &["frozen/input.txt".to_string()],
            &|| Ok(()),
            &|| false,
        );
        assert!(result.unwrap_err().to_string().contains("protected input"));
        assert_eq!(
            std::fs::read(stage.root.join("frozen/input.txt")).unwrap(),
            OLD
        );
    }

    #[test]
    fn a_parent_replaced_after_the_walk_decides_is_not_followed() {
        let stage = stage();
        std::fs::write(stage.root.join("out/settings.json"), OLD).unwrap();
        let root = stage.root.clone();
        let outside = stage.outside.clone();
        // The temporary was made in the held parent. Swapping the name leaves that
        // directory's descriptor in force, so the outside directory never receives a file.
        let _reset = test_hook::install(move |stage| {
            if stage == Stage::BeforeRename {
                std::fs::rename(root.join("out"), root.join("moved")).unwrap();
                symlink(&outside, root.join("out")).unwrap();
            }
        });
        let receipt = attempt(&stage, "out/settings.json").unwrap();
        assert!(receipt.committed);
        assert!(std::fs::read_dir(&stage.outside).unwrap().next().is_none());
        assert_eq!(
            std::fs::read(stage.root.join("moved/settings.json")).unwrap(),
            NEW
        );
    }

    #[test]
    fn a_leaf_swapped_for_a_symlink_before_the_last_check_is_refused() {
        let stage = stage();
        let target = stage.root.join("out/settings.json");
        std::fs::write(&target, OLD).unwrap();
        let outside = stage.outside.join("secret");
        std::fs::write(&outside, OLD).unwrap();
        let (swap, link_to) = (target.clone(), outside.clone());
        let _reset = test_hook::install(move |stage| {
            if stage == Stage::AfterTempWrite {
                std::fs::remove_file(&swap).unwrap();
                symlink(&link_to, &swap).unwrap();
            }
        });
        let result = attempt(&stage, "out/settings.json");
        assert_refused_unchanged(&result, &stage);
        assert_eq!(std::fs::read(&outside).unwrap(), OLD);
    }

    /// What is at the one temporary in this directory, replaced by a test at a seam.
    fn the_temporary(directory: &Path) -> PathBuf {
        let names = leftovers(directory);
        assert_eq!(names.len(), 1, "{names:?}");
        directory.join(&names[0])
    }

    /// Whichever seam it happens at, an entry that is not the checked file is never adopted,
    /// and the destination keeps its old bytes.
    #[test]
    fn a_temporary_changed_or_replaced_before_the_rename_is_not_adopted() {
        type Change = fn(&Path, &Path);
        let changes: [(&str, Change); 3] = [
            ("written in place", |temporary, _| {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .open(temporary)
                    .unwrap();
                std::io::Write::write_all(&mut file, b"UNCHECKED!").unwrap();
            }),
            ("another regular file", |temporary, _| {
                std::fs::remove_file(temporary).unwrap();
                std::fs::write(temporary, NEW).unwrap();
            }),
            ("a symlink", |temporary, outside| {
                std::fs::remove_file(temporary).unwrap();
                let target = outside.join("target");
                std::fs::write(&target, NEW).unwrap();
                symlink(&target, temporary).unwrap();
            }),
        ];
        for at in [Stage::AfterTempWrite, Stage::BeforeRename] {
            for (label, change) in changes {
                let stage = stage();
                let target = stage.root.join("out/settings.json");
                std::fs::write(&target, OLD).unwrap();
                let (directory, outside) = (stage.root.join("out"), stage.outside.clone());
                let _reset = test_hook::install(move |now| {
                    if now == at {
                        change(&the_temporary(&directory), &outside);
                    }
                });
                let error = attempt(&stage, "out/settings.json").unwrap_err();
                assert_eq!(error.kind, RefusalKind::CommitFailed, "{at:?} {label}");
                assert_eq!(std::fs::read(&target).unwrap(), OLD, "{at:?} {label}");
                assert!(
                    std::fs::symlink_metadata(&target).unwrap().is_file(),
                    "{at:?} {label}"
                );
                assert!(
                    leftovers(&stage.root.join("out")).is_empty(),
                    "{at:?} {label}"
                );
            }
        }
    }

    #[test]
    fn a_second_name_for_the_temporary_is_not_adopted() {
        let stage = stage();
        let directory = stage.root.join("out");
        let other = stage.outside.join("second-name");
        let _reset = test_hook::install(move |now| {
            if now == Stage::BeforeRename {
                std::fs::hard_link(the_temporary(&directory), &other).unwrap();
            }
        });
        let error = attempt(&stage, "out/settings.json").unwrap_err();
        assert_eq!(error.kind, RefusalKind::CommitFailed);
        assert!(!stage.root.join("out/settings.json").exists());
    }

    /// The instant after the rename: the name no longer holds what was checked. This cannot
    /// be undone, and it is not reported as a success.
    #[test]
    fn a_destination_changed_right_after_the_rename_gets_no_receipt() {
        type Change = fn(&Path);
        let changes: [(&str, Change); 2] = [
            ("written in place", |target| {
                std::fs::write(target, b"UNCHECKED!").unwrap();
            }),
            ("replaced", |target| {
                std::fs::remove_file(target).unwrap();
                std::fs::write(target, NEW).unwrap();
            }),
        ];
        for (label, change) in changes {
            let stage = stage();
            let target = stage.root.join("out/settings.json");
            let at = target.clone();
            let _reset = test_hook::install(move |now| {
                if now == Stage::AfterRename {
                    change(&at);
                }
            });
            let error = attempt(&stage, "out/settings.json").unwrap_err();
            assert_eq!(error.kind, RefusalKind::CommitFailed, "{label}");
            assert!(error.reason.contains("rename stood"), "{label}: {error}");
        }
    }

    #[test]
    fn the_temporary_is_private_and_only_permission_bits_move_to_the_file() {
        let stage = stage();
        let directory = stage.root.join("out");
        let _reset = test_hook::install(move |now| {
            if now == Stage::AfterTempWrite {
                let mode = std::fs::metadata(the_temporary(&directory)).unwrap().mode();
                assert_eq!(mode & 0o777, 0o600);
            }
        });
        let target = stage.root.join("out/settings.json");
        std::fs::write(&target, OLD).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o6755)).unwrap();
        attempt(&stage, "out/settings.json").unwrap();
        assert_eq!(std::fs::metadata(&target).unwrap().mode() & 0o7777, 0o755);
        std::fs::remove_file(&target).unwrap();
        attempt(&stage, "out/settings.json").unwrap();
        assert_eq!(std::fs::metadata(&target).unwrap().mode() & 0o7777, 0o600);
    }

    #[test]
    fn a_protected_path_that_cannot_be_identified_is_refused_and_a_missing_one_is_not() {
        let stage = stage();
        std::fs::write(stage.outside.join("frozen"), OLD).unwrap();
        symlink(stage.outside.join("frozen"), stage.root.join("frozen.link")).unwrap();
        let error = attempt_with(
            &stage,
            "out/settings.json",
            &["frozen.link".to_string()],
            &|| Ok(()),
            &|| false,
        )
        .unwrap_err();
        assert_eq!(error.kind, RefusalKind::EffectPathRefused);
        assert!(!stage.root.join("out/settings.json").exists());
        let refused = super::preflight_destination(
            &stage.file,
            &stage.root,
            "out/settings.json",
            &["frozen.link".to_string()],
        );
        assert_eq!(refused.unwrap_err().kind, RefusalKind::EffectPathRefused);
        attempt_with(
            &stage,
            "out/settings.json",
            &["absent/frozen".to_string(), "out/absent".to_string()],
            &|| Ok(()),
            &|| false,
        )
        .unwrap();
    }

    #[test]
    fn a_cancel_right_before_the_rename_saves_nothing_and_removes_the_temporary() {
        let stage = stage();
        let target = stage.root.join("out/settings.json");
        std::fs::write(&target, OLD).unwrap();
        let cancel = Rc::new(Cell::new(false));
        let setter = cancel.clone();
        let directory = stage.root.join("out");
        let saw_temporary = Rc::new(Cell::new(false));
        let saw = saw_temporary.clone();
        let _reset = test_hook::install(move |stage| {
            if stage == Stage::BeforeRename {
                saw.set(!leftovers(&directory).is_empty());
                setter.set(true);
            }
        });
        let result = attempt_with(&stage, "out/settings.json", &[], &|| Ok(()), &|| {
            cancel.get()
        });
        assert!(saw_temporary.get(), "the temporary exists at this point");
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert_eq!(std::fs::read(&target).unwrap(), OLD);
        assert!(leftovers(&stage.root.join("out")).is_empty());
    }

    #[test]
    fn a_cancel_right_after_the_rename_keeps_the_receipt_and_the_bytes() {
        let stage = stage();
        let target = stage.root.join("out/settings.json");
        std::fs::write(&target, OLD).unwrap();
        let cancel = Rc::new(Cell::new(false));
        let setter = cancel.clone();
        let _reset = test_hook::install(move |stage| {
            if stage == Stage::AfterRename {
                setter.set(true);
            }
        });
        let receipt = attempt_with(&stage, "out/settings.json", &[], &|| Ok(()), &|| {
            cancel.get()
        })
        .unwrap();
        assert!(receipt.committed);
        assert!(receipt.cancelled_after_commit);
        assert_eq!(std::fs::read(&target).unwrap(), NEW);
        assert_eq!(
            receipt.committed_sha256,
            format!("{:x}", Sha256::digest(NEW))
        );
    }

    #[test]
    fn a_directory_fsync_failure_after_the_rename_is_not_a_failed_commit() {
        let stage = stage();
        let _reset = test_hook::install(|_| {});
        test_hook::fail_directory_sync();
        let receipt = attempt(&stage, "out/settings.json").unwrap();
        assert!(receipt.committed);
        assert!(!receipt.durable);
        assert_eq!(
            std::fs::read(stage.root.join("out/settings.json")).unwrap(),
            NEW
        );
    }

    #[test]
    fn the_final_check_runs_after_the_temporary_and_a_refusal_leaves_no_trace() {
        let stage = stage();
        let target = stage.root.join("out/settings.json");
        std::fs::write(&target, OLD).unwrap();
        let calls = Cell::new(0);
        let result = attempt_with(
            &stage,
            "out/settings.json",
            &[],
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() >= 2 {
                    Err(ProfileToolRefusal::new(
                        RefusalKind::EffectReadOnly,
                        "now read-only",
                    ))
                } else {
                    Ok(())
                }
            },
            &|| false,
        );
        assert_eq!(calls.get(), 2);
        assert_eq!(result.unwrap_err().kind, RefusalKind::EffectReadOnly);
        assert_eq!(std::fs::read(&target).unwrap(), OLD);
        assert!(leftovers(&stage.root.join("out")).is_empty());
    }

    #[test]
    fn bytes_that_differ_from_their_digest_are_refused() {
        let stage = stage();
        let binding = binding();
        let result = commit(
            &CommitPlan {
                root: &stage.file,
                root_display: &stage.root,
                relative: "out/settings.json",
                bytes: NEW,
                committed_sha256: &"0".repeat(64),
                forbidden: &[],
                binding: &binding,
                output_id: "artifact",
                result_sha256: &"e".repeat(64),
                diagnostics: &[],
            },
            &|| Ok(()),
            &|| false,
        );
        assert!(result.is_err());
        assert!(!stage.root.join("out/settings.json").exists());
    }

    /// The plan is judged by itself first. The later read-backs would also refuse these bytes,
    /// but only after making a directory and a temporary file and asking the final check.
    #[test]
    fn a_plan_whose_digest_differs_stops_before_any_check_or_file() {
        let stage = stage();
        let binding = binding();
        let asked = std::cell::Cell::new(false);
        let error = commit(
            &CommitPlan {
                root: &stage.file,
                root_display: &stage.root,
                relative: "fresh/settings.json",
                bytes: NEW,
                committed_sha256: &"0".repeat(64),
                forbidden: &[],
                binding: &binding,
                output_id: "artifact",
                result_sha256: &"e".repeat(64),
                diagnostics: &[],
            },
            &|| {
                asked.set(true);
                Ok(())
            },
            &|| false,
        )
        .unwrap_err();
        assert!(error.reason.contains("differ from their digest"), "{error}");
        assert!(!asked.get());
        assert!(!stage.root.join("fresh").exists());
    }

    #[test]
    fn the_order_is_temporary_then_fsync_then_rename() {
        let stage = stage();
        let target = stage.root.join("out/settings.json");
        std::fs::write(&target, OLD).unwrap();
        let directory = stage.root.join("out");
        let observed_target = target.clone();
        let _reset = test_hook::install(move |stage| {
            let temporaries = leftovers(&directory);
            let on_disk = std::fs::read(&observed_target).unwrap();
            match stage {
                Stage::AfterTempWrite | Stage::BeforeRename => {
                    assert_eq!(temporaries.len(), 1);
                    let temporary = std::fs::read(directory.join(&temporaries[0])).unwrap();
                    assert_eq!(temporary, NEW, "the temporary holds every byte");
                    assert_eq!(on_disk, OLD, "the destination has not moved");
                }
                Stage::AfterRename => {
                    assert!(temporaries.is_empty());
                    assert_eq!(on_disk, NEW);
                }
            }
        });
        attempt(&stage, "out/settings.json").unwrap();
    }
}
