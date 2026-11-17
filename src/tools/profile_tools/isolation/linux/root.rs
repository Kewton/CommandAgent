//! What the private root of one launch holds, worked out in the parent before the fork (#982).
//!
//! The root is an empty `tmpfs`. Into it go, each bound read-only at its own host path: the
//! pinned executable, its ELF interpreter and the libraries it needs, each pinned directory,
//! and `/dev/null`, `/dev/urandom`, and `/dev/random`. A symbolic link on the way to any of
//! them is made again inside, so the loader and the tool find each at the path they ask for.
//! Nothing else of the host is there: no home, no workspace, no temporary directory, no `/etc`,
//! and no `/proc`. Landlock then says what may be read, run, or written among what is there.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::fs::File;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::FileExt as _;
use std::path::{Component, Path, PathBuf};

/// What a bound path may be used for, under Landlock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Grant {
    /// There, with no rule: its metadata shows and its contents are refused (`EACCES`).
    Visible,
    /// Read, and listed when it is a directory.
    Read,
    /// Read and run: the pinned executable and its ELF interpreter, nothing else.
    Run,
    /// Read and written: `/dev/null` only.
    ReadWrite,
}

/// One change to the mount tree, made by the child in order. Every path lies under the root.
#[derive(Debug)]
pub(super) enum Op {
    Directory(CString),
    /// An empty file to bind a file or a device over.
    File(CString),
    Link {
        target: CString,
        path: CString,
    },
    /// A bind of `source` over `target`, then a remount of it read-only with `remount` flags:
    /// the flags of the mount it comes from, which a user namespace may not drop.
    Bind {
        source: CString,
        target: CString,
        remount: libc::c_ulong,
    },
}

/// The root of one launch: the steps that build it, and what each bound path may do.
#[derive(Debug)]
pub(super) struct Plan {
    pub(super) ops: Vec<Op>,
    pub(super) grants: BTreeMap<PathBuf, Grant>,
}

/// What one launch asks for. Its working directory lies inside a pinned directory, which the
/// descriptor's own check holds, so it is there through that directory's bind.
pub(super) struct Wanted<'a> {
    pub(super) executable: &'a Path,
    pub(super) read: &'a [&'a Path],
    pub(super) visible: &'a [&'a Path],
}

pub(super) const DEVICES: [(&str, Grant); 3] = [
    ("/dev/null", Grant::ReadWrite),
    ("/dev/urandom", Grant::Read),
    ("/dev/random", Grant::Read),
];

/// More links than this on the way to one path is a loop.
const MAX_LINKS: usize = 40;

pub(super) fn cstring(path: &Path) -> Result<CString, String> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| format!("`{}` holds a NUL", path.display()))
}

/// `.` and `..` taken away by their names, as the kernel would on the way down.
fn normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::Normal(name) => out.push(name),
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    out
}

/// The flags of the mount holding `path` that a read-only remount inside a user namespace must
/// keep. Access-time flags are left out, so the kernel keeps the mount's own.
fn locked_flags(path: &Path) -> Result<libc::c_ulong, String> {
    let name = cstring(path)?;
    // SAFETY: an all-zero `statvfs` is a valid value of this plain struct.
    let mut answer: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `name` is NUL-terminated and `answer` is writable; both outlive the call.
    if unsafe { libc::statvfs(name.as_ptr(), &raw mut answer) } != 0 {
        return Err(format!(
            "the mount of `{}` cannot be read: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    let mut flags = 0;
    for (held, keep) in [
        (libc::ST_NOSUID, libc::MS_NOSUID),
        (libc::ST_NODEV, libc::MS_NODEV),
        (libc::ST_NOEXEC, libc::MS_NOEXEC),
    ] {
        if answer.f_flag & held != 0 {
            flags |= keep;
        }
    }
    Ok(flags)
}

struct Mirror {
    root: PathBuf,
    ops: Vec<Op>,
    directories: BTreeSet<PathBuf>,
    links: BTreeSet<PathBuf>,
    grants: BTreeMap<PathBuf, Grant>,
}

impl Mirror {
    fn inside(&self, path: &Path) -> PathBuf {
        self.root.join(path.strip_prefix("/").unwrap_or(path))
    }

    fn directory(&mut self, path: &Path) -> Result<(), String> {
        let mut at = PathBuf::from("/");
        for component in path.components().skip(1) {
            at.push(component);
            if self.directories.insert(at.clone()) {
                let inside = cstring(&self.inside(&at))?;
                self.ops.push(Op::Directory(inside));
            }
        }
        Ok(())
    }

    /// Each link on the way to `path` is made again, and the real path it leads to is bound.
    fn add(&mut self, path: &Path, grant: Grant) -> Result<(), String> {
        let mut path = normal(path);
        for _ in 0..MAX_LINKS {
            match self.step(&path, grant)? {
                Some(next) => path = normal(&next),
                None => return Ok(()),
            }
        }
        Err(format!(
            "`{}` has too many links on its way",
            path.display()
        ))
    }

    /// One walk down `path`: the first link met, as the path it leads to, or the path bound.
    fn step(&mut self, path: &Path, grant: Grant) -> Result<Option<PathBuf>, String> {
        let names = path.components().skip(1).collect::<Vec<_>>();
        let mut at = PathBuf::from("/");
        for (index, name) in names.iter().enumerate() {
            let next = at.join(name);
            let metadata = std::fs::symlink_metadata(&next)
                .map_err(|error| format!("`{}` cannot be read: {error}", next.display()))?;
            if metadata.file_type().is_symlink() {
                let target = std::fs::read_link(&next)
                    .map_err(|error| format!("`{}` cannot be read: {error}", next.display()))?;
                if self.links.insert(next.clone()) {
                    self.directory(&at)?;
                    let link = Op::Link {
                        target: cstring(&target)?,
                        path: cstring(&self.inside(&next))?,
                    };
                    self.ops.push(link);
                }
                let mut resolved = if target.is_absolute() {
                    target
                } else {
                    at.join(target)
                };
                resolved.extend(names.iter().skip(index + 1));
                return Ok(Some(resolved));
            }
            at = next;
        }
        let entry = self.grants.entry(at).or_insert(grant);
        *entry = (*entry).max(grant);
        Ok(None)
    }

    /// The binds, each made once: a path under a bound directory is there already, and keeps
    /// its own rule.
    fn finish(mut self) -> Result<Plan, String> {
        let directories = self
            .grants
            .keys()
            .filter(|path| path.is_dir())
            .cloned()
            .collect::<BTreeSet<_>>();
        let bound = self.grants.keys().cloned().collect::<Vec<_>>();
        for real in bound {
            if real
                .ancestors()
                .skip(1)
                .any(|above| directories.contains(above))
            {
                continue;
            }
            self.directory(real.parent().unwrap_or(Path::new("/")))?;
            let target = cstring(&self.inside(&real))?;
            if directories.contains(&real) {
                if self.directories.insert(real.clone()) {
                    self.ops.push(Op::Directory(target.clone()));
                }
            } else {
                self.ops.push(Op::File(target.clone()));
            }
            let remount = libc::MS_REMOUNT | libc::MS_BIND | libc::MS_RDONLY | locked_flags(&real)?;
            self.ops.push(Op::Bind {
                source: cstring(&real)?,
                target,
                remount,
            });
        }
        Ok(Plan {
            ops: self.ops,
            grants: self.grants,
        })
    }
}

/// The root for `wanted`, built under `root`, the empty directory the child mounts it on.
pub(super) fn plan(wanted: &Wanted<'_>, root: &Path) -> Result<Plan, String> {
    let mut mirror = Mirror {
        root: root.to_path_buf(),
        ops: Vec::new(),
        directories: BTreeSet::new(),
        links: BTreeSet::new(),
        grants: BTreeMap::new(),
    };
    mirror.add(wanted.executable, Grant::Run)?;
    let loaded = closure(wanted.executable)?;
    for interpreter in &loaded.interpreters {
        mirror.add(interpreter, Grant::Run)?;
    }
    for library in &loaded.libraries {
        mirror.add(library, Grant::Read)?;
    }
    for directory in wanted.read {
        mirror.add(directory, Grant::Read)?;
    }
    for directory in wanted.visible {
        mirror.add(directory, Grant::Visible)?;
    }
    for (device, grant) in DEVICES {
        mirror.add(Path::new(device), grant)?;
    }
    mirror.finish()
}

// ---------------------------------------------------------------------------
// The ELF interpreter and the libraries an executable needs.
// ---------------------------------------------------------------------------

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_STRTAB: u64 = 5;
const DT_RPATH: u64 = 15;
const DT_RUNPATH: u64 = 29;
/// More than this many program headers or dynamic entries is not read.
const MAX_ENTRIES: usize = 4096;
const MAX_NAME: usize = 4096;
const MAX_OBJECTS: usize = 512;

#[derive(Debug, Default)]
pub(super) struct Loaded {
    pub(super) interpreters: BTreeSet<PathBuf>,
    pub(super) libraries: BTreeSet<PathBuf>,
}

/// What one ELF object names.
struct Object {
    interpreter: Option<PathBuf>,
    needed: Vec<String>,
    search: Vec<String>,
}

fn read_at(file: &File, offset: u64, length: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = vec![0; length];
    file.read_exact_at(&mut bytes, offset)?;
    Ok(bytes)
}

fn word(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn half_word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn quarter_word(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

/// A NUL-terminated name at `offset`.
fn name_at(file: &File, offset: u64) -> Result<String, String> {
    let mut bytes = vec![0; MAX_NAME];
    let mut filled = 0;
    while filled < bytes.len() {
        let read = file
            .read_at(&mut bytes[filled..], offset + filled as u64)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        if let Some(end) = bytes[filled..filled + read]
            .iter()
            .position(|byte| *byte == 0)
        {
            bytes.truncate(filled + end);
            return String::from_utf8(bytes).map_err(|_| "a name is not UTF-8".to_string());
        }
        filled += read;
    }
    Err("a name has no end".to_string())
}

/// `None` for a file that is not ELF: a script, which the kernel would start through the
/// interpreter its first line names, and which Landlock then refuses to run.
fn object(path: &Path) -> Result<Option<Object>, String> {
    let bad = |what: &str| {
        format!(
            "`{}` is not an ELF object this can read: {what}",
            path.display()
        )
    };
    let file = File::open(path).map_err(|error| format!("`{}`: {error}", path.display()))?;
    let mut header = [0u8; 64];
    let read = file
        .read_at(&mut header, 0)
        .map_err(|error| error.to_string())?;
    if read < 4 || header[..4] != b"\x7fELF"[..] {
        return Ok(None);
    }
    if read < 64 || header[4] != 2 || header[5] != 1 {
        return Err(bad("only 64-bit little-endian objects run here"));
    }
    let table = word(&header, 32).ok_or_else(|| bad("header"))?;
    let size = usize::from(quarter_word(&header, 54).ok_or_else(|| bad("header"))?);
    let count = usize::from(quarter_word(&header, 56).ok_or_else(|| bad("header"))?);
    if count > MAX_ENTRIES || size < 56 {
        return Err(bad("program headers"));
    }
    let headers = read_at(&file, table, size * count).map_err(|_| bad("program headers"))?;
    let mut loads = Vec::new();
    let mut dynamic = None;
    let mut interpreter = None;
    for index in 0..count {
        let at = index * size;
        let kind = half_word(&headers, at).ok_or_else(|| bad("program header"))?;
        let offset = word(&headers, at + 8).ok_or_else(|| bad("program header"))?;
        let address = word(&headers, at + 16).ok_or_else(|| bad("program header"))?;
        let length = word(&headers, at + 32).ok_or_else(|| bad("program header"))?;
        match kind {
            PT_LOAD => loads.push((address, offset, length)),
            PT_DYNAMIC => dynamic = Some((offset, length)),
            PT_INTERP => interpreter = Some(PathBuf::from(name_at(&file, offset)?)),
            _ => {}
        }
    }
    let mut needed = Vec::new();
    let mut search = Vec::new();
    let mut strings = None;
    if let Some((offset, length)) = dynamic {
        let length = usize::try_from(length).map_err(|_| bad("dynamic section"))?;
        if length / 16 > MAX_ENTRIES {
            return Err(bad("dynamic section"));
        }
        let entries = read_at(&file, offset, length).map_err(|_| bad("dynamic section"))?;
        for at in (0..length / 16).map(|index| index * 16) {
            let tag = word(&entries, at).ok_or_else(|| bad("dynamic entry"))?;
            let value = word(&entries, at + 8).ok_or_else(|| bad("dynamic entry"))?;
            match tag {
                DT_NULL => break,
                DT_NEEDED => needed.push(value),
                DT_STRTAB => strings = Some(value),
                DT_RPATH | DT_RUNPATH => search.push(value),
                _ => {}
            }
        }
    }
    let in_file = |address: u64| {
        loads
            .iter()
            .find(|(start, _, length)| address >= *start && address - start < *length)
            .map(|(start, offset, _)| address - start + offset)
    };
    let table = match strings {
        Some(address) => in_file(address),
        None => None,
    };
    let names = |offsets: &[u64]| -> Result<Vec<String>, String> {
        let table = table.ok_or_else(|| bad("no string table"))?;
        offsets
            .iter()
            .map(|offset| name_at(&file, table + offset))
            .collect()
    };
    Ok(Some(Object {
        interpreter,
        needed: if needed.is_empty() {
            Vec::new()
        } else {
            names(&needed)?
        },
        search: if search.is_empty() {
            Vec::new()
        } else {
            names(&search)?
                .iter()
                .flat_map(|list| list.split(':').map(str::to_string).collect::<Vec<_>>())
                .collect()
        },
    }))
}

/// Where the loader looks when no `ld.so.cache` is there to ask, which is the case inside.
fn system_directories() -> Vec<PathBuf> {
    let triple = if cfg!(target_arch = "x86_64") {
        "x86_64-linux-gnu"
    } else {
        "aarch64-linux-gnu"
    };
    [
        format!("/lib/{triple}"),
        format!("/usr/lib/{triple}"),
        "/lib64".to_string(),
        "/usr/lib64".to_string(),
        "/lib".to_string(),
        "/usr/lib".to_string(),
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

/// The interpreter of `executable` and every library it needs, transitively, as the paths the
/// loader opens. A library that cannot be found is a refusal here, not a tool that fails inside.
pub(super) fn closure(executable: &Path) -> Result<Loaded, String> {
    let mut loaded = Loaded::default();
    let mut pending = vec![executable.to_path_buf()];
    let mut seen = BTreeSet::new();
    while let Some(path) = pending.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        if seen.len() > MAX_OBJECTS {
            return Err(format!("`{}` needs too many objects", executable.display()));
        }
        let Some(object) = object(&path)? else {
            continue;
        };
        if let Some(interpreter) = object.interpreter {
            loaded.interpreters.insert(interpreter);
        }
        let origin = path
            .parent()
            .unwrap_or(Path::new("/"))
            .to_string_lossy()
            .to_string();
        let mut directories = object
            .search
            .iter()
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                PathBuf::from(
                    entry
                        .replace("${ORIGIN}", &origin)
                        .replace("$ORIGIN", &origin),
                )
            })
            .collect::<Vec<_>>();
        directories.extend(system_directories());
        for name in object.needed {
            let found = if name.contains('/') {
                Some(PathBuf::from(&name)).filter(|path| path.is_file())
            } else {
                directories
                    .iter()
                    .map(|directory| directory.join(&name))
                    .find(|path| path.is_file())
            };
            let Some(found) = found else {
                return Err(format!(
                    "`{}` needs `{name}`, which is not in its search path or the system's",
                    path.display()
                ));
            };
            loaded.libraries.insert(found.clone());
            pending.push(found);
        }
    }
    Ok(loaded)
}

#[cfg(test)]
mod tests {
    use std::os::unix::ffi::OsStrExt as _;
    use std::path::{Path, PathBuf};

    use super::{Grant, Op, Wanted, closure, plan};

    fn shell() -> PathBuf {
        std::fs::canonicalize("/bin/bash").unwrap()
    }

    /// The shell's loader and C library are found, as the paths the loader opens.
    #[test]
    fn the_shell_needs_its_interpreter_and_its_c_library() {
        let loaded = closure(&shell()).unwrap();
        assert_eq!(loaded.interpreters.len(), 1, "{loaded:?}");
        assert!(
            loaded.libraries.iter().any(|library| library
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("libc.so")),
            "{loaded:?}"
        );
    }

    #[test]
    fn a_script_has_no_interpreter_here_and_a_broken_object_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("tool.sh");
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        let loaded = closure(&script).unwrap();
        assert!(loaded.interpreters.is_empty() && loaded.libraries.is_empty());
        let broken = temp.path().join("broken");
        std::fs::write(&broken, b"\x7fELF\x01\x01").unwrap();
        assert!(closure(&broken).is_err());
    }

    /// Only the pinned paths and the three devices are bound; a link on the way is made again.
    #[test]
    fn the_root_holds_the_pins_and_nothing_else() {
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let (tool, links, staging) = (base.join("tool"), base.join("links"), base.join("staging"));
        for directory in [&tool, &links, &staging] {
            std::fs::create_dir_all(directory).unwrap();
        }
        std::os::unix::fs::symlink(&tool, base.join("via")).unwrap();
        let via = base.join("via");
        let read = [via.as_path()];
        let visible = [links.as_path()];
        let shell = shell();
        let planned = plan(
            &Wanted {
                executable: &shell,
                read: &read,
                visible: &visible,
            },
            &staging,
        )
        .unwrap();
        assert_eq!(planned.grants.get(&tool), Some(&Grant::Read));
        assert_eq!(planned.grants.get(&links), Some(&Grant::Visible));
        assert_eq!(planned.grants.get(&shell), Some(&Grant::Run));
        assert_eq!(
            planned.grants.get(Path::new("/dev/null")),
            Some(&Grant::ReadWrite)
        );
        for (path, grant) in &planned.grants {
            let pinned = [tool.as_path(), links.as_path(), shell.as_path()]
                .contains(&path.as_path())
                || path.starts_with("/dev/")
                || *grant == Grant::Read && path.to_string_lossy().contains(".so")
                || *grant == Grant::Run && path.to_string_lossy().contains("ld-linux");
            assert!(pinned, "{} {grant:?}", path.display());
        }
        let link = planned
            .ops
            .iter()
            .any(|op| matches!(op, Op::Link { path, .. } if path.to_bytes().ends_with(b"/via")));
        assert!(link, "{:?}", planned.ops);
        for op in &planned.ops {
            let inside = match op {
                Op::Directory(path) | Op::File(path) => path,
                Op::Link { path, .. } => path,
                Op::Bind { target, .. } => target,
            };
            assert!(
                Path::new(std::ffi::OsStr::from_bytes(inside.to_bytes())).starts_with(&staging),
                "{op:?}"
            );
        }
    }
}
