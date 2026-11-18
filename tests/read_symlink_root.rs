//! Issue #966 — Read headings stay workspace-relative under a symlinked root.
//!
//! One synthetic workspace is read through three spellings of its root: the
//! canonical path, a path below a symlinked parent, and a symlink to the
//! workspace itself. Every route of `tests/corpus/apps/read-symlink-root` must
//! give the same heading and body from all three, and refusals must not change.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use commandagent::mode::ExecutionMode;
use commandagent::tools::path_guard::{resolve_existing, resolve_existing_checked};
use commandagent::tools::read;
use commandagent::tools::registry::{ToolContext, ToolRegistry, tool_error_kind};
use commandagent::tools::workspace_policy::WorkspacePolicy;

const FIXTURE: &str = "tests/corpus/apps/read-symlink-root/fixtures/read-headings.jsonl";
const NOTE: &str = "dir 空白 データ/ノート.txt";

struct Workspace {
    _dir: tempfile::TempDir,
    /// The temporary directory as handed out, and canonicalized.
    temp_spellings: [String; 2],
    canonical: PathBuf,
    parent_link: PathBuf,
    self_link: PathBuf,
    outside_file: PathBuf,
}

fn workspace() -> Workspace {
    let dir = tempfile::tempdir().unwrap();
    let temp = dir.path().to_path_buf();
    let temp_canonical = temp.canonicalize().unwrap();
    let real_parent = temp.join("real parent 実体");
    let real = real_parent.join("ws");
    std::fs::create_dir_all(real.join("dir 空白 データ")).unwrap();
    std::fs::write(real.join(NOTE), "一行目\n二行目\n三行目\n四行目\n").unwrap();
    let big = (1..=300)
        .map(|n| format!("line {n} 日本語 padding text"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(big.len() > 6_000);
    std::fs::write(real.join("big file.txt"), big).unwrap();
    std::os::unix::fs::symlink("dir 空白 データ/ノート.txt", real.join("inner link.txt")).unwrap();
    std::fs::write(real.join(".env"), "SYNTHETIC_ONLY=1\n").unwrap();

    let outside = temp.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("secret.txt");
    std::fs::write(&outside_file, "outside body\n").unwrap();
    std::os::unix::fs::symlink(&outside_file, real.join("out link.txt")).unwrap();

    let link_parent = temp.join("link parent リンク");
    std::os::unix::fs::symlink(&real_parent, &link_parent).unwrap();
    let self_link = temp.join("ws link");
    std::os::unix::fs::symlink(&real, &self_link).unwrap();

    let canonical = real.canonicalize().unwrap();
    assert_ne!(link_parent.join("ws"), canonical);
    assert_ne!(self_link, canonical);
    Workspace {
        _dir: dir,
        temp_spellings: [
            temp.to_string_lossy().into_owned(),
            temp_canonical.to_string_lossy().into_owned(),
        ],
        canonical,
        parent_link: link_parent.join("ws"),
        self_link,
        outside_file,
    }
}

impl Workspace {
    fn roots(&self) -> [(&'static str, &Path); 3] {
        [
            ("canonical", self.canonical.as_path()),
            ("parent_link", self.parent_link.as_path()),
            ("self_link", self.self_link.as_path()),
        ]
    }

    /// Removes the temporary directory and fails if it cannot be removed.
    fn close(self) {
        self._dir.close().unwrap();
    }

    fn assert_no_host_path(&self, label: &str, text: &str) {
        for spelling in &self.temp_spellings {
            assert!(
                !text.contains(spelling.as_str()),
                "{label}: output leaks the temporary root: {text}"
            );
        }
    }
}

fn context(root: &Path, eval_events_path: Option<PathBuf>) -> ToolContext {
    ToolContext {
        root: root.to_path_buf(),
        mode: ExecutionMode::Act,
        auto_approve: false,
        interactive_approval: false,
        offline: true,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path,
        expected_paths: Vec::new(),
        protected_paths: Vec::new(),
    }
}

fn fixture_rows() -> Vec<Value> {
    let text = std::fs::read_to_string(FIXTURE).unwrap();
    let rows = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 6, "fixture rows: {rows:?}");
    rows
}

fn arguments(row: &Value) -> Value {
    let mut arguments = json!({ "path": row["path"] });
    for key in ["start_line", "end_line"] {
        if let Some(value) = row.get(key) {
            arguments[key] = value.clone();
        }
    }
    arguments
}

#[test]
fn every_route_reads_the_same_from_canonical_and_symlinked_roots() {
    let ws = workspace();
    for row in fixture_rows() {
        let route = row["route"].as_str().unwrap();
        let heading = row["heading"].as_str().unwrap();
        let arguments = arguments(&row);
        let mut outputs = Vec::new();
        for (name, root) in ws.roots() {
            let output = ToolRegistry::default()
                .execute("Read", &arguments, &context(root, None))
                .unwrap_or_else(|error| panic!("{route}/{name}: {error}"));
            let label = format!("{route}/{name}");
            assert_eq!(output.lines().next().unwrap_or(""), heading, "{label}");
            ws.assert_no_host_path(&label, &output);
            outputs.push((name, output));
        }
        let (_, canonical) = &outputs[0];
        for (name, output) in &outputs[1..] {
            assert_eq!(output, canonical, "{route}: {name} differs from canonical");
        }
        match route {
            "file" | "inner_symlink" => assert!(canonical.ends_with("四行目"), "{canonical}"),
            "range" => assert_eq!(canonical, &format!("{heading}\n二行目\n三行目")),
            "summary" => assert!(canonical.contains("file summarized"), "{canonical}"),
            "directory" => assert_eq!(canonical, &format!("{heading}\nノート.txt")),
            "root_directory" => assert!(canonical.contains("dir 空白 データ/"), "{canonical}"),
            other => panic!("unknown route {other}"),
        }
    }
}

#[test]
fn unchanged_read_event_names_the_workspace_relative_path() {
    let ws = workspace();
    let events_dir = tempfile::tempdir().unwrap();
    for (name, root) in ws.roots() {
        let events = events_dir.path().join(format!("{name}.jsonl"));
        let registry = ToolRegistry::default();
        let ctx = context(root, Some(events.clone()));
        let first = registry
            .execute("Read", &json!({ "path": NOTE }), &ctx)
            .unwrap();
        let second = registry
            .execute("Read", &json!({ "path": NOTE }), &ctx)
            .unwrap();
        assert_ne!(
            first, second,
            "{name}: second read must be the unchanged form"
        );
        ws.assert_no_host_path(name, &second);
        let text = std::fs::read_to_string(&events).unwrap();
        let event = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .find(|event| event["event"] == "tool_read_unchanged")
            .unwrap_or_else(|| panic!("{name}: no tool_read_unchanged in {text}"));
        assert_eq!(event["path"], NOTE, "{name}: {event}");
    }
}

#[test]
fn refusals_are_unchanged_under_a_symlinked_root() {
    let ws = workspace();
    let outside_abs = ws.outside_file.to_string_lossy().into_owned();
    let cases = [
        ("out link.txt", "path_confinement_error"),
        ("../outside/secret.txt", ""),
        (outside_abs.as_str(), ""),
        (".env", "workspace_policy_blocked"),
    ];
    for (raw, expected_kind) in cases {
        let mut kinds = Vec::new();
        for (name, root) in ws.roots() {
            let error = ToolRegistry::default()
                .execute("Read", &json!({ "path": raw }), &context(root, None))
                .expect_err(&format!("{raw}/{name} must be refused"));
            assert!(
                !error.to_string().contains("outside body"),
                "{raw}/{name}: {error}"
            );
            kinds.push(tool_error_kind(&error));
        }
        assert!(
            kinds.iter().all(|kind| *kind == kinds[0]),
            "{raw}: {kinds:?}"
        );
        if !expected_kind.is_empty() {
            assert_eq!(kinds[0], expected_kind, "{raw}");
        }
    }

    // The leaf entry with an outside target keeps its confinement error.
    for (name, root) in ws.roots() {
        let error = read::run(
            root,
            &ws.outside_file,
            None,
            None,
            WorkspacePolicy::NormalTask,
        )
        .unwrap_err();
        assert_eq!(
            tool_error_kind(&error),
            "path_confinement_error",
            "{name}: {error}"
        );
    }
}

#[test]
fn unreadable_directory_is_still_refused_under_a_symlinked_root() {
    let ws = workspace();
    let locked = ws.canonical.join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::write(locked.join("f.txt"), "hidden\n").unwrap();
    let restore = RestorePermissions::lock(&locked);

    let probe = std::fs::read_dir(&locked);
    let results = ws.roots().map(|(name, root)| {
        (
            name,
            ToolRegistry::default()
                .execute(
                    "Read",
                    &json!({ "path": "locked/f.txt" }),
                    &context(root, None),
                )
                .map_err(|error| tool_error_kind(&error)),
        )
    });
    drop(restore);

    assert_eq!(
        probe.as_ref().err().map(|e| e.kind()),
        Some(std::io::ErrorKind::PermissionDenied),
        "this test must exercise actual EACCES; run under an unprivileged account"
    );
    for (name, result) in &results {
        assert!(result.is_err(), "{name}: {result:?}");
        assert_eq!(result, &results[0].1, "{name}");
    }
    ws.close();
}

/// B1 of the #966 review: the root alias is swapped after the containment check
/// and before the heading is made. The checks and the swap are separate calls,
/// so the order is fixed without sleeping or racing a thread.
#[test]
fn heading_keeps_the_checked_root_when_the_root_alias_moves_after_the_check() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().canonicalize().unwrap().join("parent 親");
    let real = parent.join("ws");
    std::fs::create_dir_all(&real).unwrap();
    std::fs::write(real.join("file.txt"), "body\n").unwrap();
    let alias = dir.path().join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();

    let checked = resolve_existing_checked(&alias, "file.txt").unwrap();
    let leaf_target = resolve_existing(&alias, "file.txt").unwrap();
    assert_eq!(checked.root, real.canonicalize().unwrap());
    assert_eq!(leaf_target, checked.path);

    // The seam: the alias now names the parent of the checked root.
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&parent, &alias).unwrap();

    let policy = WorkspacePolicy::NormalTask;
    let paired = read::run_checked(
        &alias,
        Some(&checked.root),
        &checked.path,
        None,
        None,
        policy,
    )
    .unwrap();
    assert_eq!(paired, "file.txt\nbody");

    // Without the checked root the leaf does not resolve the alias again: it
    // keeps the old absolute heading instead of relabeling the target as
    // `ws/file.txt` under the moved alias.
    let leaf = read::run(&alias, &leaf_target, None, None, policy).unwrap();
    let heading = leaf.lines().next().unwrap_or("");
    let absolute = leaf_target.to_string_lossy();
    assert!(
        heading == "file.txt" || heading == absolute,
        "heading names the target under the moved alias: {heading}"
    );
    assert_ne!(heading, "ws/file.txt");
}

/// B1 remainder of the #966 review: one registry reads a route, the root alias
/// is moved to the parent after that resolution (same thread, no sleep), and
/// the same target is read again as `ws/...`. Every heading, the cached
/// unchanged response, and `tool_read_unchanged.path` must come from the
/// current check, not from the earlier root.
#[test]
fn one_registry_labels_every_route_from_the_current_root_after_the_alias_moves() {
    for row in fixture_rows() {
        let route = row["route"].as_str().unwrap();
        let ws = workspace();
        let events_dir = tempfile::tempdir().unwrap();
        let events = events_dir.path().join("events.jsonl");
        let registry = ToolRegistry::default();
        let ctx = context(&ws.self_link, Some(events.clone()));
        registry
            .execute("Read", &arguments(&row), &ctx)
            .unwrap_or_else(|error| panic!("{route}: first read: {error}"));

        // The alias now names the parent of the root that was checked.
        std::fs::remove_file(&ws.self_link).unwrap();
        std::os::unix::fs::symlink(ws.canonical.parent().unwrap(), &ws.self_link).unwrap();

        let moved = |value: &str| match value {
            "." | "" => "ws".to_string(),
            other => format!("ws/{other}"),
        };
        let mut again = arguments(&row);
        again["path"] = json!(moved(row["path"].as_str().unwrap()));
        let expected = moved(row["heading"].as_str().unwrap());
        let output = registry
            .execute("Read", &again, &ctx)
            .unwrap_or_else(|error| panic!("{route}: second read: {error}"));
        assert_eq!(output.lines().next().unwrap_or(""), expected, "{route}");
        ws.assert_no_host_path(route, &output);

        let unchanged = std::fs::read_to_string(&events)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|event| event["event"] == "tool_read_unchanged")
            .collect::<Vec<_>>();
        if matches!(route, "directory" | "root_directory") {
            assert!(unchanged.is_empty(), "{route}: {unchanged:?}");
        } else {
            assert!(output.contains("unchanged since"), "{route}: {output}");
            assert_eq!(unchanged.len(), 1, "{route}: {unchanged:?}");
            assert_eq!(unchanged[0]["path"], expected.as_str(), "{route}");
        }
    }
}

/// R-966d2: a heading that itself spans lines (a directory name with a real
/// LF) must not leave a fragment in the cached body. One registry reads the
/// file under the parent root, the alias moves to the inner workspace after
/// that resolution (same thread, no sleep), and the same file is read again.
#[test]
fn a_multi_line_heading_leaves_nothing_in_the_cached_body() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().canonicalize().unwrap().join("parent");
    let inner = parent.join("outer\nold").join("ws");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(
        inner.join("file.txt"),
        "first body line\nsecond body line\n",
    )
    .unwrap();
    let alias = dir.path().join("alias");
    std::os::unix::fs::symlink(&parent, &alias).unwrap();
    let events = dir.path().join("events.jsonl");
    let registry = ToolRegistry::default();
    let ctx = context(&alias, Some(events.clone()));

    let first = registry
        .execute("Read", &json!({ "path": "outer\nold/ws/file.txt" }), &ctx)
        .unwrap();
    assert_eq!(
        first,
        "outer\nold/ws/file.txt\nfirst body line\nsecond body line"
    );

    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&inner, &alias).unwrap();

    let again = registry
        .execute("Read", &json!({ "path": "file.txt" }), &ctx)
        .unwrap();
    let mut lines = again.lines();
    assert_eq!(lines.next(), Some("file.txt"), "{again}");
    let notice = lines.next().unwrap_or("");
    assert!(notice.contains("unchanged since"), "{again}");
    assert_eq!(lines.next(), Some("first body line"), "{again}");
    assert!(!again.contains("old/ws"), "{again}");
    assert!(!again.contains("outer"), "{again}");

    let text = std::fs::read_to_string(&events).unwrap();
    let event = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|event| event["event"] == "tool_read_unchanged")
        .unwrap_or_else(|| panic!("no tool_read_unchanged in {text}"));
    assert_eq!(event["path"], "file.txt", "{event}");
}

/// N1 of the #966 review: a root that cannot be resolved at all is refused by
/// the public entry in either spelling, with the existing error.
#[test]
fn unresolvable_root_is_refused_in_either_spelling() {
    let dir = tempfile::tempdir().unwrap();
    let gate = dir.path().join("gate");
    let real = gate.join("ws");
    std::fs::create_dir_all(&real).unwrap();
    std::fs::write(real.join("f.txt"), "hidden\n").unwrap();
    let alias = dir.path().join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let restore = RestorePermissions::lock(&gate);

    let probe = std::fs::read_dir(&real);
    let results = [real.as_path(), alias.as_path()].map(|root| {
        ToolRegistry::default()
            .execute("Read", &json!({ "path": "f.txt" }), &context(root, None))
            .map_err(|error| (tool_error_kind(&error), format!("{error:#}")))
    });
    drop(restore);

    assert_eq!(
        probe.as_ref().err().map(|e| e.kind()),
        Some(std::io::ErrorKind::PermissionDenied),
        "this test must exercise actual EACCES; run under an unprivileged account"
    );
    for result in &results {
        let (kind, message) = result
            .as_ref()
            .expect_err("an unresolvable root is refused");
        assert_eq!(*kind, results[0].as_ref().unwrap_err().0);
        assert!(
            message.contains("workspace root is not accessible"),
            "{message}"
        );
        assert!(!message.contains("hidden"), "{message}");
    }
    dir.close().unwrap();
}

/// Makes a directory unreadable and gives it back its own permissions on drop,
/// so a failing assertion still leaves the temporary directory removable.
struct RestorePermissions {
    path: PathBuf,
    original: std::fs::Permissions,
}

impl RestorePermissions {
    fn lock(path: &Path) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let original = std::fs::metadata(path).unwrap().permissions();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
        Self {
            path: path.to_path_buf(),
            original,
        }
    }
}

impl Drop for RestorePermissions {
    fn drop(&mut self) {
        let restored = std::fs::set_permissions(&self.path, self.original.clone());
        if !std::thread::panicking() {
            restored.unwrap();
        }
    }
}
