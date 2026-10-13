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

use std::path::{Path, PathBuf};

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
];

/// The home secret locations as canonical absolute paths under `home`. Returns
/// an empty list when `home` is empty, so a run without `HOME` denies nothing
/// rather than resolving names relative to the process working directory. Each
/// location is canonicalized, so a symlinked location (`~/.ssh` pointing outside
/// the home) denies its target as well.
pub fn home_secret_locations(home: &Path) -> Vec<PathBuf> {
    if home.as_os_str().is_empty() {
        return Vec::new();
    }
    let home = canonicalize_or_keep(home);
    HOME_SECRET_LOCATIONS
        .iter()
        .map(|relative| canonicalize_or_keep(&home.join(relative)))
        .collect()
}

/// Canonicalize a path, keeping it as-is when it does not exist or cannot be
/// resolved.
fn canonicalize_or_keep(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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
    escape_regex(&canonicalize_or_keep(workspace).to_string_lossy())
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
        let home = Path::new("/home/alice");
        let locations = home_secret_locations(home);
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
