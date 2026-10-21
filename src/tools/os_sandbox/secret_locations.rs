//! The single read-deny list shared by the macOS and Linux read backends
//! (Issue #657, stage 2 of #502).
//!
//! The credential-name rule is aligned with the #501 leaf
//! (`src/tools/sensitive_path.rs`): the workspace basenames and path sequences
//! below mirror `sensitive_name_rule` and `path_rule`, and the three strict
//! dotenv templates stay readable. The home list adds the places that only exist
//! outside a workspace: `~/.gnupg`, the `gh`/`gcloud` configuration, and the
//! kube config. Both backends import this module so the deny list cannot drift
//! between platforms.
//!
//! Positions travel to the backends as locations only: the Seatbelt profile
//! receives each home location as a `-D` parameter and the workspace rules as
//! generated regexes; the Landlock backend expands the same location list into a
//! read-allow list. Every path is canonicalized first, so a symlinked secret
//! location denies its target and a workspace under a symlinked root still
//! matches. No secret bytes and no credential value ever appear here.

use std::path::{Component, Path, PathBuf};

/// Home-relative secret locations whose reads are denied (Issue #657). Aligned
/// with #501's credential rule (`sensitive_name_rule` and `path_rule`) plus the
/// home-specific places called out in the design: `.gnupg`, the `gh`/`gcloud`
/// configuration, and `.kube`.
///
/// `.docker/config.json` is a file; the rest are directories.
const HOME_SECRET_LOCATIONS: &[&str] = &[
    ".ssh",
    ".aws",
    ".gnupg",
    ".netrc",
    ".git-credentials",
    ".npmrc",
    ".pypirc",
    ".docker/config.json",
    ".kube",
    ".config/gh",
    ".config/gcloud",
    ".cargo/credentials.toml",
    ".cargo/credentials",
];

/// The prefix that marks a [`HOME_SECRET_LOCATIONS`] entry as a file of the
/// cargo home. The same entries are re-rooted under `$CARGO_HOME` (Issue #695),
/// so the cargo credential names live in the list once.
const CARGO_HOME_PREFIX: &str = ".cargo/";

/// The home secret locations as canonical absolute paths under `home`, plus the
/// cargo credential files under the `CARGO_HOME` of this process (Issue #695).
/// Returns an empty list when `home` is empty, so a run without `HOME` denies
/// nothing rather than resolving names relative to the process working
/// directory. Each location is canonicalized, so a symlinked location (`~/.ssh`
/// pointing outside the home) denies its target as well.
///
/// The child inherits `CARGO_HOME` from this process only, so reading it here
/// denies the credentials of the cargo home the child will use, whether it sits
/// inside the home or outside it (for example `/usr/local/cargo`).
pub fn home_secret_locations(home: &Path) -> Vec<PathBuf> {
    let cargo_home = std::env::var_os("CARGO_HOME").map(PathBuf::from);
    home_secret_locations_with(home, cargo_home.as_deref())
}

/// [`home_secret_locations`] with an explicit `CARGO_HOME`. A relative or empty
/// `CARGO_HOME` is ignored: it would resolve against the child's working
/// directory, not a fixed location.
pub fn home_secret_locations_with(home: &Path, cargo_home: Option<&Path>) -> Vec<PathBuf> {
    if home.as_os_str().is_empty() {
        return Vec::new();
    }
    let lexical_home = home;
    let home = resolve_nearest_existing(home);
    let mut locations: Vec<PathBuf> = Vec::new();
    for relative in HOME_SECRET_LOCATIONS {
        push_unique(
            &mut locations,
            resolve_nearest_existing(&home.join(relative)),
        );
        push_unique(&mut locations, lexical_home.join(relative));
    }
    if let Some(cargo_home) = cargo_home.filter(|path| path.is_absolute()) {
        let resolved_cargo_home = resolve_nearest_existing(cargo_home);
        for relative in HOME_SECRET_LOCATIONS {
            if let Some(name) = relative.strip_prefix(CARGO_HOME_PREFIX) {
                push_unique(
                    &mut locations,
                    resolve_nearest_existing(&resolved_cargo_home.join(name)),
                );
                push_unique(&mut locations, cargo_home.join(name));
            }
        }
    }
    locations
}

fn push_unique(locations: &mut Vec<PathBuf>, location: PathBuf) {
    if !locations.contains(&location) {
        locations.push(location);
    }
}

/// The most dangling links one resolution follows before it stops and keeps the
/// rest as written, so a link cycle ends.
const MAX_DANGLING_LINKS: usize = 40;

/// Resolve a path through every symlink on it. A path that does not exist yet (a
/// secret a process creates during the run) keeps its missing tail: the existing
/// part is canonicalized and the remaining names are appended to that real path,
/// so the denial follows a symlinked ancestor to its target (Issue #788). An
/// ancestor that is a dangling link is followed by `read_link` (a relative
/// target is taken from the link's directory), at most [`MAX_DANGLING_LINKS`]
/// times; past that, or when the target cannot be read, the rest stays as
/// written. Callers that must also deny the spelling as written keep the original
/// path beside this one. The result is a snapshot taken when the sandbox is
/// installed; a relink by a process outside the sandbox afterwards is not
/// prevented.
pub(super) fn resolve_nearest_existing(path: &Path) -> PathBuf {
    let mut budget = MAX_DANGLING_LINKS;
    resolve_with_budget(path, &mut budget)
}

fn resolve_with_budget(path: &Path, budget: &mut usize) -> PathBuf {
    if let Ok(resolved) = path.canonicalize() {
        return resolved;
    }
    if !path.is_absolute() {
        return path.to_path_buf();
    }
    let components: Vec<Component> = path.components().collect();
    let mut resolved = PathBuf::from("/");
    for (index, component) in components.iter().enumerate() {
        let name = match component {
            Component::Normal(name) => name,
            Component::ParentDir => {
                resolved.pop();
                continue;
            }
            _ => continue,
        };
        let candidate = resolved.join(name);
        let is_link = std::fs::symlink_metadata(&candidate)
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false);
        if let Ok(real) = candidate.canonicalize() {
            resolved = real;
        } else if is_link {
            // A dangling or cyclic link: follow it by hand within the budget.
            let target = std::fs::read_link(&candidate).ok();
            match target {
                Some(target) if *budget > 0 => {
                    *budget -= 1;
                    let target = if target.is_absolute() {
                        target
                    } else {
                        resolved.join(target)
                    };
                    resolved = resolve_with_budget(&target, budget);
                }
                _ => resolved = candidate,
            }
        } else {
            // Nothing exists here, so nothing below can be a link.
            resolved = candidate;
        }
        if index + 1 < components.len() && !resolved.exists() && !is_link {
            resolved.extend(
                components[index + 1..]
                    .iter()
                    .filter_map(|rest| match rest {
                        Component::Normal(name) => Some(*name),
                        _ => None,
                    }),
            );
            return resolved;
        }
    }
    resolved
}

/// The workspace credential basenames denied for reading, mirroring #501's
/// `sensitive_name_rule` (Issue #657). `.env.*` matches every variant; the three
/// strict templates are re-allowed by [`workspace_template_regex`], which the
/// Seatbelt profile applies after this rule. The whole rule is case-insensitive,
/// so `.ENV`, `.Env.local`, and `SERVICE-ACCOUNT.json` are refused on a
/// case-insensitive filesystem, matching #501's lowercased comparison.
#[cfg(target_os = "macos")]
const WORKSPACE_CREDENTIAL_BASENAMES: &[&str] = &[
    r"\.env",
    r"\.env\.[^/]*",
    r"\.envrc",
    r"\.npmrc",
    r"\.pypirc",
    r"\.netrc",
    r"\.git-credentials",
    r"\.ssh",
    r"service-account[^/]*\.json",
    r"[^/]*\.private\.key",
    r"[^/]*\.private\.pem",
    r"[^/]*\.p12",
    r"[^/]*\.pfx",
];

/// The workspace credential path sequences denied for reading, mirroring #501's
/// `path_rule` (Issue #657): the credential is a fixed parent/child pair.
#[cfg(target_os = "macos")]
const WORKSPACE_CREDENTIAL_PATH_SEQUENCES: &[&str] = &[
    r"\.aws/credentials",
    r"\.docker/config\.json",
    r"\.config/gcloud/application_default_credentials\.json",
    r"\.cargo/credentials\.toml",
    r"\.cargo/credentials",
];

/// The strict dotenv template basenames re-allowed as files.
#[cfg(target_os = "macos")]
const WORKSPACE_TEMPLATE_PATTERN: &str = r"\.env\.(example|sample|template)";

/// A Seatbelt regex that matches a credential file anywhere under `workspace`,
/// including every file beneath a credential-named directory. The root is
/// canonicalized (Seatbelt matches the resolved path), the rule is
/// case-insensitive, and the trailing `(/.*)?$` denies a secret directory's
/// contents (`.ssh/id_rsa`, `.env.d/x`), matching #501's parent rule.
#[cfg(target_os = "macos")]
pub fn workspace_credential_regex(workspace: &Path) -> String {
    let names = case_fold(
        &WORKSPACE_CREDENTIAL_BASENAMES
            .iter()
            .chain(WORKSPACE_CREDENTIAL_PATH_SEQUENCES)
            .copied()
            .collect::<Vec<_>>()
            .join("|"),
    );
    format!("^{}/(.*/)?({names})(/.*)?$", rooted_regex(workspace))
}

/// A Seatbelt regex that re-allows the three strict dotenv template files. The
/// Seatbelt profile applies this rule after [`workspace_credential_regex`]
/// (last match wins), so a template stays readable while `.env.example.local`
/// and every other variant remain denied. A template under a secret-named
/// directory is re-denied by [`workspace_template_under_secret_regex`].
#[cfg(target_os = "macos")]
pub fn workspace_template_regex(workspace: &Path) -> String {
    format!(
        "^{}/(.*/)?{}$",
        rooted_regex(workspace),
        case_fold(WORKSPACE_TEMPLATE_PATTERN)
    )
}

/// A Seatbelt regex that re-denies a template file below a secret-named
/// directory (for example `.ssh/.env.example`), so the template re-allow cannot
/// open a file inside a credential directory.
#[cfg(target_os = "macos")]
pub fn workspace_template_under_secret_regex(workspace: &Path) -> String {
    let names = case_fold(&WORKSPACE_CREDENTIAL_BASENAMES.join("|"));
    format!(
        "^{}/(.*/)?({names})/(.*/)?{}$",
        rooted_regex(workspace),
        case_fold(WORKSPACE_TEMPLATE_PATTERN)
    )
}

/// Fold every ASCII letter in a regex fragment to an `[aA]` class, so the rule
/// matches case-insensitively on a case-sensitive volume too. Regex
/// metacharacters and the one class used here (`[^/]`, which holds no letter)
/// are left as-is.
#[cfg(target_os = "macos")]
fn case_fold(pattern: &str) -> String {
    let mut folded = String::with_capacity(pattern.len() * 3);
    for character in pattern.chars() {
        if character.is_ascii_alphabetic() {
            folded.push('[');
            folded.push(character.to_ascii_lowercase());
            folded.push(character.to_ascii_uppercase());
            folded.push(']');
        } else {
            folded.push(character);
        }
    }
    folded
}

/// The regex-escaped, canonicalized workspace root.
#[cfg(target_os = "macos")]
fn rooted_regex(workspace: &Path) -> String {
    escape_regex(&resolve_nearest_existing(workspace).to_string_lossy())
}

/// Escape the regex metacharacters in a literal path so it can be embedded in a
/// Seatbelt regex. `/` is not a metacharacter and is left as is.
#[cfg(target_os = "macos")]
fn escape_regex(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '.' | '[' | ']' | '(' | ')' | '{' | '}' | '*' | '+' | '?' | '^' | '$' | '|' | '\\'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn home_secret_locations_are_under_home_and_empty_home_is_empty() {
        // A canonical home, so the written and resolved spellings coincide
        // (`/home` is a link on macOS).
        let dir = tempfile::tempdir().unwrap();
        let home = &dir.path().canonicalize().unwrap().join("alice");
        let locations = home_secret_locations_with(home, None);
        assert_eq!(locations.len(), HOME_SECRET_LOCATIONS.len());
        assert!(locations.contains(&home.join(".ssh")));
        assert!(locations.contains(&home.join(".docker/config.json")));
        assert!(locations.contains(&home.join(".config/gh")));
        assert!(home_secret_locations(Path::new("")).is_empty());
    }

    #[test]
    fn home_secret_locations_follow_a_symlink_to_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, home.join(".ssh")).unwrap();

        let locations = home_secret_locations(&home);
        assert!(
            locations.contains(&outside.canonicalize().unwrap()),
            "{locations:?}"
        );
    }

    #[test]
    fn workspace_regex_canonicalizes_the_root_and_denies_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let real = real.canonicalize().unwrap();

        // The root is the canonical path, the credential names are case-folded,
        // the path sequences are present, and a secret directory's contents are
        // denied.
        let deny = workspace_credential_regex(&link);
        assert!(deny.starts_with('^'), "{deny}");
        assert!(deny.contains(r"\.[eE][nN][vV]"), "{deny}");
        assert!(deny.contains(r"\.[aA][wW][sS]/[cC]"), "{deny}");
        assert!(deny.ends_with("(/.*)?$"), "{deny}");
        // The rule is anchored at the canonical workspace root: a credential
        // inside the workspace matches, but the same name outside it does not.
        // Built with the `regex` crate so a mutation that empties the escaped
        // root (which would deny the name everywhere) is caught.
        let deny_re = regex::Regex::new(&deny).expect("the deny regex must compile");
        assert!(
            deny_re.is_match(&format!("{}/.env", real.display())),
            "{deny}"
        );
        assert!(!deny_re.is_match("/elsewhere/.env"), "{deny}");

        // The template re-allow folds the template name and stays anchored.
        let allow = workspace_template_regex(&link);
        assert!(allow.starts_with('^'), "{allow}");
        assert!(allow.contains(r"\.[eE][nN][vV]\."), "{allow}");
        assert!(allow.ends_with('$'), "{allow}");

        // The template re-deny requires a secret component before the template.
        let re_deny = workspace_template_under_secret_regex(&link);
        assert!(re_deny.contains(r"\.[sS][sS][hH]"), "{re_deny}");
        assert!(re_deny.ends_with('$'), "{re_deny}");
    }
}

#[cfg(test)]
mod missing_secret_tests {
    use super::{home_secret_locations_with, resolve_nearest_existing};

    #[test]
    fn a_missing_path_is_resolved_through_its_nearest_existing_parent() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let real = base.join("real");
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, base.join("link")).unwrap();

        assert_eq!(
            resolve_nearest_existing(&base.join("link/a/b")),
            real.join("a/b")
        );
        assert_eq!(resolve_nearest_existing(&base.join("link")), real);
        assert_eq!(
            resolve_nearest_existing(std::path::Path::new("relative/missing")),
            std::path::Path::new("relative/missing")
        );
    }

    #[test]
    fn a_dangling_link_is_followed_by_hand_and_a_cycle_ends() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(base.join("real")).unwrap();
        // Absolute and relative dangling targets, and a chain of them.
        std::os::unix::fs::symlink(base.join("real/ssh"), base.join("abs")).unwrap();
        std::os::unix::fs::symlink("real/ssh", base.join("rel")).unwrap();
        std::os::unix::fs::symlink(base.join("abs"), base.join("chain")).unwrap();
        for link in ["abs", "rel", "chain"] {
            assert_eq!(
                resolve_nearest_existing(&base.join(link).join("id")),
                base.join("real/ssh/id"),
                "{link}"
            );
        }
        // A cycle ends and keeps the rest as written.
        std::os::unix::fs::symlink(base.join("b"), base.join("a")).unwrap();
        std::os::unix::fs::symlink(base.join("a"), base.join("b")).unwrap();
        let cyclic = resolve_nearest_existing(&base.join("a/id"));
        assert!(cyclic.ends_with("id"), "{cyclic:?}");
    }

    #[test]
    fn a_missing_cargo_credential_under_a_linked_cargo_is_denied_by_both_spellings() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let home = base.join("home");
        let real = base.join("real-cargo");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, home.join(".cargo")).unwrap();

        let locations = home_secret_locations_with(&home, None);
        assert!(
            locations.contains(&real.join("credentials.toml")),
            "{locations:?}"
        );
        assert!(
            locations.contains(&home.join(".cargo/credentials.toml")),
            "{locations:?}"
        );
    }
}

#[cfg(test)]
mod cargo_credential_tests {
    use super::{Path, home_secret_locations_with};

    #[test]
    fn home_list_denies_both_cargo_credential_files_but_not_the_caches() {
        let home = Path::new("/home/alice");
        let locations = home_secret_locations_with(home, None);
        assert!(locations.contains(&home.join(".cargo/credentials.toml")));
        assert!(locations.contains(&home.join(".cargo/credentials")));
        for cache in [".cargo/registry", ".cargo/git", ".cargo/.package-cache"] {
            assert!(
                !locations
                    .iter()
                    .any(|location| home.join(cache).starts_with(location)),
                "{cache} must stay readable: {locations:?}"
            );
        }
    }

    #[test]
    fn cargo_home_outside_the_home_adds_its_credential_files() {
        let home = Path::new("/home/alice");
        let cargo_home = Path::new("/opt/cargo-home-695");
        let locations = home_secret_locations_with(home, Some(cargo_home));
        assert!(locations.contains(&cargo_home.join("credentials.toml")));
        assert!(locations.contains(&cargo_home.join("credentials")));
        assert!(!locations.contains(&cargo_home.to_path_buf()));
        assert!(locations.contains(&home.join(".cargo/credentials.toml")));
    }

    #[test]
    fn relative_or_empty_cargo_home_is_ignored_and_a_default_one_is_not_duplicated() {
        let home = Path::new("/home/alice");
        let base = home_secret_locations_with(home, None);
        assert_eq!(home_secret_locations_with(home, Some(Path::new(""))), base);
        assert_eq!(
            home_secret_locations_with(home, Some(Path::new("relative/cargo"))),
            base
        );
        assert_eq!(
            home_secret_locations_with(home, Some(&home.join(".cargo"))),
            base
        );
    }

    #[test]
    fn symlinked_cargo_home_denies_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let real = dir.path().join("real-cargo");
        let link = dir.path().join("link-cargo");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let locations = home_secret_locations_with(&home, Some(&link));
        assert!(
            locations.contains(&real.canonicalize().unwrap().join("credentials.toml")),
            "{locations:?}"
        );
    }
}
