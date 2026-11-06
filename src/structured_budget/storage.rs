//! The calls a wallet's directories are reached and used by, each on a descriptor held from the
//! walk that judged them: no name is resolved again from the root once it has been judged.
//!
//! A directory on the way is opened for search only where the system has such a mode, so a
//! directory that may be passed through and not listed is passed as before. The wallet's own
//! directory is opened to be read as well, since its contents are fsynced through it.

use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

/// What makes a directory or a file the one it is: its device and inode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Identity {
    device: u64,
    inode: u64,
}

impl Identity {
    pub(super) fn of_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    pub(super) fn of(file: &File) -> io::Result<Self> {
        file.metadata().map(|metadata| Self::of_metadata(&metadata))
    }
}

/// A name in a held directory, as it is now, with no link followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Found {
    Link,
    /// Anything else; only a directory can be opened as one.
    Entry(Identity),
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const SEARCH_ONLY: libc::c_int = libc::O_PATH;
#[cfg(target_os = "macos")]
const SEARCH_ONLY: libc::c_int = libc::O_SEARCH;
#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
const SEARCH_ONLY: libc::c_int = libc::O_RDONLY;

/// A directory on the way, never through a link at its own name.
const ON_THE_WAY: libc::c_int =
    SEARCH_ONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
/// A directory this module made, never through a link at its own name.
const MADE: libc::c_int = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;

fn name_of(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

fn checked(rc: libc::c_int) -> io::Result<()> {
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// The root, the one place a walk starts from.
pub(super) fn open_root() -> io::Result<File> {
    let root = CString::new("/").map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: `root` is NUL-terminated and outlives the call; the descriptor returned is owned
    // by the `File` made from it and by nothing else.
    let fd = unsafe {
        libc::open(
            root.as_ptr(),
            SEARCH_ONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    owned(fd)
}

fn owned(fd: libc::c_int) -> io::Result<File> {
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` was just returned open by the system, and nothing else owns it.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn open_at(
    directory: &File,
    name: &OsStr,
    flags: libc::c_int,
    mode: libc::c_uint,
) -> io::Result<File> {
    let name = name_of(name)?;
    // SAFETY: the directory's descriptor is open for the call, the name is NUL-terminated, and
    // the descriptor returned is owned by the `File` made from it.
    let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags, mode) };
    owned(fd)
}

/// What a name in `directory` is, with no link followed, or `None` when nothing is there.
pub(super) fn look_at(directory: &File, name: &OsStr) -> io::Result<Option<Found>> {
    let name = name_of(name)?;
    // SAFETY: an all-zero `stat` is a valid value of this plain struct.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: the directory's descriptor is open, the name is NUL-terminated, and `stat` is
    // writable; all three outlive the call.
    let rc = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            &raw mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    match checked(rc) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    if stat.st_mode & libc::S_IFMT == libc::S_IFLNK {
        return Ok(Some(Found::Link));
    }
    #[allow(
        clippy::unnecessary_cast,
        reason = "the width of these fields differs between platforms"
    )]
    Ok(Some(Found::Entry(Identity {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
    })))
}

/// The text of a link in `directory`, as it is written.
pub(super) fn read_link_at(directory: &File, name: &OsStr) -> io::Result<PathBuf> {
    let name = name_of(name)?;
    let mut buffer = vec![0_u8; 256];
    loop {
        // SAFETY: the directory's descriptor is open, the name is NUL-terminated, and the
        // buffer is writable for the length given; all outlive the call.
        let length = unsafe {
            libc::readlinkat(
                directory.as_raw_fd(),
                name.as_ptr(),
                buffer.as_mut_ptr().cast::<libc::c_char>(),
                buffer.len(),
            )
        };
        let length = usize::try_from(length).map_err(|_| io::Error::last_os_error())?;
        if length < buffer.len() {
            buffer.truncate(length);
            return Ok(PathBuf::from(OsStr::from_bytes(&buffer)));
        }
        // The text may have been cut at the buffer's end: read it again into twice the room.
        buffer.resize(buffer.len() * 2, 0);
    }
}

/// A directory on the way, opened by its name in the directory before it.
pub(super) fn open_on_the_way(directory: &File, name: &OsStr) -> io::Result<File> {
    open_at(directory, name, ON_THE_WAY, 0)
}

/// A directory this module made, opened by its name in the directory it was made in.
pub(super) fn open_made(directory: &File, name: &OsStr) -> io::Result<File> {
    open_at(directory, name, MADE, 0)
}

/// The held directory itself, to be read and fsynced through.
pub(super) fn open_itself(directory: &File) -> io::Result<File> {
    open_at(directory, OsStr::new("."), MADE, 0)
}

/// A new directory, private to this user. One already there is an error, never a success.
pub(super) fn make_directory_at(directory: &File, name: &OsStr) -> io::Result<()> {
    let name = name_of(name)?;
    // SAFETY: the directory's descriptor is open and the name is NUL-terminated for the call.
    checked(unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) })
}

/// A file in `directory`, with the flags and mode of the one caller that knows what it is for.
pub(super) fn open_file_at(
    directory: &File,
    name: &str,
    flags: libc::c_int,
    mode: libc::c_uint,
) -> io::Result<File> {
    open_at(directory, OsStr::new(name), flags | libc::O_CLOEXEC, mode)
}

/// One name replaced by another, both in the one held directory.
pub(super) fn rename_at(directory: &File, from: &str, to: &str) -> io::Result<()> {
    let (from, to) = (name_of(OsStr::new(from))?, name_of(OsStr::new(to))?);
    let fd = directory.as_raw_fd();
    // SAFETY: the directory's descriptor is open and both names are NUL-terminated for the call.
    checked(unsafe { libc::renameat(fd, from.as_ptr(), fd, to.as_ptr()) })
}
