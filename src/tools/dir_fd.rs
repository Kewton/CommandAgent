//! fd-relative, no-symlink traversal for the Write and Edit leaf tools.
//!
//! Issue #505: the previous Write/Edit flow inspected each path component with
//! `symlink_metadata`/`canonicalize` and then opened the file again by path, so
//! a concurrent swap of a parent directory for a symlink (TOCTOU) could redirect
//! the write outside the workspace. This leaf walks the workspace from a single
//! directory fd with `openat(..., O_NOFOLLOW | O_DIRECTORY)` and holds the final
//! parent fd for the whole read/write, so no component is ever re-resolved by
//! path after the check.
//!
//! The existing path-based pre-checks in `write.rs` stay in place and keep their
//! wording and classification; the fd walk here is the final authority. Because
//! no component ever follows a symlink, a parent directory that is a symlink is
//! refused even when it points back inside the workspace.

use std::path::Path;

#[cfg(unix)]
mod unix_impl {
    use std::ffi::{CString, OsStr, OsString};
    use std::fs::File;
    use std::io::{self, Read as _, Write as _};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Component, Path, PathBuf};

    use anyhow::{Context, bail};

    use super::fire_seam;

    /// A resolved parent directory fd plus the leaf name to open relative to it.
    /// Holding the fd for the whole operation is what closes the race: the leaf
    /// is always reached from this fd, never from a re-walked path.
    #[derive(Debug)]
    pub(crate) struct ParentDir {
        dir: File,
        name: CString,
        path: PathBuf,
    }

    impl ParentDir {
        /// Read the leaf file relative to the held parent fd. Uses `O_NOFOLLOW`
        /// so a leaf swapped for a symlink is refused instead of followed.
        pub(crate) fn read_leaf_to_string(&self) -> anyhow::Result<String> {
            let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            let mut file = match openat_file(&self.dir, &self.name, flags, 0) {
                Ok(file) => file,
                Err(err) => return Err(self.leaf_error(err)),
            };
            let mut content = String::new();
            // A raw `io::Error` (for example "Is a directory" or "No such file
            // or directory") is returned unchanged so the existing error
            // classification and wording are preserved.
            file.read_to_string(&mut content)?;
            Ok(content)
        }

        /// Create or replace the leaf file relative to the held parent fd, in
        /// place (`O_TRUNC`, not a rename). `O_NOFOLLOW` refuses a leaf symlink.
        pub(crate) fn write_leaf(&self, content: &str) -> anyhow::Result<()> {
            let flags =
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            let mut file = match openat_file(&self.dir, &self.name, flags, 0o666) {
                Ok(file) => file,
                Err(err) => return Err(self.leaf_error(err)),
            };
            file.write_all(content.as_bytes())
                .with_context(|| format!("failed to write {}", self.path.display()))?;
            file.flush()
                .with_context(|| format!("failed to flush {}", self.path.display()))?;
            Ok(())
        }

        fn leaf_error(&self, err: io::Error) -> anyhow::Error {
            if err.raw_os_error() == Some(libc::ELOOP) {
                return anyhow::anyhow!(
                    "symlink_write_blocked: refusing to write through {}",
                    self.path.display()
                );
            }
            anyhow::Error::new(err).context(format!(
                "failed to open {} for writing",
                self.path.display()
            ))
        }
    }

    /// Open the target's parent directory from the workspace root, creating
    /// missing intermediate directories with `mkdirat` when `create` is set.
    pub(crate) fn open_parent(root: &Path, path: &Path, create: bool) -> anyhow::Result<ParentDir> {
        let raw_root = root.to_path_buf();
        let root = root
            .canonicalize()
            .with_context(|| format!("workspace root is not accessible: {}", raw_root.display()))?;
        let candidate = if path.is_absolute() && path.starts_with(&raw_root) {
            root.join(
                path.strip_prefix(&raw_root)
                    .with_context(|| format!("path escapes workspace: {}", path.display()))?,
            )
        } else if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        let relative = candidate
            .strip_prefix(&root)
            .with_context(|| format!("path escapes workspace: {}", candidate.display()))?;

        let mut names: Vec<OsString> = Vec::new();
        for component in relative.components() {
            let Component::Normal(part) = component else {
                bail!("path escapes workspace: {}", candidate.display());
            };
            names.push(part.to_os_string());
        }
        let Some(leaf) = names.pop() else {
            bail!("path escapes workspace: {}", candidate.display());
        };
        let name = CString::new(leaf.as_bytes())
            .with_context(|| format!("path is not valid UTF-8: {}", candidate.display()))?;

        let mut dir = File::open(&root)
            .with_context(|| format!("workspace root is not accessible: {}", raw_root.display()))?;
        let mut current = root.clone();
        for part in &names {
            current.push(part);
            fire_seam(&current);
            dir = open_component(dir, part, &current, create)?;
        }
        Ok(ParentDir {
            dir,
            name,
            path: candidate,
        })
    }

    fn open_component(
        dir: File,
        part: &OsStr,
        display: &Path,
        create: bool,
    ) -> anyhow::Result<File> {
        let name = CString::new(part.as_bytes())
            .with_context(|| format!("path is not valid UTF-8: {}", display.display()))?;
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        match openat_file(&dir, &name, flags, 0) {
            Ok(file) => Ok(file),
            Err(err) if err.kind() == io::ErrorKind::NotFound && create => {
                // SAFETY: dir owns its fd; name is NUL-terminated.
                let rc = unsafe { libc::mkdirat(dir.as_raw_fd(), name.as_ptr(), 0o777) };
                if rc != 0 {
                    let mkdir_err = io::Error::last_os_error();
                    if mkdir_err.kind() != io::ErrorKind::AlreadyExists {
                        return Err(mkdir_err).with_context(|| {
                            format!("failed to create directory {}", display.display())
                        });
                    }
                }
                openat_file(&dir, &name, flags, 0).map_err(|err| component_error(err, display))
            }
            Err(err) => Err(component_error(err, display)),
        }
    }

    fn component_error(err: io::Error, display: &Path) -> anyhow::Error {
        // Linux reports `ELOOP`, macOS reports `ENOTDIR`, for a trailing symlink
        // opened with `O_NOFOLLOW`; `lstat` distinguishes a symlink from a plain
        // non-directory so the escape message stays exact on both.
        let is_symlink = std::fs::symlink_metadata(display)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false);
        if err.raw_os_error() == Some(libc::ELOOP) || is_symlink {
            return anyhow::anyhow!(
                "path escapes workspace through existing component {}; use workspace-relative paths",
                display.display()
            );
        }
        anyhow::Error::new(err).context(format!(
            "path component is not accessible: {}",
            display.display()
        ))
    }

    /// Open the leaf below `root` through a no-follow fd walk for reading.
    ///
    /// Issue #506: the GUI read paths used to `canonicalize`/`lstat` a path and
    /// then read it again by path, so a concurrent swap of a parent directory
    /// (or the leaf) for a symlink could redirect the read outside the root.
    /// Every component below the (canonicalized) root is opened with
    /// `O_NOFOLLOW | O_DIRECTORY` and the leaf with
    /// `O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC`, and the returned handle
    /// is the only reference to the leaf: callers read from it directly, so no
    /// component is ever re-resolved by path between a check and the read. The
    /// leaf is `fstat`ed and only an ordinary file is returned, so a directory,
    /// FIFO, socket, or device is refused without blocking the open. A parent or
    /// leaf that is a symlink is refused even when it points back inside `root`.
    pub fn open_read_file(root: &Path, path: &Path) -> anyhow::Result<File> {
        let raw_root = root.to_path_buf();
        let root = root
            .canonicalize()
            .with_context(|| format!("root is not accessible: {}", raw_root.display()))?;
        let candidate = if path.is_absolute() && path.starts_with(&raw_root) {
            root.join(
                path.strip_prefix(&raw_root)
                    .with_context(|| format!("path escapes root: {}", path.display()))?,
            )
        } else if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        let relative = candidate
            .strip_prefix(&root)
            .with_context(|| format!("path escapes root: {}", candidate.display()))?;

        let mut names: Vec<OsString> = Vec::new();
        for component in relative.components() {
            let Component::Normal(part) = component else {
                bail!("path escapes root: {}", candidate.display());
            };
            names.push(part.to_os_string());
        }
        let Some(leaf) = names.pop() else {
            bail!("path escapes root: {}", candidate.display());
        };
        let name = CString::new(leaf.as_bytes())
            .with_context(|| format!("path is not valid UTF-8: {}", candidate.display()))?;

        let mut dir = File::open(&root)
            .with_context(|| format!("root is not accessible: {}", raw_root.display()))?;
        let mut current = root.clone();
        for part in &names {
            current.push(part);
            fire_seam(&current);
            dir = open_component(dir, part, &current, false)?;
        }
        fire_seam(&candidate);
        let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC;
        let file =
            openat_file(&dir, &name, flags, 0).map_err(|err| leaf_read_error(err, &candidate))?;
        let metadata = file
            .metadata()
            .with_context(|| format!("inspect {}", candidate.display()))?;
        if !metadata.is_file() {
            bail!("not a regular file: {}", candidate.display());
        }
        Ok(file)
    }

    fn leaf_read_error(err: io::Error, display: &Path) -> anyhow::Error {
        // As in `component_error`, Linux reports `ELOOP` and macOS reports
        // `ENOTDIR` for a trailing symlink opened with `O_NOFOLLOW`; `lstat`
        // distinguishes a symlink from a plain non-file so the message stays
        // exact on both.
        let is_symlink = std::fs::symlink_metadata(display)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false);
        if err.raw_os_error() == Some(libc::ELOOP) || is_symlink {
            return anyhow::anyhow!("refusing to read through symlink {}", display.display());
        }
        anyhow::Error::new(err).context(format!("failed to open {} for reading", display.display()))
    }

    fn openat_file(dir: &File, name: &CString, flags: i32, mode: libc::c_uint) -> io::Result<File> {
        // SAFETY: dir owns its fd; name is NUL-terminated. The returned fd is
        // checked and transferred to exactly one File owner.
        let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags, mode) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat returned a fresh valid fd owned by this function.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

#[cfg(unix)]
pub(crate) use unix_impl::{ParentDir, open_parent};

/// fd-relative, no-follow read entry (Issue #506). Unix uses the `openat` walk;
/// other platforms fall back to a path-based read with a canonical-containment
/// check, matching the pre-#506 behavior the GUI keeps off Unix.
#[cfg(unix)]
pub use unix_impl::open_read_file;

#[cfg(not(unix))]
pub fn open_read_file(root: &Path, path: &Path) -> anyhow::Result<std::fs::File> {
    use anyhow::{Context, bail};

    let root = root
        .canonicalize()
        .with_context(|| format!("root is not accessible: {}", root.display()))?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let canonical = candidate
        .canonicalize()
        .with_context(|| format!("path is not accessible: {}", candidate.display()))?;
    if !canonical.starts_with(&root) {
        bail!("path escapes root: {}", candidate.display());
    }
    let file = std::fs::File::open(&canonical)
        .with_context(|| format!("failed to open {} for reading", canonical.display()))?;
    if !file.metadata()?.is_file() {
        bail!("not a regular file: {}", canonical.display());
    }
    Ok(file)
}

/// Fires the test-only race seam at a traversal decision point. A no-op in
/// production builds; the [`test_hook`] module is compiled only under
/// `cfg(test)`.
#[cfg(test)]
pub(crate) fn fire_seam(component: &Path) {
    test_hook::before_open(component);
}

#[cfg(not(test))]
pub(crate) fn fire_seam(_component: &Path) {}

/// Test-only, one-shot race seam, absent from production builds. Same shape as
/// `read_missing/test_hook.rs`, with one difference: the fd walk reaches several
/// decision points (after the pre-checks, before the fd walk, and immediately
/// before each component is opened), so the installed closure is invoked at each
/// point instead of exactly once. It is still a single controlled swap: the
/// closure decides which component to replace, and any replacement makes the
/// next `openat` fail before the swap target is touched.
#[cfg(test)]
pub(crate) mod test_hook {
    use std::cell::RefCell;
    use std::path::Path;

    type Hook = Box<dyn FnMut(&Path)>;
    thread_local! {
        static BEFORE_OPEN: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    /// Clears the installed hook when dropped, so a test cannot leak it into the
    /// next test on the same thread.
    pub(crate) struct Reset;

    impl Drop for Reset {
        fn drop(&mut self) {
            BEFORE_OPEN.with(|slot| {
                slot.borrow_mut().take();
            });
        }
    }

    /// Install a swap closure and return the guard that removes it.
    pub(crate) fn install(hook: impl FnMut(&Path) + 'static) -> Reset {
        BEFORE_OPEN.with(|slot| {
            assert!(slot.borrow().is_none(), "a race hook is already installed");
            *slot.borrow_mut() = Some(Box::new(hook));
        });
        Reset
    }

    /// Invoke the installed closure, if any, with the component about to be opened.
    pub(crate) fn before_open(component: &Path) {
        BEFORE_OPEN.with(|slot| {
            if let Some(hook) = slot.borrow_mut().as_mut() {
                hook(component);
            }
        });
    }
}

#[cfg(all(test, unix))]
mod tests {
    //! Direct unit tests for the fd-relative walker, including the deterministic
    //! swap seam. Synthetic values only, inside a tempdir; the "outside"
    //! directory is a sibling of the workspace, never a real host path.

    use std::fs;
    use std::io::Read as _;
    use std::path::{Path, PathBuf};

    use super::open_parent;
    use super::open_read_file;
    use super::test_hook::install;

    const PAYLOAD: &str = "PAYLOAD_505";
    const OUTSIDE_CANARY: &str = "OUTSIDE_CANARY_505";
    const READ_CANARY: &str = "H01_506_FAKE_outside_secret";

    fn outside_of(root: &Path) -> PathBuf {
        let parent = root.parent().expect("workspace has a parent");
        let outside = parent.join("outside_seam_505");
        fs::create_dir_all(&outside).unwrap();
        outside
    }

    fn entries(dir: &Path) -> Vec<std::ffi::OsString> {
        match fs::read_dir(dir) {
            Ok(entries) => entries.map(|entry| entry.unwrap().file_name()).collect(),
            Err(_) => Vec::new(),
        }
    }

    #[test]
    fn component_seam_swaps_deep_parent_and_refuses_outside() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(root.join("a/b/c")).unwrap();
        let outside = outside_of(&root);

        let swapped = root.join("a/b");
        let target = root.join("a/b/c/f.txt");
        let outside_target = outside.clone();
        let _reset = install(move |component: &Path| {
            if component.file_name().and_then(|name| name.to_str()) == Some("b") {
                let _ = fs::remove_dir_all(&swapped);
                std::os::unix::fs::symlink(&outside_target, &swapped).unwrap();
            }
        });

        let err = open_parent(&root, &target, true).unwrap_err();
        assert!(
            err.to_string()
                .contains("path escapes workspace through existing component"),
            "{err}"
        );
        assert!(entries(&outside).is_empty(), "outside dir must stay empty");
        assert!(!outside.join("sub").exists());
    }

    #[test]
    fn inner_symlink_parent_is_refused_even_when_target_is_inside() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(root.join("real")).unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join("link")).unwrap();

        let err = open_parent(&root, &root.join("link/f.txt"), false).unwrap_err();
        assert!(
            err.to_string()
                .contains("path escapes workspace through existing component"),
            "{err}"
        );
        assert!(!root.join("real/f.txt").exists());
    }

    #[test]
    fn write_leaf_creates_nested_file_from_the_held_fd() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(&root).unwrap();

        let parent = open_parent(&root, &root.join("ok/nested/new.txt"), true).unwrap();
        parent.write_leaf(PAYLOAD).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("ok/nested/new.txt")).unwrap(),
            PAYLOAD
        );
    }

    #[test]
    fn missing_parent_without_create_errors_without_creating_anything() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(&root).unwrap();

        let err = open_parent(&root, &root.join("missing/f.txt"), false).unwrap_err();
        assert!(!err.to_string().is_empty());
        assert!(!root.join("missing").exists());
    }

    #[test]
    fn final_leaf_symlink_is_refused_with_the_legacy_message() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(&root).unwrap();
        let outside = outside_of(&root);
        fs::write(outside.join("f.txt"), OUTSIDE_CANARY).unwrap();
        std::os::unix::fs::symlink(outside.join("f.txt"), root.join("leaf")).unwrap();

        let parent = open_parent(&root, &root.join("leaf"), true).unwrap();
        let err = parent.write_leaf(PAYLOAD).unwrap_err();
        assert!(err.to_string().contains("symlink_write_blocked"), "{err}");
        assert_eq!(
            fs::read_to_string(outside.join("f.txt")).unwrap(),
            OUTSIDE_CANARY
        );
    }

    fn read_to_string(root: &Path, path: &Path) -> anyhow::Result<String> {
        let mut file = open_read_file(root, path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Ok(content)
    }

    #[test]
    fn read_file_returns_the_leaf_content_from_the_held_fd() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(root.join("d/e")).unwrap();
        fs::write(root.join("d/e/f.txt"), READ_CANARY).unwrap();

        assert_eq!(
            read_to_string(&root, Path::new("d/e/f.txt")).unwrap(),
            READ_CANARY
        );
    }

    #[test]
    fn read_file_parent_swap_to_outside_symlink_is_refused() {
        // Issue #506, deterministic (a): swap the parent after the walk begins,
        // and prove the read never reaches the outside sibling.
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(root.join("d/e")).unwrap();
        fs::write(root.join("d/e/f.txt"), "inside").unwrap();
        let outside = outside_of(&root);
        fs::write(outside.join("f.txt"), READ_CANARY).unwrap();

        let swapped = root.join("d/e");
        let outside_target = outside.clone();
        let _reset = install(move |component: &Path| {
            if component.file_name().and_then(|name| name.to_str()) == Some("e") {
                let _ = fs::remove_dir_all(&swapped);
                std::os::unix::fs::symlink(&outside_target, &swapped).unwrap();
            }
        });

        let err = read_to_string(&root, Path::new("d/e/f.txt")).unwrap_err();
        assert!(
            err.to_string()
                .contains("path escapes workspace through existing component"),
            "{err}"
        );
        assert_eq!(
            fs::read_to_string(outside.join("f.txt")).unwrap(),
            READ_CANARY,
            "the outside canary must be untouched"
        );
    }

    #[test]
    fn read_file_refuses_inner_symlink_parent_even_when_target_is_inside() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(root.join("real")).unwrap();
        fs::write(root.join("real/f.txt"), READ_CANARY).unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join("link")).unwrap();

        let err = read_to_string(&root, Path::new("link/f.txt")).unwrap_err();
        assert!(
            err.to_string()
                .contains("path escapes workspace through existing component"),
            "{err}"
        );
    }

    #[test]
    fn read_file_refuses_symlink_leaf_without_following_it() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(&root).unwrap();
        let outside = outside_of(&root);
        fs::write(outside.join("secret.txt"), READ_CANARY).unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("leak.txt")).unwrap();

        let err = read_to_string(&root, Path::new("leak.txt")).unwrap_err();
        assert!(
            err.to_string().contains("refusing to read through symlink"),
            "{err}"
        );
    }

    #[test]
    fn read_file_refuses_a_directory_leaf() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(root.join("d.md")).unwrap();

        let err = open_read_file(&root, Path::new("d.md")).unwrap_err();
        assert!(err.to_string().contains("not a regular file"), "{err}");
    }

    #[test]
    fn read_file_refuses_a_fifo_leaf_without_blocking() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt as _;

        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(&root).unwrap();
        let fifo = root.join("pipe.md");
        let name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: the path is a valid NUL-terminated C string inside the tempdir.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);

        let err = open_read_file(&root, Path::new("pipe.md")).unwrap_err();
        assert!(err.to_string().contains("not a regular file"), "{err}");
    }
}
