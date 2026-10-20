//! The dependency-cache write allowance, in one place (Issue #772, a child of
//! #697).
//!
//! Both OS backends receive the same resolved answer from here: which known
//! caches are writable, per operating system, and why a candidate was refused.
//! A candidate is allowed only when it exists, has the expected kind, resolves
//! (all symlinks followed) to a strict descendant of the resolved home, and
//! overlaps no home secret location in either direction. The home itself, a
//! secret, a secret's child, and a secret's ancestor (`~/.config` for
//! `~/.config/gh`) are therefore never writable, even when a cache name such as
//! `~/.npm` is a symlink to one of them.
//!
//! The secret list is only consulted, never copied:
//! [`secret_locations::home_secret_locations`] stays the single source.
//!
//! The decision is taken when the sandbox is installed. The resolved path is what
//! the backends receive, which narrows the gap between check and use, but a
//! process outside the sandbox that renames or relinks a directory afterwards is
//! not prevented (see `SECURITY.md`).

use std::path::{Path, PathBuf};

use super::secret_locations;

/// The platform whose cache list applies. Tests pass both to prove the lists do
/// not mix; production uses [`current_os`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheOs {
    Linux,
    MacOs,
}

/// The OS this build runs on.
pub const fn current_os() -> CacheOs {
    if cfg!(target_os = "macos") {
        CacheOs::MacOs
    } else {
        CacheOs::Linux
    }
}

/// Whether a cache location is a directory or a single file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheKind {
    Dir,
    File,
}

/// Which `CachePaths` field a location fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheSlot {
    CargoRegistry,
    CargoGit,
    CargoPackageCache,
    Npm,
    Pip,
    Playwright,
}

/// One known cache location, relative to the home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheEntry {
    pub slot: CacheSlot,
    pub relative: &'static str,
    pub kind: CacheKind,
}

const fn entry(slot: CacheSlot, relative: &'static str, kind: CacheKind) -> CacheEntry {
    CacheEntry {
        slot,
        relative,
        kind,
    }
}

/// Cargo and npm caches, shared by both operating systems.
const COMMON: &[CacheEntry] = &[
    entry(CacheSlot::CargoRegistry, ".cargo/registry", CacheKind::Dir),
    entry(CacheSlot::CargoGit, ".cargo/git", CacheKind::Dir),
    entry(
        CacheSlot::CargoPackageCache,
        ".cargo/.package-cache",
        CacheKind::File,
    ),
    entry(CacheSlot::Npm, ".npm", CacheKind::Dir),
];

const MACOS_ONLY: &[CacheEntry] = &[
    entry(CacheSlot::Pip, "Library/Caches/pip", CacheKind::Dir),
    entry(
        CacheSlot::Playwright,
        "Library/Caches/ms-playwright",
        CacheKind::Dir,
    ),
];

const LINUX_ONLY: &[CacheEntry] = &[
    entry(CacheSlot::Pip, ".cache/pip", CacheKind::Dir),
    entry(
        CacheSlot::Playwright,
        ".cache/ms-playwright",
        CacheKind::Dir,
    ),
];

/// The known caches for `os`: the shared list plus that OS's own locations.
pub fn entries(os: CacheOs) -> Vec<CacheEntry> {
    let own = match os {
        CacheOs::Linux => LINUX_ONLY,
        CacheOs::MacOs => MACOS_ONLY,
    };
    COMMON.iter().chain(own).copied().collect()
}

/// Why a candidate that is present was not made writable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheRefusal {
    /// A symlink whose target is missing or cannot be resolved.
    Dangling,
    /// Resolves outside the home.
    OutsideHome,
    /// Resolves to the home directory itself.
    HomeItself,
    /// Resolves to a home secret, a child of one, or an ancestor of one.
    CredentialOverlap,
    /// Resolves to a file where a directory is expected, or the reverse.
    WrongKind,
}

/// The decision for one candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheVerdict {
    /// Nothing exists at the candidate path; there is nothing to allow or refuse.
    Absent,
    /// Writable; carries the resolved (canonical) path the backends receive.
    Allowed(PathBuf),
    Refused(CacheRefusal),
}

/// Decide one candidate `home/relative` against explicit `secrets` (canonical
/// absolute paths). An empty home allows nothing.
pub fn judge(home: &Path, relative: &str, kind: CacheKind, secrets: &[PathBuf]) -> CacheVerdict {
    if home.as_os_str().is_empty() {
        return CacheVerdict::Absent;
    }
    let candidate = home.join(relative);
    let Ok(link_meta) = std::fs::symlink_metadata(&candidate) else {
        return CacheVerdict::Absent;
    };
    let Ok(home) = home.canonicalize() else {
        return CacheVerdict::Absent;
    };
    let resolved = match candidate.canonicalize() {
        Ok(resolved) => resolved,
        Err(_) if link_meta.file_type().is_symlink() => {
            return CacheVerdict::Refused(CacheRefusal::Dangling);
        }
        Err(_) => return CacheVerdict::Absent,
    };
    if resolved == home {
        return CacheVerdict::Refused(CacheRefusal::HomeItself);
    }
    if !resolved.starts_with(&home) {
        return CacheVerdict::Refused(CacheRefusal::OutsideHome);
    }
    if secrets
        .iter()
        .any(|secret| resolved.starts_with(secret) || secret.starts_with(&resolved))
    {
        return CacheVerdict::Refused(CacheRefusal::CredentialOverlap);
    }
    let right_kind = match kind {
        CacheKind::Dir => resolved.is_dir(),
        CacheKind::File => resolved.is_file(),
    };
    if right_kind {
        CacheVerdict::Allowed(resolved)
    } else {
        CacheVerdict::Refused(CacheRefusal::WrongKind)
    }
}

/// Every known cache of `os` that exists under `home`, with its decision.
/// Absent candidates are left out.
pub fn resolve(home: &Path, os: CacheOs) -> Vec<(CacheSlot, CacheVerdict)> {
    let secrets = secret_locations::home_secret_locations(home);
    entries(os)
        .into_iter()
        .map(|cache| {
            (
                cache.slot,
                judge(home, cache.relative, cache.kind, &secrets),
            )
        })
        .filter(|(_, verdict)| *verdict != CacheVerdict::Absent)
        .collect()
}

/// The reason clause for the refused caches, or `None` when none were refused.
pub fn refusal_note(decisions: &[(CacheSlot, CacheVerdict)]) -> Option<String> {
    const LABELS: &[(CacheRefusal, &str)] = &[
        (CacheRefusal::OutsideHome, "resolve outside the home"),
        (CacheRefusal::HomeItself, "resolve to the home itself"),
        (
            CacheRefusal::CredentialOverlap,
            "overlap a protected credential location",
        ),
        (CacheRefusal::WrongKind, "are not the expected file type"),
        (CacheRefusal::Dangling, "are dangling links"),
    ];
    let refusals: Vec<CacheRefusal> = decisions
        .iter()
        .filter_map(|(_, verdict)| match verdict {
            CacheVerdict::Refused(reason) => Some(*reason),
            _ => None,
        })
        .collect();
    if refusals.is_empty() {
        return None;
    }
    let parts: Vec<String> = LABELS
        .iter()
        .filter_map(|(reason, label)| {
            let count = refusals.iter().filter(|found| *found == reason).count();
            (count > 0).then(|| format!("{count} {label}"))
        })
        .collect();
    Some(format!(
        "{} dependency cache location(s) are not writable ({})",
        refusals.len(),
        parts.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    use super::{
        CacheKind, CacheOs, CacheRefusal, CacheVerdict, entries, judge, refusal_note, resolve,
    };
    use crate::tools::os_sandbox::secret_locations::home_secret_locations_with;

    struct Home {
        _dir: tempfile::TempDir,
        base: PathBuf,
        home: PathBuf,
    }

    fn home() -> Home {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let home = base.join("home");
        std::fs::create_dir_all(&home).unwrap();
        Home {
            _dir: dir,
            base,
            home,
        }
    }

    fn secrets(home: &std::path::Path) -> Vec<PathBuf> {
        home_secret_locations_with(home, None)
    }

    fn verdict(h: &Home, relative: &str, kind: CacheKind) -> CacheVerdict {
        judge(&h.home, relative, kind, &secrets(&h.home))
    }

    #[test]
    fn the_lists_are_per_os_and_keep_cargo_and_npm_on_both() {
        let linux: Vec<_> = entries(CacheOs::Linux).iter().map(|e| e.relative).collect();
        let macos: Vec<_> = entries(CacheOs::MacOs).iter().map(|e| e.relative).collect();
        for shared in [
            ".cargo/registry",
            ".cargo/git",
            ".cargo/.package-cache",
            ".npm",
        ] {
            assert!(
                linux.contains(&shared) && macos.contains(&shared),
                "{shared}"
            );
        }
        assert!(linux.contains(&".cache/pip") && linux.contains(&".cache/ms-playwright"));
        assert!(macos.contains(&"Library/Caches/pip"));
        assert!(macos.contains(&"Library/Caches/ms-playwright"));
        assert!(
            !linux.iter().any(|path| path.starts_with("Library/")),
            "{linux:?}"
        );
        assert!(
            !macos.iter().any(|path| path.starts_with(".cache/")),
            "{macos:?}"
        );
    }

    #[test]
    fn a_real_cache_inside_the_home_is_allowed_by_its_resolved_path() {
        let h = home();
        std::fs::create_dir_all(h.home.join(".npm")).unwrap();
        assert_eq!(
            verdict(&h, ".npm", CacheKind::Dir),
            CacheVerdict::Allowed(h.home.join(".npm"))
        );
        std::fs::create_dir_all(h.home.join("store")).unwrap();
        std::fs::create_dir_all(h.home.join(".cargo")).unwrap();
        symlink(h.home.join("store"), h.home.join(".cargo/git")).unwrap();
        assert_eq!(
            verdict(&h, ".cargo/git", CacheKind::Dir),
            CacheVerdict::Allowed(h.home.join("store"))
        );
    }

    #[test]
    fn absent_and_empty_home_allow_nothing_and_refuse_nothing() {
        let h = home();
        assert_eq!(verdict(&h, ".npm", CacheKind::Dir), CacheVerdict::Absent);
        assert_eq!(
            judge(std::path::Path::new(""), ".npm", CacheKind::Dir, &[]),
            CacheVerdict::Absent
        );
    }

    #[test]
    fn the_home_itself_is_refused() {
        let h = home();
        symlink(&h.home, h.home.join(".npm")).unwrap();
        assert_eq!(
            verdict(&h, ".npm", CacheKind::Dir),
            CacheVerdict::Refused(CacheRefusal::HomeItself)
        );
    }

    #[test]
    fn a_secret_its_child_and_its_ancestor_are_refused() {
        let h = home();
        for dir in [".ssh/keys", ".config/gh/hosts", ".config/other", ".aws"] {
            std::fs::create_dir_all(h.home.join(dir)).unwrap();
        }
        for target in [
            ".ssh",
            ".ssh/keys",
            ".aws",
            ".config",
            ".config/gh",
            ".config/gh/hosts",
        ] {
            let link = h.home.join(".npm");
            let _ = std::fs::remove_file(&link);
            symlink(h.home.join(target), &link).unwrap();
            assert_eq!(
                verdict(&h, ".npm", CacheKind::Dir),
                CacheVerdict::Refused(CacheRefusal::CredentialOverlap),
                "{target}"
            );
        }
        // A sibling of a secret under a shared ancestor is not a secret.
        let link = h.home.join(".npm");
        std::fs::remove_file(&link).unwrap();
        symlink(h.home.join(".config/other"), &link).unwrap();
        assert!(matches!(
            verdict(&h, ".npm", CacheKind::Dir),
            CacheVerdict::Allowed(_)
        ));
    }

    #[test]
    fn a_secret_reached_through_a_nested_link_is_refused() {
        let h = home();
        std::fs::create_dir_all(h.home.join(".ssh")).unwrap();
        symlink(h.home.join(".ssh"), h.home.join("hop1")).unwrap();
        symlink(h.home.join("hop1"), h.home.join("hop2")).unwrap();
        symlink(h.home.join("hop2"), h.home.join(".npm")).unwrap();
        assert_eq!(
            verdict(&h, ".npm", CacheKind::Dir),
            CacheVerdict::Refused(CacheRefusal::CredentialOverlap)
        );
    }

    #[test]
    fn outside_dangling_and_wrong_kind_are_refused() {
        let h = home();
        let outside = h.base.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, h.home.join(".npm")).unwrap();
        assert_eq!(
            verdict(&h, ".npm", CacheKind::Dir),
            CacheVerdict::Refused(CacheRefusal::OutsideHome)
        );

        std::fs::create_dir_all(h.home.join(".cargo")).unwrap();
        symlink(h.home.join("missing"), h.home.join(".cargo/git")).unwrap();
        assert_eq!(
            verdict(&h, ".cargo/git", CacheKind::Dir),
            CacheVerdict::Refused(CacheRefusal::Dangling)
        );

        std::fs::write(h.home.join(".cargo/registry"), "x").unwrap();
        assert_eq!(
            verdict(&h, ".cargo/registry", CacheKind::Dir),
            CacheVerdict::Refused(CacheRefusal::WrongKind)
        );
        std::fs::create_dir_all(h.home.join(".cargo/.package-cache")).unwrap();
        assert_eq!(
            verdict(&h, ".cargo/.package-cache", CacheKind::File),
            CacheVerdict::Refused(CacheRefusal::WrongKind)
        );
    }

    #[test]
    fn a_file_that_is_a_secret_cache_target_is_refused_not_dropped() {
        let h = home();
        std::fs::create_dir_all(h.home.join(".cargo")).unwrap();
        std::fs::write(h.home.join(".cargo/credentials.toml"), "t").unwrap();
        symlink(
            h.home.join(".cargo/credentials.toml"),
            h.home.join(".cargo/.package-cache"),
        )
        .unwrap();
        assert_eq!(
            verdict(&h, ".cargo/.package-cache", CacheKind::File),
            CacheVerdict::Refused(CacheRefusal::CredentialOverlap)
        );
    }

    #[test]
    fn resolve_applies_only_the_requested_os_list() {
        let h = home();
        for dir in [".cache/pip", "Library/Caches/pip"] {
            std::fs::create_dir_all(h.home.join(dir)).unwrap();
        }
        let linux = resolve(&h.home, CacheOs::Linux);
        let macos = resolve(&h.home, CacheOs::MacOs);
        let pip = |decisions: &[(super::CacheSlot, CacheVerdict)]| {
            decisions
                .iter()
                .find(|(slot, _)| *slot == super::CacheSlot::Pip)
                .map(|(_, verdict)| verdict.clone())
        };
        let allowed = |relative: &str| {
            Some(CacheVerdict::Allowed(
                h.home.canonicalize().unwrap().join(relative),
            ))
        };
        assert_eq!(pip(&linux), allowed(".cache/pip"));
        assert_eq!(pip(&macos), allowed("Library/Caches/pip"));
    }

    #[test]
    fn the_note_names_each_reason_and_stays_silent_without_refusals() {
        let h = home();
        let outside = h.base.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(h.home.join(".ssh")).unwrap();
        symlink(&outside, h.home.join(".npm")).unwrap();
        std::fs::create_dir_all(h.home.join(".cargo")).unwrap();
        symlink(h.home.join(".ssh"), h.home.join(".cargo/git")).unwrap();
        let note = refusal_note(&resolve(&h.home, CacheOs::Linux)).expect("refusals");
        assert!(note.starts_with("2 dependency cache location(s)"), "{note}");
        assert!(note.contains("1 resolve outside the home"), "{note}");
        assert!(note.contains("1 overlap a protected credential"), "{note}");
        assert_eq!(refusal_note(&[]), None);
    }
}
