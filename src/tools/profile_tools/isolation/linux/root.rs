//! What the private root of one launch holds, worked out in the parent before the fork (#982).
//!
//! The root is an empty `tmpfs`. Into it go, each bound read-only at its own host path: the
//! pinned executable, its ELF interpreter and the libraries it needs, each pinned directory,
//! and `/dev/null`, `/dev/urandom`, and `/dev/random`. A symbolic link on the way to any of
//! them is made again inside, so the loader and the tool find each at the path they ask for.
//! An interpreter or library the executable names is bound at its real path, every link
//! resolved, and only when that path lies under a system library directory or a pinned
//! directory; anything else, and an interpreter or library that is not a 64-bit ELF object this
//! can read, refuses the launch. The name the loader opens is made again inside only when the
//! kernel's walk of it leads to that same real file.
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

/// Where one walk down a path stopped.
enum Walked {
    /// At a link: the path it leads to, with the rest of the walk after it.
    Link(PathBuf),
    /// At the real path, with no link left on the way.
    Reached(PathBuf),
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

    /// Each link on the way to `path` is made again, and the real path it leads to is bound
    /// with `grant`.
    fn add(&mut self, path: &Path, grant: Grant) -> Result<(), String> {
        let real = self.walk(path)?;
        let entry = self.grants.entry(real).or_insert(grant);
        *entry = (*entry).max(grant);
        Ok(())
    }

    /// The name the loader opens, made again inside with each link on its way, when it leads
    /// to `real`, the file that was checked and is bound: a name that leads anywhere else
    /// refuses the launch. The name itself gets no grant.
    fn name(&mut self, name: &Path, real: &Path) -> Result<(), String> {
        let reached = self.walk(name)?;
        if reached != real {
            return Err(format!(
                "`{}` leads to `{}` here, not to the checked `{}`",
                name.display(),
                reached.display(),
                real.display()
            ));
        }
        Ok(())
    }

    /// The real path `path` leads to, as the kernel walks it: each link met is made again
    /// inside and followed, and `..` goes up from the real directory reached so far, never
    /// from the name as written.
    fn walk(&mut self, path: &Path) -> Result<PathBuf, String> {
        let mut path = path.to_path_buf();
        for _ in 0..MAX_LINKS {
            match self.step(&path)? {
                Walked::Link(next) => path = next,
                Walked::Reached(real) => return Ok(real),
            }
        }
        Err(format!(
            "`{}` has too many links on its way",
            path.display()
        ))
    }

    /// One walk down `path` from `/`: the first link met, as the path it leads to followed by
    /// the rest of `path`, or the real path reached when there is no link.
    fn step(&mut self, path: &Path) -> Result<Walked, String> {
        let components = path.components().collect::<Vec<_>>();
        let mut at = PathBuf::from("/");
        for (index, component) in components.iter().enumerate() {
            let name = match component {
                Component::Normal(name) => name,
                Component::ParentDir => {
                    // The directory left is real (every link before it was followed), and
                    // the loader walks through it inside too.
                    self.directory(&at)?;
                    at.pop();
                    continue;
                }
                Component::RootDir | Component::CurDir | Component::Prefix(_) => continue,
            };
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
                resolved.extend(&components[index + 1..]);
                return Ok(Walked::Link(resolved));
            }
            at = next;
        }
        Ok(Walked::Reached(at))
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
    plan_within(wanted, root, &allowed_roots(wanted.read))
}

/// The same, with the directories an interpreter or a library may really lie in given.
fn plan_within(wanted: &Wanted<'_>, root: &Path, roots: &[PathBuf]) -> Result<Plan, String> {
    let mut mirror = Mirror {
        root: root.to_path_buf(),
        ops: Vec::new(),
        directories: BTreeSet::new(),
        links: BTreeSet::new(),
        grants: BTreeMap::new(),
    };
    mirror.add(wanted.executable, Grant::Run)?;
    let loaded = closure(wanted.executable, roots)?;
    // The grant and the bind go to the real file that was checked; the name the loader asks
    // for is made again inside only when it leads to that same file.
    for (found, grant) in loaded
        .interpreters
        .iter()
        .map(|found| (found, Grant::Run))
        .chain(loaded.libraries.iter().map(|found| (found, Grant::Read)))
    {
        mirror.add(&found.real, grant)?;
        mirror.name(&found.name, &found.real)?;
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
const DT_STRSZ: u64 = 10;
const DT_RPATH: u64 = 15;
const DT_RUNPATH: u64 = 29;
/// The sizes ELF64 fixes: the file header, one program header, one dynamic entry.
const HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;
const DYNAMIC_ENTRY_SIZE: usize = 16;
/// More than this many program headers or dynamic entries is not read.
const MAX_ENTRIES: usize = 4096;
/// A name longer than this, its NUL left out, is not read.
const MAX_NAME: usize = 4096;
const MAX_OBJECTS: usize = 512;

/// One interpreter or library: the name the loader opens, and the real file it was checked as.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Found {
    pub(super) name: PathBuf,
    pub(super) real: PathBuf,
}

#[derive(Debug, Default)]
pub(super) struct Loaded {
    pub(super) interpreters: BTreeSet<Found>,
    pub(super) libraries: BTreeSet<Found>,
}

/// What one ELF object names.
struct Object {
    interpreter: Option<PathBuf>,
    needed: Vec<String>,
    search: Vec<String>,
}

/// `length` bytes at `offset`, only when the whole range lies inside the file's `size`. Every
/// caller bounds `length` first, so nothing larger than a fixed limit is made.
fn read_at(file: &File, size: u64, offset: u64, length: usize) -> Option<Vec<u8>> {
    let end = offset.checked_add(u64::try_from(length).ok()?)?;
    if end > size {
        return None;
    }
    let mut bytes = vec![0; length];
    file.read_exact_at(&mut bytes, offset).ok()?;
    Some(bytes)
}

/// The name that starts `region`: it must end with a NUL inside the region.
fn name_in(region: &[u8]) -> Result<String, String> {
    let end = region
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| "a name has no end".to_string())?;
    String::from_utf8(region[..end].to_vec()).map_err(|_| "a name is not UTF-8".to_string())
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
    let size = file
        .metadata()
        .map_err(|error| format!("`{}`: {error}", path.display()))?
        .len();
    let magic = read_at(&file, size, 0, 4);
    if magic.as_deref() != Some(&b"\x7fELF"[..]) {
        return Ok(None);
    }
    let header = read_at(&file, size, 0, HEADER_SIZE).ok_or_else(|| bad("a cut header"))?;
    if header[4] != 2 || header[5] != 1 || header[6] != 1 {
        return Err(bad("only 64-bit little-endian objects run here"));
    }
    let field = |at: usize| quarter_word(&header, at).map(usize::from);
    if field(52) != Some(HEADER_SIZE) || field(54) != Some(PROGRAM_HEADER_SIZE) {
        return Err(bad("not the sizes of ELF64"));
    }
    let table = word(&header, 32).ok_or_else(|| bad("header"))?;
    let count = field(56).ok_or_else(|| bad("header"))?;
    if count > MAX_ENTRIES {
        return Err(bad("program headers"));
    }
    let headers = read_at(&file, size, table, PROGRAM_HEADER_SIZE * count)
        .ok_or_else(|| bad("program headers"))?;
    // A segment's bytes in the file: every one read here lies inside the file.
    let inside =
        |offset: u64, length: u64| offset.checked_add(length).is_some_and(|end| end <= size);
    let mut loads = Vec::new();
    let mut dynamic = None;
    let mut interpreter = None;
    for index in 0..count {
        let at = index * PROGRAM_HEADER_SIZE;
        let kind = half_word(&headers, at).ok_or_else(|| bad("program header"))?;
        let offset = word(&headers, at + 8).ok_or_else(|| bad("program header"))?;
        let address = word(&headers, at + 16).ok_or_else(|| bad("program header"))?;
        let length = word(&headers, at + 32).ok_or_else(|| bad("program header"))?;
        if matches!(kind, PT_LOAD | PT_DYNAMIC | PT_INTERP) && !inside(offset, length) {
            return Err(bad("a segment past the end of the file"));
        }
        match kind {
            PT_LOAD => loads.push((address, offset, length)),
            PT_DYNAMIC => dynamic = Some((offset, length)),
            PT_INTERP => {
                let length = usize::try_from(length)
                    .ok()
                    .filter(|length| *length <= MAX_NAME + 1)
                    .ok_or_else(|| bad("interpreter"))?;
                let region =
                    read_at(&file, size, offset, length).ok_or_else(|| bad("interpreter"))?;
                interpreter = Some(PathBuf::from(
                    name_in(&region).map_err(|error| bad(&error))?,
                ));
            }
            _ => {}
        }
    }
    let mut needed = Vec::new();
    let mut search = Vec::new();
    let mut strings = None;
    let mut strings_size = None;
    if let Some((offset, length)) = dynamic {
        let length = usize::try_from(length)
            .ok()
            .filter(|length| *length / DYNAMIC_ENTRY_SIZE <= MAX_ENTRIES)
            .ok_or_else(|| bad("dynamic section"))?;
        let entries = read_at(&file, size, offset, length).ok_or_else(|| bad("dynamic section"))?;
        for at in (0..length / DYNAMIC_ENTRY_SIZE).map(|index| index * DYNAMIC_ENTRY_SIZE) {
            let tag = word(&entries, at).ok_or_else(|| bad("dynamic entry"))?;
            let value = word(&entries, at + 8).ok_or_else(|| bad("dynamic entry"))?;
            match tag {
                DT_NULL => break,
                DT_NEEDED => needed.push(value),
                DT_STRTAB => strings = Some(value),
                DT_STRSZ => strings_size = Some(value),
                DT_RPATH | DT_RUNPATH => search.push(value),
                _ => {}
            }
        }
    }
    // Where `length` bytes from `address` lie in the file: inside one loaded segment's bytes.
    let in_file = |address: u64, length: u64| {
        loads.iter().find_map(|(start, offset, held)| {
            let into = address.checked_sub(*start)?;
            if into.checked_add(length)? > *held {
                return None;
            }
            offset.checked_add(into)
        })
    };
    let names = |offsets: &[u64]| -> Result<Vec<String>, String> {
        let (address, length) = strings
            .zip(strings_size)
            .ok_or_else(|| bad("no string table"))?;
        let table = in_file(address, length).ok_or_else(|| bad("string table"))?;
        offsets
            .iter()
            .map(|offset| {
                if *offset >= length {
                    return Err(bad("a name past its table"));
                }
                // To the end of the table, or one name's room and its NUL.
                let room = (length - offset).min(MAX_NAME as u64 + 1);
                let region = table
                    .checked_add(*offset)
                    .and_then(|start| read_at(&file, size, start, usize::try_from(room).ok()?))
                    .ok_or_else(|| bad("string table"))?;
                name_in(&region).map_err(|error| bad(&error))
            })
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

/// Where an interpreter or a library may really be: the system's library directories and the
/// pinned directories, each with its links resolved. One that does not exist adds nothing.
pub(super) fn allowed_roots(pinned: &[&Path]) -> Vec<PathBuf> {
    system_directories()
        .iter()
        .map(PathBuf::as_path)
        .chain(pinned.iter().copied())
        .filter_map(|directory| std::fs::canonicalize(directory).ok())
        .collect()
}

/// The real file `path` leads to, every link resolved, when it is a file under one of `roots`.
/// `by` names it as `what`. This real file is what is read, granted, and bound afterwards.
fn admitted(path: &Path, roots: &[PathBuf], by: &Path, what: &str) -> Result<PathBuf, String> {
    let refused = |why: String| {
        format!(
            "`{}` needs `{what}` at `{}`, {why}",
            by.display(),
            path.display()
        )
    };
    let real = std::fs::canonicalize(path).map_err(|error| refused(error.to_string()))?;
    if !real.is_file() {
        return Err(refused("which is not a file".to_string()));
    }
    if !roots.iter().any(|root| real.starts_with(root)) {
        return Err(refused(format!(
            "which is `{}`, outside the system's library directories and every pinned directory",
            real.display()
        )));
    }
    Ok(real)
}

/// The interpreter of `executable` and every library it needs, transitively, each as the name
/// the loader opens and the real file it was checked as. A library that cannot be found, that
/// lies outside `roots` once its links are resolved, or that is not an ELF object this can read,
/// and an interpreter that is not one, is a refusal here, not a grant and not a tool that fails
/// inside. Every object is read at its real file, the one that is granted.
///
/// An object is searched once per name it is reached by, not once per real file: `$ORIGIN`
/// comes from the name, so the same file reached under two names can need two different
/// leaves, and every one is collected, whatever the loader would map first. The objects are
/// taken in the order they are named (`DT_NEEDED` order, breadth first, as the loader maps
/// them). More than `MAX_OBJECTS` names in all is a refusal.
pub(super) fn closure(executable: &Path, roots: &[PathBuf]) -> Result<Loaded, String> {
    let mut loaded = Loaded::default();
    let mut pending = std::collections::VecDeque::from([Found {
        name: executable.to_path_buf(),
        real: executable.to_path_buf(),
    }]);
    let mut seen = BTreeSet::new();
    while let Some(Found { name: path, real }) = pending.pop_front() {
        if !seen.insert((path.clone(), real.clone())) {
            continue;
        }
        if seen.len() > MAX_OBJECTS {
            return Err(format!("`{}` needs too many objects", executable.display()));
        }
        let Some(object) = object(&real)? else {
            if path == executable {
                continue;
            }
            return Err(format!(
                "`{}` is not an ELF object, and something `{}` loads names it",
                path.display(),
                executable.display()
            ));
        };
        if let Some(interpreter) = object.interpreter {
            let checked = admitted(&interpreter, roots, &path, "its interpreter")?;
            if self::object(&checked)?.is_none() {
                return Err(format!(
                    "`{}` names `{}` as its interpreter, which is not an ELF object",
                    path.display(),
                    interpreter.display()
                ));
            }
            loaded.interpreters.insert(Found {
                name: interpreter,
                real: checked,
            });
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
            let checked = admitted(&found, roots, &path, &name)?;
            let found = Found {
                name: found,
                real: checked,
            };
            loaded.libraries.insert(found.clone());
            pending.push_back(found);
        }
    }
    Ok(loaded)
}

#[cfg(test)]
mod tests {
    use std::os::unix::ffi::OsStrExt as _;
    use std::path::{Path, PathBuf};

    use super::{Grant, Op, Wanted, allowed_roots, closure, object, plan};

    fn shell() -> PathBuf {
        std::fs::canonicalize("/bin/bash").unwrap()
    }

    /// The system's library directories, with no pinned directory.
    fn system() -> Vec<PathBuf> {
        allowed_roots(&[])
    }

    /// The shell's loader and C library are found, as the paths the loader opens.
    #[test]
    fn the_shell_needs_its_interpreter_and_its_c_library() {
        let loaded = closure(&shell(), &system()).unwrap();
        assert_eq!(loaded.interpreters.len(), 1, "{loaded:?}");
        assert!(
            loaded.libraries.iter().any(|library| library
                .name
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
        let loaded = closure(&script, &system()).unwrap();
        assert!(loaded.interpreters.is_empty() && loaded.libraries.is_empty());
        let broken = temp.path().join("broken");
        std::fs::write(&broken, b"\x7fELF\x01\x01").unwrap();
        assert!(closure(&broken, &system()).is_err());
    }

    // -----------------------------------------------------------------------------------------
    // Objects made here: an ELF64 header, its program headers, a string table, an interpreter's
    // name, and a dynamic section, all inside one loaded segment.
    // -----------------------------------------------------------------------------------------

    const BASE: u64 = 0x40_0000;

    /// One object, with where its parts are, so that a test can break one of them.
    struct Made {
        bytes: Vec<u8>,
        /// The program headers: the load, the dynamic section, then the interpreter if any.
        headers: usize,
        dynamic: usize,
        strings_size: usize,
        interpreter: Option<usize>,
    }

    fn put(bytes: &mut [u8], at: usize, value: &[u8]) {
        bytes[at..at + value.len()].copy_from_slice(value);
    }

    fn made(interpreter: Option<&str>, needed: &[&str], search: Option<&str>) -> Made {
        let count = if interpreter.is_some() { 3 } else { 2 };
        let headers = 64;
        let strings = headers + 56 * count;
        let mut table = vec![0u8];
        let mut name_offsets = Vec::new();
        for name in needed.iter().chain(search.iter()) {
            name_offsets.push(table.len() as u64);
            table.extend_from_slice(name.as_bytes());
            table.push(0);
        }
        let strings_size = table.len();
        let interpreter_at = strings + strings_size;
        let mut tail = table;
        if let Some(name) = interpreter {
            tail.extend_from_slice(name.as_bytes());
            tail.push(0);
        }
        while !(strings + tail.len()).is_multiple_of(8) {
            tail.push(0);
        }
        let dynamic = strings + tail.len();
        let mut entries = Vec::new();
        for offset in &name_offsets[..needed.len()] {
            entries.push((1u64, *offset));
        }
        if search.is_some() {
            entries.push((29, name_offsets[needed.len()]));
        }
        entries.push((5, BASE + strings as u64));
        entries.push((10, strings_size as u64));
        entries.push((0, 0));
        let total = dynamic + 16 * entries.len();
        let mut bytes = vec![0u8; total];
        put(&mut bytes, 0, b"\x7fELF\x02\x01\x01");
        put(&mut bytes, 16, &3u16.to_le_bytes());
        put(&mut bytes, 20, &1u32.to_le_bytes());
        put(&mut bytes, 32, &(headers as u64).to_le_bytes());
        put(&mut bytes, 52, &64u16.to_le_bytes());
        put(&mut bytes, 54, &56u16.to_le_bytes());
        put(&mut bytes, 56, &(count as u16).to_le_bytes());
        let mut header = |index: usize, kind: u32, offset: usize, length: usize| {
            let at = headers + 56 * index;
            put(&mut bytes, at, &kind.to_le_bytes());
            put(&mut bytes, at + 8, &(offset as u64).to_le_bytes());
            put(&mut bytes, at + 16, &(BASE + offset as u64).to_le_bytes());
            put(&mut bytes, at + 32, &(length as u64).to_le_bytes());
            put(&mut bytes, at + 40, &(length as u64).to_le_bytes());
        };
        header(0, 1, 0, total);
        header(1, 2, dynamic, 16 * entries.len());
        if let Some(name) = interpreter {
            header(2, 3, interpreter_at, name.len() + 1);
        }
        put(&mut bytes, strings, &tail);
        for (index, (tag, value)) in entries.iter().enumerate() {
            put(&mut bytes, dynamic + 16 * index, &tag.to_le_bytes());
            put(&mut bytes, dynamic + 16 * index + 8, &value.to_le_bytes());
        }
        Made {
            bytes,
            headers,
            dynamic,
            strings_size,
            interpreter: interpreter.map(|_| interpreter_at),
        }
    }

    fn write(path: &Path, bytes: &[u8]) -> PathBuf {
        std::fs::write(path, bytes).unwrap();
        path.to_path_buf()
    }

    /// The interpreter the system's shell names, which lies in a system directory.
    fn system_interpreter() -> String {
        let loaded = closure(&shell(), &system()).unwrap();
        let interpreter = loaded.interpreters.iter().next().unwrap();
        interpreter.name.display().to_string()
    }

    /// A pinned directory and a directory outside every pin, each with an ELF library of its own.
    struct Places {
        _temp: tempfile::TempDir,
        pinned: PathBuf,
        outside: PathBuf,
    }

    fn places() -> Places {
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let (pinned, outside) = (base.join("pinned"), base.join("outside"));
        for directory in [&pinned, &outside] {
            std::fs::create_dir(directory).unwrap();
            write(&directory.join("libdummy.so"), &made(None, &[], None).bytes);
        }
        Places {
            _temp: temp,
            pinned,
            outside,
        }
    }

    /// Codex M1's positive control: a library in a pinned directory, found through `$ORIGIN`,
    /// and the C library in the system's, are both found and granted.
    #[test]
    fn a_dependency_in_a_pinned_or_a_system_directory_is_granted() {
        let places = places();
        let interpreter = system_interpreter();
        let tool = write(
            &places.pinned.join("tool"),
            &made(
                Some(&interpreter),
                &["libdummy.so", "libc.so.6"],
                Some("$ORIGIN"),
            )
            .bytes,
        );
        let roots = allowed_roots(&[places.pinned.as_path()]);
        let loaded = closure(&tool, &roots).unwrap();
        assert!(
            loaded
                .libraries
                .iter()
                .any(|library| library.real == places.pinned.join("libdummy.so")),
            "{loaded:?}"
        );
        assert!(
            loaded.libraries.iter().any(|library| library
                .name
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("libc.so"))),
            "{loaded:?}"
        );
        let staging = places.pinned.parent().unwrap().join("staging");
        std::fs::create_dir(&staging).unwrap();
        let read = [places.pinned.as_path()];
        let planned = plan(
            &Wanted {
                executable: &tool,
                read: &read,
                visible: &[],
            },
            &staging,
        )
        .unwrap();
        assert_eq!(planned.grants.get(&places.pinned), Some(&Grant::Read));
    }

    /// Codex M1: a library named by an absolute `DT_NEEDED`, or found through an RPATH, outside
    /// every pin and the system's directories, adds no grant and refuses the launch; so does an
    /// interpreter there, and a dependency that is not ELF.
    #[test]
    fn a_dependency_outside_the_pins_and_the_system_adds_no_grant_and_is_refused() {
        let places = places();
        let interpreter = system_interpreter();
        let outside_library = places.outside.join("libdummy.so");
        std::fs::write(places.pinned.join("libtext.so"), "not an object\n").unwrap();
        let outside_search = places.outside.display().to_string();
        let outside_interpreter = outside_library.display().to_string();
        let cases = [
            (
                "an absolute DT_NEEDED",
                made(
                    Some(&interpreter),
                    &[outside_library.to_str().unwrap()],
                    None,
                ),
            ),
            (
                "an RPATH",
                made(Some(&interpreter), &["libdummy.so"], Some(&outside_search)),
            ),
            (
                "an interpreter",
                made(Some(&outside_interpreter), &[], None),
            ),
            (
                "a dependency that is not ELF",
                made(Some(&interpreter), &["libtext.so"], Some("$ORIGIN")),
            ),
        ];
        let roots = allowed_roots(&[places.pinned.as_path()]);
        for (index, (label, object)) in cases.into_iter().enumerate() {
            let tool = write(&places.pinned.join(format!("tool-{index}")), &object.bytes);
            let refused = closure(&tool, &roots).unwrap_err();
            assert!(
                refused.contains("outside the system's library directories")
                    || refused.contains("is not an ELF object"),
                "{label}: {refused}"
            );
            let staging = places
                .pinned
                .parent()
                .unwrap()
                .join(format!("staging-{index}"));
            std::fs::create_dir(&staging).unwrap();
            let read = [places.pinned.as_path()];
            let planned = plan(
                &Wanted {
                    executable: &tool,
                    read: &read,
                    visible: &[],
                },
                &staging,
            );
            assert!(planned.is_err(), "{label}: {planned:?}");
        }
        // The control: the same library, pinned, is granted.
        let pinned_roots = allowed_roots(&[places.pinned.as_path(), places.outside.as_path()]);
        let tool = write(
            &places.pinned.join("tool-control"),
            &made(
                Some(&interpreter),
                &[outside_library.to_str().unwrap()],
                None,
            )
            .bytes,
        );
        let loaded = closure(&tool, &pinned_roots).unwrap();
        assert!(
            loaded
                .libraries
                .iter()
                .any(|library| library.real == outside_library),
            "{loaded:?}"
        );
    }

    fn mirror_at(root: &Path) -> super::Mirror {
        super::Mirror {
            root: root.to_path_buf(),
            ops: Vec::new(),
            directories: std::collections::BTreeSet::new(),
            links: std::collections::BTreeSet::new(),
            grants: std::collections::BTreeMap::new(),
        }
    }

    fn plan_for(tool: &Path, pinned: &Path, staging: &Path) -> Result<super::Plan, String> {
        std::fs::create_dir(staging).unwrap();
        let read = [pinned];
        plan(
            &Wanted {
                executable: tool,
                read: &read,
                visible: &[],
            },
            staging,
        )
    }

    fn bound_sources(planned: &super::Plan) -> Vec<PathBuf> {
        planned
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Bind { source, .. } => Some(PathBuf::from(std::ffi::OsStr::from_bytes(
                    source.to_bytes(),
                ))),
                _ => None,
            })
            .collect()
    }

    /// Fix round 2 (a): a link `P/s` to `A/sub` followed by `..`. The kernel, and the check,
    /// resolve `P/s/../libdummy.so` to `A/libdummy.so`; removing `..` by its name would give
    /// `P/libdummy.so`. The grant and the bind go to the checked `A/libdummy.so`, and nothing
    /// to `P/libdummy.so`. The other way round, where the real file lies outside, it is refused
    /// with no plan. And a name that leads to another file than the one checked is refused.
    #[test]
    fn a_link_and_dot_dot_grant_only_the_checked_file() {
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let (outside, pinned) = (base.join("P"), base.join("A"));
        for directory in [&outside, &pinned, &pinned.join("sub"), &outside.join("sub")] {
            std::fs::create_dir(directory).unwrap();
        }
        for directory in [&outside, &pinned] {
            write(&directory.join("libdummy.so"), &made(None, &[], None).bytes);
        }
        std::os::unix::fs::symlink(pinned.join("sub"), outside.join("s")).unwrap();
        std::os::unix::fs::symlink(outside.join("sub"), pinned.join("s")).unwrap();
        let interpreter = system_interpreter();
        let roots = allowed_roots(&[pinned.as_path()]);

        // Through the outside link into the pinned directory: the checked file is granted.
        let name = outside.join("s/../libdummy.so");
        let tool = write(
            &pinned.join("tool-in"),
            &made(Some(&interpreter), &[name.to_str().unwrap()], None).bytes,
        );
        let loaded = closure(&tool, &roots).unwrap();
        let found = loaded
            .libraries
            .iter()
            .find(|library| library.name == name)
            .unwrap_or_else(|| panic!("{loaded:?}"));
        assert_eq!(found.real, pinned.join("libdummy.so"));
        let planned = plan_for(&tool, &pinned, &base.join("staging-in")).unwrap();
        assert_eq!(planned.grants.get(&outside.join("libdummy.so")), None);
        assert!(
            !bound_sources(&planned).contains(&outside.join("libdummy.so")),
            "{:?}",
            planned.ops
        );
        assert!(
            planned
                .grants
                .keys()
                .all(|path| !path.starts_with(&outside)),
            "{:?}",
            planned.grants
        );

        // Through the pinned link out to the other directory: the real file is outside.
        let name = pinned.join("s/../libdummy.so");
        let tool = write(
            &pinned.join("tool-out"),
            &made(Some(&interpreter), &[name.to_str().unwrap()], None).bytes,
        );
        let refused = closure(&tool, &roots).unwrap_err();
        assert!(
            refused.contains("outside the system's library directories"),
            "{refused}"
        );
        assert!(plan_for(&tool, &pinned, &base.join("staging-out")).is_err());

        // A name made again inside must lead to the file that was checked.
        let mut mirror = mirror_at(&base.join("staging-name"));
        let refused = mirror
            .name(
                &outside.join("s/../libdummy.so"),
                &outside.join("libdummy.so"),
            )
            .unwrap_err();
        assert!(refused.contains("not to the checked"), "{refused}");
        assert!(mirror.grants.is_empty());
        mirror
            .name(
                &outside.join("s/../libdummy.so"),
                &pinned.join("libdummy.so"),
            )
            .unwrap();
        assert!(mirror.grants.is_empty(), "a name gets no grant");
    }

    /// Fix round 2 (b): an interpreter inside a pinned directory that is not an ELF object, or is
    /// a broken one, is refused before any grant.
    #[test]
    fn an_interpreter_that_is_not_a_sound_elf_object_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let pinned = std::fs::canonicalize(temp.path()).unwrap().join("A");
        std::fs::create_dir(&pinned).unwrap();
        let text = write(&pinned.join("ld-text.so"), b"#!/bin/sh\nexit 0\n");
        let mut cut = b"\x7fELF\x02\x01\x01".to_vec();
        cut.resize(40, 0);
        let broken = write(&pinned.join("ld-broken.so"), &cut);
        let roots = allowed_roots(&[pinned.as_path()]);
        for (index, interpreter) in [text, broken].iter().enumerate() {
            let tool = write(
                &pinned.join(format!("tool-{index}")),
                &made(Some(interpreter.to_str().unwrap()), &[], None).bytes,
            );
            let refused = closure(&tool, &roots).unwrap_err();
            assert!(
                refused.contains("not an ELF object") || refused.contains("a cut header"),
                "{refused}"
            );
            let staging = pinned.parent().unwrap().join(format!("staging-{index}"));
            assert!(plan_for(&tool, &pinned, &staging).is_err());
        }
    }

    /// Fix round 2 (c): the usual forms still pass. The system's interpreter, named through its
    /// usual link (`/lib/ld-linux-aarch64.so.1` or `/lib64/ld-linux-x86-64.so.2`), is granted
    /// at its real file and its name is made again inside; a library found through
    /// `$ORIGIN/../lib` in a pinned directory is granted at its real file.
    #[test]
    fn the_usual_linked_and_dot_dot_forms_still_pass() {
        let shell = shell();
        let loaded = closure(&shell, &system()).unwrap();
        let interpreter = loaded.interpreters.iter().next().unwrap();
        assert_eq!(
            interpreter.real,
            std::fs::canonicalize(&interpreter.name).unwrap()
        );
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let pinned = base.join("tool");
        for directory in [pinned.join("bin"), pinned.join("lib")] {
            std::fs::create_dir_all(directory).unwrap();
        }
        write(
            &pinned.join("lib/libdummy.so"),
            &made(None, &[], None).bytes,
        );
        let tool = write(
            &pinned.join("bin/tool"),
            &made(
                Some(&interpreter.name.display().to_string()),
                &["libdummy.so", "libc.so.6"],
                Some("$ORIGIN/../lib"),
            )
            .bytes,
        );
        let planned = plan_for(&tool, &pinned, &base.join("staging")).unwrap();
        assert_eq!(planned.grants.get(&interpreter.real), Some(&Grant::Run));
        if interpreter.name != interpreter.real {
            assert_eq!(planned.grants.get(&interpreter.name), None);
            assert!(
                planned.ops.iter().any(|op| matches!(op, Op::Link { .. })),
                "{:?}",
                planned.ops
            );
        }
        let loaded = closure(&tool, &allowed_roots(&[pinned.as_path()])).unwrap();
        assert!(
            loaded.libraries.iter().any(|library| {
                library.name == pinned.join("bin/../lib/libdummy.so")
                    && library.real == pinned.join("lib/libdummy.so")
            }),
            "{loaded:?}"
        );
        assert_eq!(planned.grants.get(&pinned), Some(&Grant::Read));
    }

    /// Fix round 3 (codex R2-01): `a/libwrap.so` and `b/libwrap.so` are two names of the same
    /// `common/libwrap.so`, whose `RUNPATH` is `$ORIGIN/private`, so each name needs its own
    /// `private/libleaf.so`. The tool names `a`, then `b`. Both leaves are found, granted, and
    /// bound, and both names are made again inside. The allowed root is a directory of this
    /// test standing in for a system one; the real system directories are not touched.
    #[test]
    fn one_file_reached_under_two_names_searches_each_names_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let system = base.join("system");
        for directory in ["common", "a/private", "b/private"] {
            std::fs::create_dir_all(system.join(directory)).unwrap();
        }
        write(
            &system.join("common/libwrap.so"),
            &made(None, &["libleaf.so"], Some("$ORIGIN/private")).bytes,
        );
        for name in ["a", "b"] {
            std::os::unix::fs::symlink(
                "../common/libwrap.so",
                system.join(name).join("libwrap.so"),
            )
            .unwrap();
            write(
                &system.join(name).join("private/libleaf.so"),
                &made(None, &[], None).bytes,
            );
        }
        let tool_directory = base.join("tool");
        std::fs::create_dir(&tool_directory).unwrap();
        let (a, b) = (system.join("a/libwrap.so"), system.join("b/libwrap.so"));
        let tool = write(
            &tool_directory.join("tool"),
            &made(None, &[a.to_str().unwrap(), b.to_str().unwrap()], None).bytes,
        );
        let roots = vec![system.clone()];
        let leaves = [
            system.join("a/private/libleaf.so"),
            system.join("b/private/libleaf.so"),
        ];

        let loaded = closure(&tool, &roots).unwrap();
        for leaf in &leaves {
            assert!(
                loaded.libraries.iter().any(|library| library.real == *leaf),
                "{} was not searched: {loaded:?}",
                leaf.display()
            );
        }
        let common = system.join("common/libwrap.so");
        for name in [&a, &b] {
            assert!(
                loaded
                    .libraries
                    .iter()
                    .any(|library| library.name == *name && library.real == common),
                "{loaded:?}"
            );
        }

        let staging = base.join("staging");
        std::fs::create_dir(&staging).unwrap();
        let planned = super::plan_within(
            &Wanted {
                executable: &tool,
                read: &[],
                visible: &[],
            },
            &staging,
            &roots,
        )
        .unwrap();
        let bound = bound_sources(&planned);
        for leaf in leaves.iter().chain([&common]) {
            assert_eq!(planned.grants.get(leaf), Some(&Grant::Read), "{leaf:?}");
            assert!(bound.contains(leaf), "{leaf:?} is not bound: {bound:?}");
        }
        for name in [&a, &b] {
            let link = staging.join(name.strip_prefix("/").unwrap());
            assert!(
                planned.ops.iter().any(|op| matches!(
                    op,
                    Op::Link { path, .. } if path.to_bytes() == link.as_os_str().as_bytes()
                )),
                "no link for {}: {:?}",
                name.display(),
                planned.ops
            );
        }
    }

    /// Codex M2: a cut, a huge value, or an overflow anywhere in an object is a refusal, never a
    /// panic and never a read past the file.
    #[test]
    fn a_malformed_object_is_refused_without_a_panic() {
        let temp = tempfile::tempdir().unwrap();
        let interpreter = system_interpreter();
        let good = made(Some(&interpreter), &["libc.so.6"], Some("$ORIGIN"));
        let path = temp.path().join("object");
        write(&path, &good.bytes);
        let parsed = object(&path).unwrap().unwrap();
        assert_eq!(parsed.needed, vec!["libc.so.6".to_string()]);
        assert_eq!(parsed.search, vec!["$ORIGIN".to_string()]);
        let judged = |bytes: &[u8]| {
            write(&path, bytes);
            let path = path.clone();
            std::panic::catch_unwind(move || object(&path).map(|object| object.is_some()))
                .unwrap_or_else(|_| panic!("a panic on {} bytes", bytes.len()))
        };
        for length in 0..good.bytes.len() {
            let answer = judged(&good.bytes[..length]);
            if length < 4 {
                assert_eq!(answer, Ok(false), "cut at {length}");
            } else {
                assert!(answer.is_err(), "cut at {length}: {answer:?}");
            }
        }
        let (load, dynamic) = (good.headers, good.headers + 56);
        let interpreter_header = good.headers + 112;
        let huge = u64::MAX.to_le_bytes();
        let almost = (u64::MAX - 7).to_le_bytes();
        let mut damages: Vec<(&str, usize, Vec<u8>)> = vec![
            ("a 32-bit class", 4, vec![1]),
            ("big endian", 5, vec![2]),
            ("another version", 6, vec![0]),
            ("a header size of 65", 52, 65u16.to_le_bytes().to_vec()),
            (
                "a program header size of 64",
                54,
                64u16.to_le_bytes().to_vec(),
            ),
            ("65535 program headers", 56, 65535u16.to_le_bytes().to_vec()),
            ("program headers past the end", 32, huge.to_vec()),
            (
                "program headers at the end",
                32,
                (good.bytes.len() as u64).to_le_bytes().to_vec(),
            ),
            ("a load at the last offset", load + 8, huge.to_vec()),
            ("a load as long as it gets", load + 32, huge.to_vec()),
            ("a load at the last address", load + 16, huge.to_vec()),
            (
                "a dynamic section that overflows",
                dynamic + 8,
                almost.to_vec(),
            ),
            (
                "a dynamic section too long",
                dynamic + 32,
                (1u64 << 40).to_le_bytes().to_vec(),
            ),
            (
                "an interpreter that overflows",
                interpreter_header + 8,
                almost.to_vec(),
            ),
            (
                "an interpreter with no room",
                interpreter_header + 32,
                0u64.to_le_bytes().to_vec(),
            ),
            (
                "an interpreter too long",
                interpreter_header + 32,
                8192u64.to_le_bytes().to_vec(),
            ),
            // The dynamic entries: DT_NEEDED, DT_RUNPATH, DT_STRTAB, DT_STRSZ, DT_NULL.
            (
                "a needed name past its table",
                good.dynamic + 8,
                huge.to_vec(),
            ),
            (
                "a search name past its table",
                good.dynamic + 24,
                huge.to_vec(),
            ),
            (
                "a string table at the last address",
                good.dynamic + 40,
                huge.to_vec(),
            ),
            (
                "a string table with no size",
                good.dynamic + 48,
                0x7fffu64.to_le_bytes().to_vec(),
            ),
            (
                "a string table as long as it gets",
                good.dynamic + 56,
                huge.to_vec(),
            ),
            (
                "a string table past its segment",
                good.dynamic + 56,
                (1u64 << 20).to_le_bytes().to_vec(),
            ),
            (
                "a string table cut before its last NUL",
                good.dynamic + 56,
                ((good.strings_size - 1) as u64).to_le_bytes().to_vec(),
            ),
        ];
        if let Some(at) = good.interpreter {
            damages.push((
                "an interpreter with no NUL",
                at + interpreter.len(),
                b"x".to_vec(),
            ));
        }
        for (label, at, value) in damages {
            let mut bytes = good.bytes.clone();
            bytes[at..at + value.len()].copy_from_slice(&value);
            assert!(judged(&bytes).is_err(), "{label}");
        }
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
