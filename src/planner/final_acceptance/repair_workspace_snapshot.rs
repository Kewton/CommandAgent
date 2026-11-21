//! Before/after fingerprint of the acceptance-relevant sources around one
//! final-acceptance repair (Issue #851).
//!
//! The snapshot keeps the `BTreeMap<String, Option<Vec<u8>>>` shape that
//! `changed_snapshot_paths` compares, but the value is never file content:
//! `None` is an absent path, `sha256:<hex>` a regular file, `kind:dir` a
//! directory, and `unknown:<n>` a path that could not be judged safely. Every
//! `unknown:<n>` carries a fresh number, so an unjudgeable path never compares
//! equal to itself and the repair proceeds to re-verification instead of
//! being skipped as "no source change".
//!
//! Candidates are the existing expected, config and route-bound paths plus an
//! independent walk of the workspace for Web source extensions and `.py`, so
//! the delta does not depend on what the repair session reported. Paths an
//! admitted artifact check bound, with the profile's declared artifacts, come
//! first whatever their extension (Issue #949, `artifact_repair_paths`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use super::artifact_repair_paths::artifact_repair_candidates;
use super::{
    merge_final_acceptance_snapshot_candidates, route_bound_source_paths, safe_dir_rel_path,
    safe_final_acceptance_snapshot_rel_path, snapshot_source_candidates,
};
use crate::tools::sensitive_path::{SkippedPath, sensitive_skip};

const MAX_CANDIDATES: usize = 128;
const MAX_DEPTH: usize = 4;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_VISITED_ENTRIES: usize = 20_000;
const INCOMPLETE_SCAN_KEY: &str = "<workspace-scan-incomplete>";
const SKIPPED_DIR_NAMES: [&str; 12] = [
    "node_modules",
    "target",
    "dist",
    "dist-packages",
    "build",
    "out",
    "__pycache__",
    "venv",
    "env",
    "site-packages",
    "vendor",
    "coverage",
];

static UNKNOWN_SEQ: AtomicU64 = AtomicU64::new(0);

type Fingerprint = Option<Vec<u8>>;

fn unknown() -> Fingerprint {
    let seq = UNKNOWN_SEQ.fetch_add(1, Ordering::Relaxed);
    Some(format!("unknown:{seq}").into_bytes())
}

pub(super) fn capture(
    root: &Path,
    profile: &str,
    goal: &str,
    expected_paths: &[String],
    extra_paths: &[String],
    recorded: &[String],
) -> BTreeMap<String, Fingerprint> {
    let artifacts = artifact_repair_candidates(root, profile, goal, recorded);
    capture_with_artifacts(root, profile, goal, expected_paths, extra_paths, &artifacts)
}

/// [`capture`] with the artifact candidates of Issue #949 (see
/// `artifact_repair_paths`) placed first, so the candidate cap keeps them.
pub(super) fn capture_with_artifacts(
    root: &Path,
    profile: &str,
    goal: &str,
    expected_paths: &[String],
    extra_paths: &[String],
    artifacts: &[String],
) -> BTreeMap<String, Fingerprint> {
    let mut paths = artifacts.to_vec();
    for path in legacy_candidates(root, profile, goal, expected_paths, extra_paths) {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    let mut walk = Walk::default();
    walk.dir(root, "", 0);
    for path in walk.found {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    // The candidate count and depth limits are the scope boundary, as in the
    // existing collector: what lies beyond them is not part of the delta and
    // is not marked unknown. Deep route-bound sources stay covered because
    // the legacy candidates come first and are hashed directly.
    paths.truncate(MAX_CANDIDATES);
    let mut snapshot = BTreeMap::new();
    let mut budget = MAX_TOTAL_BYTES;
    for rel in paths.into_iter().collect::<BTreeSet<_>>() {
        if let Some(fingerprint) = fingerprint(root, &rel, &mut budget) {
            snapshot.insert(rel, fingerprint);
        }
    }
    for rel in walk.unjudgeable {
        if !matches!(
            sensitive_skip(root, &root.join(&rel)),
            Some(SkippedPath::Credential)
        ) {
            snapshot.insert(rel, unknown());
        }
    }
    if walk.incomplete {
        snapshot.insert(INCOMPLETE_SCAN_KEY.to_string(), unknown());
    }
    snapshot
}

fn legacy_candidates(
    root: &Path,
    profile: &str,
    goal: &str,
    expected_paths: &[String],
    extra_paths: &[String],
) -> Vec<String> {
    let mut paths = snapshot_source_candidates(root, profile, goal, expected_paths);
    merge_final_acceptance_snapshot_candidates(
        &mut paths,
        expected_paths.iter().map(String::as_str),
    );
    merge_final_acceptance_snapshot_candidates(&mut paths, extra_paths.iter().map(String::as_str));
    merge_final_acceptance_snapshot_candidates(
        &mut paths,
        crate::planner::repair_targeting::final_acceptance_snapshot_candidates(root, profile)
            .iter()
            .map(String::as_str),
    );
    for path in route_bound_source_paths(root, profile) {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    for path in extra_paths.iter().chain(expected_paths) {
        if let Some(rel) = python_source_rel_path(path)
            && !paths.contains(&rel)
        {
            paths.push(rel);
        }
    }
    let explicit = extra_paths
        .iter()
        .chain(expected_paths)
        .filter_map(|path| candidate_rel_path(path))
        .collect::<BTreeSet<_>>();
    paths
        .into_iter()
        .filter_map(|path| candidate_rel_path(&path))
        .filter(|rel| {
            !has_component(rel, |part| part.starts_with('.'))
                && (explicit.contains(rel) || !has_component(rel, is_skipped_dir_name))
        })
        .collect()
}

fn has_component(rel: &str, matches: impl Fn(&str) -> bool) -> bool {
    rel.split('/').any(matches)
}

fn is_skipped_dir_name(name: &str) -> bool {
    SKIPPED_DIR_NAMES.contains(&name.to_ascii_lowercase().as_str())
}

fn python_source_rel_path(raw: &str) -> Option<String> {
    let rel = safe_dir_rel_path(raw)?;
    (Path::new(&rel).extension().and_then(|ext| ext.to_str()) == Some("py")).then_some(rel)
}

fn candidate_rel_path(raw: &str) -> Option<String> {
    safe_final_acceptance_snapshot_rel_path(raw).or_else(|| python_source_rel_path(raw))
}

#[derive(Default)]
struct Walk {
    found: Vec<String>,
    unjudgeable: Vec<String>,
    incomplete: bool,
    visited: usize,
}

impl Walk {
    fn dir(&mut self, root: &Path, rel_dir: &str, depth: usize) {
        let dir = if rel_dir.is_empty() {
            root.to_path_buf()
        } else {
            root.join(rel_dir)
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            self.incomplete = true;
            return;
        };
        let mut names = Vec::new();
        for entry in entries {
            let Ok(entry) = entry else {
                self.incomplete = true;
                continue;
            };
            let Ok(file_type) = entry.file_type() else {
                self.incomplete = true;
                continue;
            };
            names.push((entry.file_name().to_string_lossy().to_string(), file_type));
        }
        names.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, file_type) in names {
            if name.starts_with('.') || is_skipped_dir_name(&name) {
                continue;
            }
            self.visited += 1;
            if self.visited > MAX_VISITED_ENTRIES {
                self.incomplete = true;
                return;
            }
            let rel = if rel_dir.is_empty() {
                name
            } else {
                format!("{rel_dir}/{name}")
            };
            if file_type.is_dir() {
                if depth < MAX_DEPTH {
                    self.dir(root, &rel, depth + 1);
                }
            } else if candidate_rel_path(&rel).is_some() {
                self.found.push(rel);
            } else if file_type.is_symlink()
                && !std::fs::metadata(root.join(&rel)).is_ok_and(|meta| !meta.is_dir())
            {
                self.unjudgeable.push(rel);
            }
        }
    }
}

/// `None` leaves the path out of the snapshot (a credential path is neither
/// read nor named); `Some(None)` is an absent path.
fn fingerprint(root: &Path, rel: &str, budget: &mut u64) -> Option<Fingerprint> {
    let path = root.join(rel);
    match sensitive_skip(root, &path) {
        Some(SkippedPath::Credential) => return None,
        Some(SkippedPath::Outside) => return Some(unknown()),
        None => {}
    }
    let mut meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == ErrorKind::NotFound => return Some(None),
        Err(_) => return Some(unknown()),
    };
    if meta.file_type().is_symlink() {
        meta = match std::fs::metadata(&path) {
            Ok(meta) => meta,
            Err(_) => return Some(unknown()),
        };
    }
    if meta.is_dir() {
        return Some(Some(b"kind:dir".to_vec()));
    }
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES || meta.len() > *budget {
        return Some(unknown());
    }
    let Ok(file) = File::open(&path) else {
        return Some(unknown());
    };
    let mut reader = file.take(MAX_FILE_BYTES + 1);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    let mut total = 0u64;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                hasher.update(&buffer[..read]);
                total += read as u64;
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Some(unknown()),
        }
    }
    let stable = std::fs::metadata(&path)
        .is_ok_and(|after| after.len() == total && after.modified().ok() == meta.modified().ok());
    if total > MAX_FILE_BYTES || !stable {
        return Some(unknown());
    }
    *budget -= total;
    let hex = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Some(Some(format!("sha256:{hex}").into_bytes()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    use super::super::changed_snapshot_paths;
    use super::{INCOMPLETE_SCAN_KEY, capture};

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn snap(root: &Path, extra: &[String]) -> super::BTreeMap<String, super::Fingerprint> {
        capture(root, "python-cli", "build a cli", &[], extra, &[])
    }

    fn changed(root: &Path, before: &super::BTreeMap<String, super::Fingerprint>) -> Vec<String> {
        changed_snapshot_paths(before, &snap(root, &[]))
    }

    #[test]
    fn rewrite_touch_and_create_then_delete_are_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app/main.py", "print(1)\n");
        write(dir.path(), "pyproject.toml", "[project]\nname = \"a\"\n");
        let before = snap(dir.path(), &[]);
        write(dir.path(), "src/app/main.py", "print(1)\n");
        let file = std::fs::File::options()
            .write(true)
            .open(dir.path().join("pyproject.toml"))
            .unwrap();
        file.set_modified(SystemTime::now() + Duration::from_secs(60))
            .unwrap();
        write(dir.path(), "src/app/scratch.py", "x = 1\n");
        std::fs::remove_file(dir.path().join("src/app/scratch.py")).unwrap();
        write(dir.path(), "src/app/__pycache__/main.pyc", "bytecode");
        write(dir.path(), "notes.log", "log");
        assert_eq!(changed(dir.path(), &before), Vec::<String>::new());
    }

    #[test]
    fn deletion_move_creation_and_py_edit_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app/main.py", "print(1)\n");
        write(dir.path(), "src/csv_stats/main.py", "print(2)\n");
        write(dir.path(), "src/csv_stats/util.py", "u = 1\n");
        let before = snap(dir.path(), &[]);
        std::fs::remove_file(dir.path().join("src/app/main.py")).unwrap();
        std::fs::rename(
            dir.path().join("src/csv_stats/util.py"),
            dir.path().join("src/csv_stats/helpers.py"),
        )
        .unwrap();
        write(dir.path(), "src/csv_stats/main.py", "print(3)\n");
        write(dir.path(), "web/app.tsx", "export {}\n");
        assert_eq!(
            changed(dir.path(), &before),
            [
                "src/app/main.py",
                "src/csv_stats/helpers.py",
                "src/csv_stats/main.py",
                "src/csv_stats/util.py",
                "web/app.tsx",
            ]
        );
    }

    #[test]
    fn same_size_same_mtime_content_change_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app/main.py", "aaaa");
        let path = dir.path().join("src/app/main.py");
        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        let before = snap(dir.path(), &[]);
        std::fs::write(&path, "bbbb").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        assert_eq!(changed(dir.path(), &before), ["src/app/main.py"]);
    }

    #[test]
    fn snapshot_values_are_digests_never_content() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app/main.py", "FAKE_BODY_MARKER");
        let snapshot = snap(dir.path(), &[]);
        let value = snapshot["src/app/main.py"].clone().unwrap();
        let text = String::from_utf8(value).unwrap();
        assert!(text.starts_with("sha256:"), "{text}");
        assert!(!text.contains("FAKE_BODY_MARKER"));
    }

    #[test]
    fn credential_runtime_vcs_and_dependency_paths_are_never_candidates() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app/main.py", "ok = 1\n");
        write(dir.path(), ".env.js", "FAKE_SECRET_VALUE");
        write(dir.path(), "config/.env.local.js", "FAKE_SECRET_VALUE");
        write(dir.path(), ".ssh/id.py", "FAKE_SECRET_VALUE");
        write(dir.path(), ".git/hooks/x.js", "x");
        write(dir.path(), ".commandagent/evidence/e.py", "x");
        write(dir.path(), ".anvil/runs/r.js", "x");
        write(dir.path(), "node_modules/pkg/index.js", "x");
        write(dir.path(), "target/gen.js", "x");
        write(dir.path(), ".venv/lib/site.py", "x");
        write(dir.path(), ".cache/a.js", "x");
        let extra = [
            ".env.js".to_string(),
            "config/.env.local.js".to_string(),
            ".ssh/id.py".to_string(),
            ".cache/a.js".to_string(),
            ".commandagent/evidence/e.py".to_string(),
        ];
        let snapshot = snap(dir.path(), &extra);
        let keys = snapshot.keys().map(String::as_str).collect::<Vec<_>>();
        assert!(keys.contains(&"src/app/main.py"), "{keys:?}");
        for key in &keys {
            assert!(
                [".", "config/", "node_modules", "target"]
                    .iter()
                    .all(|excluded| !key.starts_with(excluded)),
                "{keys:?}"
            );
        }
        write(dir.path(), ".env.js", "FAKE_SECRET_CHANGED");
        assert_eq!(
            changed_snapshot_paths(&snapshot, &snap(dir.path(), &extra)),
            Vec::<String>::new()
        );
    }

    #[test]
    fn explicit_expected_path_under_a_build_directory_is_still_judged() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "build/out.js", "a");
        write(dir.path(), "build/other.js", "a");
        let expected = ["build/out.js".to_string()];
        let before = capture(dir.path(), "generic", "goal", &expected, &[], &[]);
        assert!(before.contains_key("build/out.js"));
        assert!(!before.contains_key("build/other.js"));
        write(dir.path(), "build/out.js", "b");
        assert_eq!(
            changed_snapshot_paths(
                &before,
                &capture(dir.path(), "generic", "goal", &expected, &[], &[])
            ),
            ["build/out.js"]
        );
    }

    #[test]
    fn oversized_file_is_unknown_and_never_the_same() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app/big.py", &"a".repeat(1024 * 1024 + 1));
        let before = snap(dir.path(), &[]);
        assert_eq!(changed(dir.path(), &before), ["src/app/big.py"]);
    }

    #[test]
    fn file_at_the_size_limit_is_hashed() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app/edge.py", &"a".repeat(1024 * 1024));
        let before = snap(dir.path(), &[]);
        assert_eq!(changed(dir.path(), &before), Vec::<String>::new());
    }

    #[test]
    fn total_budget_overflow_is_unknown() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..17 {
            write(
                dir.path(),
                &format!("pkg/m{index:02}.py"),
                &"a".repeat(1024 * 1024),
            );
        }
        let before = snap(dir.path(), &[]);
        assert_eq!(changed(dir.path(), &before), ["pkg/m16.py"]);
    }

    #[test]
    fn candidate_limit_is_a_scope_boundary_without_an_incomplete_mark() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..130 {
            write(dir.path(), &format!("pkg/m{index:03}.py"), "x = 1\n");
        }
        let before = snap(dir.path(), &[]);
        assert_eq!(before.len(), 128);
        assert!(!before.contains_key(INCOMPLETE_SCAN_KEY));
        assert_eq!(changed(dir.path(), &before), Vec::<String>::new());
    }

    #[test]
    fn source_below_the_depth_limit_is_outside_the_delta_without_a_mark() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a/b/c/d/ok.py", "x = 1\n");
        write(dir.path(), "a/b/c/d/e/deep.py", "x = 1\n");
        let before = snap(dir.path(), &[]);
        assert!(before.contains_key("a/b/c/d/ok.py"));
        assert!(!before.contains_key("a/b/c/d/e/deep.py"));
        assert!(!before.contains_key(INCOMPLETE_SCAN_KEY));
        write(dir.path(), "a/b/c/d/e/deep.py", "x = 2\n");
        assert_eq!(changed(dir.path(), &before), Vec::<String>::new());
    }

    #[test]
    fn explicit_deep_path_is_still_hashed_directly() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a/b/c/d/e/route.ts", "x");
        let extra = ["a/b/c/d/e/route.ts".to_string()];
        let before = snap(dir.path(), &extra);
        assert!(before["a/b/c/d/e/route.ts"].is_some());
        write(dir.path(), "a/b/c/d/e/route.ts", "y");
        assert_eq!(
            changed_snapshot_paths(&before, &snap(dir.path(), &extra)),
            ["a/b/c/d/e/route.ts"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_a_credential_is_dropped_by_name_and_never_read() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app/main.py", "ok = 1\n");
        write(dir.path(), ".env.js", "FAKE_SECRET_VALUE");
        std::os::unix::fs::symlink("../../.env.js", dir.path().join("src/app/key.js")).unwrap();
        std::os::unix::fs::symlink(
            "missing-target",
            dir.path().join("src/app/service-account-x.json"),
        )
        .unwrap();
        let snapshot = snap(dir.path(), &[]);
        assert!(!snapshot.contains_key("src/app/key.js"), "{snapshot:?}");
        assert!(!snapshot.contains_key("src/app/service-account-x.json"));
        assert!(!format!("{snapshot:?}").contains("FAKE_SECRET_VALUE"));
    }

    #[test]
    fn missing_candidate_compares_as_absent_not_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let extra = ["src/gone/main.py".to_string()];
        let before = snap(dir.path(), &extra);
        assert_eq!(before["src/gone/main.py"], None);
        assert_eq!(
            changed_snapshot_paths(&before, &snap(dir.path(), &extra)),
            Vec::<String>::new()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed_and_are_never_the_same() {
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "ext.py", "FAKE_OUTSIDE_BODY");
        write(outside.path(), "lib/inner.py", "FAKE_OUTSIDE_BODY");
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app/main.py", "ok = 1\n");
        std::os::unix::fs::symlink(
            outside.path().join("ext.py"),
            dir.path().join("src/app/linked.py"),
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path().join("lib"), dir.path().join("src/lib")).unwrap();
        std::os::unix::fs::symlink(
            dir.path().join("src/app/main.py"),
            dir.path().join("src/app/alias.py"),
        )
        .unwrap();
        let before = snap(dir.path(), &[]);
        assert!(!format!("{before:?}").contains("FAKE_OUTSIDE_BODY"));
        assert!(!before.contains_key("src/lib/inner.py"));
        assert_eq!(
            changed(dir.path(), &before),
            ["src/app/linked.py", "src/lib"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_file_is_unknown() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app/main.py", "ok = 1\n");
        let path = dir.path().join("src/app/main.py");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::File::open(&path).is_ok() {
            return;
        }
        let before = snap(dir.path(), &[]);
        assert_eq!(changed(dir.path(), &before), ["src/app/main.py"]);
    }
}
