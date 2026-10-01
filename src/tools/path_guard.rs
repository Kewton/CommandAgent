use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};

mod bash_pattern;

const EXPECTED_PATH_FORM: &str = "use workspace-relative paths";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePathNormalizationKind {
    AbsoluteInsideWorkspace,
    RootAnchorSalvage,
    MissingLeadingSlashRootAnchorSalvage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePathNormalization {
    pub relative: String,
    pub kind: WorkspacePathNormalizationKind,
}

pub fn validate_workspace_relative(raw: &str) -> anyhow::Result<()> {
    if raw.is_empty() {
        bail!("path is empty");
    }
    if raw.as_bytes().contains(&0) {
        bail!("path contains NUL byte; {EXPECTED_PATH_FORM}");
    }
    if looks_like_windows_absolute(raw) {
        bail!("absolute path is not allowed; {EXPECTED_PATH_FORM}");
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        bail!("absolute path is not allowed; {EXPECTED_PATH_FORM}");
    }
    for component in path.components() {
        match component {
            Component::ParentDir => bail!("path may not contain ..; {EXPECTED_PATH_FORM}"),
            Component::Prefix(_) | Component::RootDir => {
                bail!("absolute path is not allowed; {EXPECTED_PATH_FORM}")
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn normalize_absolute_workspace_path(root: &Path, raw: &str) -> anyhow::Result<Option<String>> {
    Ok(normalize_workspace_path(root, raw)?.map(|normalization| normalization.relative))
}

pub fn normalize_workspace_path(
    root: &Path,
    raw: &str,
) -> anyhow::Result<Option<WorkspacePathNormalization>> {
    if raw.as_bytes().contains(&0) {
        bail!("path contains NUL byte; {EXPECTED_PATH_FORM}");
    }
    if looks_like_windows_absolute(raw) {
        bail!("absolute path is not allowed; {EXPECTED_PATH_FORM}");
    }
    let path = Path::new(raw);
    if !path.is_absolute() {
        if looks_like_missing_leading_slash_absolute(raw) {
            let root = root
                .canonicalize()
                .context("workspace root is not accessible")?;
            if let Some(relative) = root_anchor_salvage(&root, path) {
                return Ok(Some(WorkspacePathNormalization {
                    relative,
                    kind: WorkspacePathNormalizationKind::MissingLeadingSlashRootAnchorSalvage,
                }));
            }
            bail!(
                "tool_args_path_malformed: path looks like an absolute path missing its leading slash; {EXPECTED_PATH_FORM}"
            );
        }
        return Ok(None);
    }
    reject_parent_components(path)?;
    let root = root
        .canonicalize()
        .context("workspace root is not accessible")?;
    match canonicalize_with_missing_leaf(path) {
        Ok(canonical) if ensure_inside(&root, &canonical).is_ok() => {
            let relative = canonical
                .strip_prefix(&root)
                .context("path escapes workspace")?;
            let relative = relative.to_string_lossy().replace('\\', "/");
            Ok(Some(WorkspacePathNormalization {
                relative: if relative.is_empty() {
                    ".".to_string()
                } else {
                    relative
                },
                kind: WorkspacePathNormalizationKind::AbsoluteInsideWorkspace,
            }))
        }
        Ok(canonical) => {
            if looks_like_near_root_digit_variance(&root, &canonical)
                || looks_like_near_root_digit_variance(&root, path)
            {
                bail!(
                    "tool_args_path_near_root_corruption: current workspace root `{}`; rejected absolute path `{raw}` because it differs by digit variance; {EXPECTED_PATH_FORM}",
                    root.display()
                );
            }
            if let Some(relative) = root_anchor_salvage(&root, path) {
                Ok(Some(WorkspacePathNormalization {
                    relative,
                    kind: WorkspacePathNormalizationKind::RootAnchorSalvage,
                }))
            } else {
                bail!("path escapes workspace; {EXPECTED_PATH_FORM}")
            }
        }
        Err(err) => {
            if looks_like_near_root_digit_variance(&root, path) {
                bail!(
                    "tool_args_path_near_root_corruption: current workspace root `{}`; rejected absolute path `{raw}` because it differs by digit variance; {EXPECTED_PATH_FORM}",
                    root.display()
                );
            }
            if let Some(relative) = root_anchor_salvage(&root, path) {
                Ok(Some(WorkspacePathNormalization {
                    relative,
                    kind: WorkspacePathNormalizationKind::RootAnchorSalvage,
                }))
            } else {
                Err(err).with_context(|| {
                    format!(
                        "path escapes workspace or parent is not accessible; {EXPECTED_PATH_FORM}"
                    )
                })
            }
        }
    }
}

pub fn normalize_absolute_workspace_glob(root: &Path, raw: &str) -> anyhow::Result<Option<String>> {
    if raw.as_bytes().contains(&0) {
        bail!("path contains NUL byte; {EXPECTED_PATH_FORM}");
    }
    if looks_like_windows_absolute(raw) {
        bail!("absolute path is not allowed; {EXPECTED_PATH_FORM}");
    }
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Ok(None);
    }
    reject_parent_components(path)?;
    let Some((base, suffix)) = split_absolute_glob_base(raw) else {
        return normalize_absolute_workspace_path(root, raw);
    };
    let Some(base_relative) = normalize_absolute_workspace_path(root, base)? else {
        return Ok(None);
    };
    let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
    if base_relative == "." {
        Ok(Some(suffix.to_string()))
    } else if suffix.is_empty() {
        Ok(Some(base_relative))
    } else {
        Ok(Some(format!("{base_relative}/{suffix}")))
    }
}

pub fn resolve_existing(root: &Path, raw: &str) -> anyhow::Result<PathBuf> {
    validate_workspace_relative(raw)?;
    let root = root
        .canonicalize()
        .context("workspace root is not accessible")?;
    let normalized = strip_redundant_root_prefix(&root, raw);
    let candidate = root.join(&normalized);
    let canonical = candidate
        .canonicalize()
        .with_context(|| missing_path_message(&root, raw))?;
    ensure_inside(&root, &canonical)?;
    Ok(canonical)
}

pub fn resolve_for_create(root: &Path, raw: &str) -> anyhow::Result<PathBuf> {
    validate_workspace_relative(raw)?;
    let root = root
        .canonicalize()
        .context("workspace root is not accessible")?;
    let normalized = strip_redundant_root_prefix(&root, raw);
    let candidate = root.join(normalized);
    let existing_parent = nearest_existing_parent(&candidate)?;
    let parent_canonical = existing_parent
        .canonicalize()
        .with_context(|| format!("parent is not accessible: {}", existing_parent.display()))?;
    ensure_inside(&root, &parent_canonical)?;
    Ok(candidate)
}

pub fn resolve_optional_existing(root: &Path, raw: &str) -> anyhow::Result<PathBuf> {
    validate_workspace_relative(raw)?;
    let normalized = strip_redundant_root_prefix(root, raw);
    let candidate = root.join(&normalized);
    if candidate.exists() {
        resolve_existing(root, normalized.to_string_lossy().as_ref())
    } else {
        resolve_for_create(root, normalized.to_string_lossy().as_ref())
    }
}

pub(super) fn ensure_bash_write_target(root: &Path, raw: &str) -> anyhow::Result<()> {
    if raw.starts_with('~') {
        bail!("home-relative Bash write target is outside the workspace contract");
    }
    if raw.contains(['$', '`']) {
        bail!("dynamic Bash write target cannot be proven to remain in the workspace");
    }
    ensure_single_target(root, raw)?;
    // A glob or brace target spells a name that does not exist yet, so the
    // literal proof above stops at the workspace root. Expand the word the way
    // the shell would and prove every result too.
    if bash_pattern::contains_expansion(raw) {
        bash_pattern::ensure_expanded_write_target(root, raw)?;
    }
    Ok(())
}

fn ensure_single_target(root: &Path, raw: &str) -> anyhow::Result<()> {
    let path = Path::new(raw);
    if !path.is_absolute() {
        validate_workspace_relative(raw)?;
    } else {
        reject_parent_components(path)?;
    }

    let root = root
        .canonicalize()
        .context("workspace root is not accessible")?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let resolved = match std::fs::symlink_metadata(&candidate) {
        Ok(_) => candidate
            .canonicalize()
            .with_context(|| format!("Bash write target is not accessible: {raw}"))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = nearest_existing_parent_no_follow(&candidate)?;
            parent.canonicalize().with_context(|| {
                format!(
                    "Bash write target parent is not accessible: {}",
                    parent.display()
                )
            })?
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Bash write target is not accessible: {raw}"));
        }
    };
    ensure_inside(&root, &resolved)
}

pub fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn nearest_existing_parent(path: &Path) -> anyhow::Result<PathBuf> {
    let mut current = path.parent().unwrap_or(path).to_path_buf();
    loop {
        if current.exists() {
            return Ok(current);
        }
        if !current.pop() {
            bail!("no existing parent for {}", path.display());
        }
    }
}

fn nearest_existing_parent_no_follow(path: &Path) -> anyhow::Result<PathBuf> {
    let mut current = path.parent().unwrap_or(path).to_path_buf();
    loop {
        match std::fs::symlink_metadata(&current) {
            Ok(_) => return Ok(current),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("path is not accessible: {}", current.display()));
            }
        }
        if !current.pop() {
            bail!("no existing parent for {}", path.display());
        }
    }
}

fn ensure_inside(root: &Path, candidate: &Path) -> anyhow::Result<()> {
    if !candidate.starts_with(root) {
        bail!("path escapes workspace; {EXPECTED_PATH_FORM}");
    }
    Ok(())
}

fn reject_parent_components(path: &Path) -> anyhow::Result<()> {
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            bail!("path may not contain ..; {EXPECTED_PATH_FORM}");
        }
    }
    Ok(())
}

fn canonicalize_with_missing_leaf(path: &Path) -> anyhow::Result<PathBuf> {
    let mut missing = Vec::new();
    let mut cursor = path;
    while !cursor.exists() {
        let Some(name) = cursor.file_name() else {
            bail!("path escapes workspace or parent is not accessible; {EXPECTED_PATH_FORM}");
        };
        missing.push(name.to_os_string());
        let Some(parent) = cursor.parent() else {
            bail!("path escapes workspace or parent is not accessible; {EXPECTED_PATH_FORM}");
        };
        cursor = parent;
    }
    let mut resolved = cursor.canonicalize().with_context(|| {
        format!("path escapes workspace or parent is not accessible; {EXPECTED_PATH_FORM}")
    })?;
    for component in missing.iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn split_absolute_glob_base(raw: &str) -> Option<(&str, &str)> {
    let meta = raw.find(['*', '?', '[', '{'])?;
    let split = raw[..meta].rfind('/')?;
    if split == 0 {
        Some(("/", &raw[split + 1..]))
    } else {
        Some((&raw[..split], &raw[split + 1..]))
    }
}

pub(super) fn strip_redundant_root_prefix(root: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    let Some(root_name) = root.file_name() else {
        return path.to_path_buf();
    };
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return path.to_path_buf();
    };
    if first != root_name {
        return path.to_path_buf();
    }
    let stripped = components.as_path();
    if stripped.as_os_str().is_empty() {
        path.to_path_buf()
    } else {
        stripped.to_path_buf()
    }
}

fn root_anchor_salvage(root: &Path, raw: &Path) -> Option<String> {
    let root_components = normal_components(root);
    let raw_components = normal_components(raw);
    if root_components.is_empty() || raw_components.is_empty() {
        return None;
    }
    let anchor_len = root_components.len().min(2);
    let anchor = &root_components[root_components.len() - anchor_len..];
    let mut matches = raw_components
        .windows(anchor_len)
        .enumerate()
        .filter_map(|(index, window)| (window == anchor).then_some(index))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return None;
    }
    let index = matches.pop()?;
    let tail = &raw_components[index + anchor_len..];
    let relative = if tail.is_empty() {
        ".".to_string()
    } else {
        tail.join("/")
    };
    if validate_workspace_relative(&relative).is_err() {
        return None;
    }
    Some(relative)
}

fn normal_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect()
}

fn missing_path_message(root: &Path, raw: &str) -> String {
    if let Some(candidate) = missing_path_candidate(root, raw) {
        format!(
            "path_not_found_recoverable: path does not exist: {raw}; did you mean `{candidate}`?"
        )
    } else {
        format!("path does not exist: {raw}")
    }
}

fn missing_path_candidate(root: &Path, raw: &str) -> Option<String> {
    let path = Path::new(raw);
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return None;
    };
    if first != "workdir" {
        return None;
    }
    let tail = components.as_path();
    if tail.as_os_str().is_empty() {
        return None;
    }
    let candidate = root.join(tail);
    if !candidate.exists() {
        return None;
    }
    Some(tail.to_string_lossy().replace('\\', "/"))
}

fn looks_like_windows_absolute(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    raw.starts_with("\\\\")
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/'))
}

fn looks_like_missing_leading_slash_absolute(raw: &str) -> bool {
    let path = Path::new(raw);
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return false;
    };
    let first = first.to_string_lossy();
    matches!(first.as_ref(), "Users" | "home") && components.next().is_some()
}

fn looks_like_near_root_digit_variance(root: &Path, raw: &Path) -> bool {
    let root_components = normal_components(root);
    let raw_components = normal_components(raw);
    near_root_digit_variance_components(&root_components, &raw_components)
        || (root_components
            .first()
            .is_some_and(|component| component == "private")
            && near_root_digit_variance_components(&root_components[1..], &raw_components))
}

fn near_root_digit_variance_components(
    root_components: &[String],
    raw_components: &[String],
) -> bool {
    if raw_components.len() < root_components.len() || root_components.is_empty() {
        return false;
    }
    let mut digit_variance_components = 0usize;
    for (root, raw) in root_components.iter().zip(raw_components.iter()) {
        if root == raw {
            continue;
        }
        if same_shape_with_digit_variance(root, raw) {
            digit_variance_components += 1;
        } else {
            return false;
        }
    }
    digit_variance_components == 1
}

fn same_shape_with_digit_variance(left: &str, right: &str) -> bool {
    let mut saw_digit_difference = false;
    let mut left_chars = left.chars();
    let mut right_chars = right.chars();
    loop {
        match (left_chars.next(), right_chars.next()) {
            (Some(left), Some(right)) if left == right => {}
            (Some(left), Some(right)) if left.is_ascii_digit() && right.is_ascii_digit() => {
                saw_digit_difference = true;
            }
            (None, None) => return saw_digit_difference,
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_escape_paths() {
        for raw in [
            "/etc/passwd",
            "../secret",
            "a/../../secret",
            "C:\\Users\\x",
            "\\\\server\\share",
        ] {
            assert!(validate_workspace_relative(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn allows_missing_child_under_existing_parent() {
        let dir = tempfile::tempdir().unwrap();
        let path = resolve_for_create(dir.path(), "notes/new.md").unwrap();
        assert!(path.ends_with("notes/new.md"));
    }

    #[test]
    fn strips_redundant_workspace_root_prefix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "ok").unwrap();
        let raw = format!(
            "{}/a.txt",
            dir.path().file_name().unwrap().to_string_lossy()
        );
        let path = resolve_existing(dir.path(), &raw).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "ok");
    }

    #[test]
    fn salvages_absolute_path_with_unique_root_anchor() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0708_013");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("package.json"), "{}").unwrap();
        let raw = dir
            .path()
            .join("share/work/commandagent_mvp/01/test0708_013/package.json");
        let normalization = normalize_workspace_path(&root, raw.to_str().unwrap())
            .unwrap()
            .expect("salvaged");

        assert_eq!(normalization.relative, "package.json");
        assert_eq!(
            normalization.kind,
            WorkspacePathNormalizationKind::RootAnchorSalvage
        );
        let path = resolve_existing(&root, &normalization.relative).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{}");
    }

    #[test]
    fn salvages_missing_leading_slash_path_with_unique_root_anchor() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0709_camp_003");
        std::fs::create_dir_all(&root).unwrap();
        let raw =
            "Users/maenokota/share/work/localwork/commandagent_mvp/01/test0709_camp_003/src/app/page.tsx"
                .to_string();

        let normalization = normalize_workspace_path(&root, &raw).unwrap().unwrap();

        assert_eq!(normalization.relative, "src/app/page.tsx");
        assert_eq!(
            normalization.kind,
            WorkspacePathNormalizationKind::MissingLeadingSlashRootAnchorSalvage
        );
    }

    #[test]
    fn rejects_missing_leading_slash_path_without_unique_root_anchor_as_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0709_camp_003");
        std::fs::create_dir_all(&root).unwrap();

        let err = normalize_workspace_path(&root, "Users/maenokota/other/src/App.js").unwrap_err();

        assert!(
            err.to_string().contains("tool_args_path_malformed"),
            "{err}"
        );
    }

    #[test]
    fn rejects_current_root_digit_variance_without_cross_workspace_salvage() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0710_camp_002");
        std::fs::create_dir_all(&root).unwrap();
        let raw = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0710_camp_001/src/app/page.tsx");

        let err = normalize_workspace_path(&root, raw.to_str().unwrap()).unwrap_err();

        assert!(
            err.to_string()
                .contains("tool_args_path_near_root_corruption"),
            "{err}"
        );
        assert!(err.to_string().contains(root.to_str().unwrap()), "{err}");
    }

    #[test]
    fn root_anchor_salvage_requires_unique_anchor() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0708_013");
        std::fs::create_dir_all(&root).unwrap();
        let raw = dir
            .path()
            .join("share/01/test0708_013/archive/01/test0708_013/package.json");

        let err = normalize_workspace_path(&root, raw.to_str().unwrap()).unwrap_err();

        assert!(err.to_string().contains("path escapes workspace"), "{err}");
    }

    #[test]
    fn suggests_workdir_prefix_without_silent_normalization() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "ok").unwrap();
        let err = resolve_existing(dir.path(), "workdir/a.txt").unwrap_err();
        assert!(err.to_string().contains("path_not_found_recoverable"));
        assert!(err.to_string().contains("did you mean `a.txt`"));
    }

    #[test]
    fn rejects_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/tmp", dir.path().join("out")).unwrap();
            assert!(resolve_existing(dir.path(), "out").is_err());
        }
    }

    #[test]
    fn validate_workspace_relative_table() {
        assert!(validate_workspace_relative("").is_err());
        assert!(validate_workspace_relative("a\0b").is_err());
        assert!(validate_workspace_relative("a/b.txt").is_ok());
        assert!(validate_workspace_relative("C:x").is_ok());
    }

    #[test]
    fn normalize_absolute_workspace_path_table() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "ok").unwrap();

        let existing = root.join("a.txt");
        assert_eq!(
            normalize_absolute_workspace_path(&root, existing.to_str().unwrap())
                .unwrap()
                .as_deref(),
            Some("a.txt")
        );

        assert_eq!(
            normalize_absolute_workspace_path(&root, root.to_str().unwrap())
                .unwrap()
                .as_deref(),
            Some(".")
        );

        let missing = root.join("new/deep/f");
        assert_eq!(
            normalize_absolute_workspace_path(&root, missing.to_str().unwrap())
                .unwrap()
                .as_deref(),
            Some("new/deep/f")
        );

        assert!(
            normalize_absolute_workspace_path(&root, "a.txt")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn normalize_workspace_path_table() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "ok").unwrap();

        let inside = root.join("a.txt");
        let normalization = normalize_workspace_path(&root, inside.to_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(normalization.relative, "a.txt");
        assert_eq!(
            normalization.kind,
            WorkspacePathNormalizationKind::AbsoluteInsideWorkspace
        );

        for raw in ["C:\\x", "C:/x", "\\\\srv\\s"] {
            assert!(normalize_workspace_path(&root, raw).is_err(), "{raw}");
        }
        for raw in ["1:/x", "ab/c", "C:x", "a.txt"] {
            assert!(
                normalize_workspace_path(&root, raw).unwrap().is_none(),
                "{raw}"
            );
        }

        let parent = root.join("../x");
        assert!(normalize_workspace_path(&root, parent.to_str().unwrap()).is_err());
        assert!(normalize_workspace_path(&root, "a\0b").is_err());
        assert!(normalize_workspace_path(&root, "/etc/hosts").is_err());
    }

    #[test]
    fn near_root_digit_variance_table() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let root = base.join("work/camp_002");
        std::fs::create_dir_all(&root).unwrap();

        let near = base.join("work/camp_001/src/app/page.tsx");
        let err = normalize_workspace_path(&root, near.to_str().unwrap()).unwrap_err();
        assert!(
            err.to_string()
                .contains("tool_args_path_near_root_corruption"),
            "{err}"
        );

        let same_length = base.join("work/camp_001");
        let err = normalize_workspace_path(&root, same_length.to_str().unwrap()).unwrap_err();
        assert!(
            err.to_string()
                .contains("tool_args_path_near_root_corruption"),
            "{err}"
        );

        let two_root = base.join("work_002/camp_002");
        std::fs::create_dir_all(&two_root).unwrap();
        let two = base.join("work_001/camp_001/src/app/page.tsx");
        for raw in [
            base.join("work/camp_0002/src/app/page.tsx"),
            base.join("work/camp_00x/src/app/page.tsx"),
            base.join("work/other_002/src/app/page.tsx"),
            base.clone(),
        ] {
            let err = normalize_workspace_path(&root, raw.to_str().unwrap()).unwrap_err();
            assert!(
                err.to_string().contains("path escapes workspace"),
                "{raw:?}: {err}"
            );
        }
        let err = normalize_workspace_path(&two_root, two.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("path escapes workspace"), "{err}");

        let short_root = base.join("work/camp_002/sub");
        std::fs::create_dir_all(&short_root).unwrap();
        let short_raw = base.join("work/camp_001");
        let err = normalize_workspace_path(&short_root, short_raw.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("path escapes workspace"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn near_root_digit_variance_uses_raw_path_when_canonical_differs() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let root = base.join("work/camp_002");
        std::fs::create_dir_all(&root).unwrap();
        let outside = base.join("outside_dir");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("work/camp_001")).unwrap();

        let raw = base.join("work/camp_001");
        let err = normalize_workspace_path(&root, raw.to_str().unwrap()).unwrap_err();
        assert!(
            err.to_string()
                .contains("tool_args_path_near_root_corruption"),
            "{err}"
        );
    }

    #[test]
    fn looks_like_near_root_digit_variance_table() {
        assert!(looks_like_near_root_digit_variance(
            Path::new("/private/work/camp_002"),
            Path::new("/work/camp_001/src/app/page.tsx"),
        ));
        assert!(looks_like_near_root_digit_variance(
            Path::new("/work/camp_002"),
            Path::new("/work/camp_001/x"),
        ));
        assert!(!looks_like_near_root_digit_variance(
            Path::new("/a/camp_002"),
            Path::new("/camp_001/x"),
        ));
    }

    #[test]
    fn split_absolute_glob_base_table() {
        assert_eq!(
            split_absolute_glob_base("/ws/*.txt"),
            Some(("/ws", "*.txt"))
        );
        assert_eq!(
            split_absolute_glob_base("/ws/src/*.rs"),
            Some(("/ws/src", "*.rs"))
        );
        assert_eq!(
            split_absolute_glob_base("/ws/src/**/?.rs"),
            Some(("/ws/src", "**/?.rs"))
        );
        assert_eq!(split_absolute_glob_base("/*.txt"), Some(("/", "*.txt")));
        assert_eq!(split_absolute_glob_base("/ws/src"), None);
    }

    #[test]
    fn missing_leading_slash_table() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let root = base.join("ws_001");
        std::fs::create_dir_all(&root).unwrap();

        for raw in ["Users/x/y", "home/x/y"] {
            let err = normalize_workspace_path(&root, raw).unwrap_err();
            assert!(
                err.to_string().contains("tool_args_path_malformed"),
                "{raw}: {err}"
            );
        }
        for raw in ["Users", "Usersx/y", "src/Users/x"] {
            assert!(
                normalize_workspace_path(&root, raw).unwrap().is_none(),
                "{raw}"
            );
        }
    }

    #[test]
    fn root_anchor_salvage_on_filesystem_root_does_not_panic() {
        let err = normalize_workspace_path(Path::new("/"), "Users/x/y").unwrap_err();
        assert!(
            err.to_string().contains("tool_args_path_malformed"),
            "{err}"
        );
    }

    #[test]
    fn normalize_absolute_workspace_glob_table() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        let r = root.to_str().unwrap().to_string();

        for (raw, expected) in [
            (format!("{r}/*.txt"), "*.txt"),
            (format!("{r}/src/*.rs"), "src/*.rs"),
            (format!("{r}/src/**/?.rs"), "src/**/?.rs"),
            (format!("{r}/src"), "src"),
        ] {
            assert_eq!(
                normalize_absolute_workspace_glob(&root, &raw)
                    .unwrap()
                    .as_deref(),
                Some(expected),
                "{raw}"
            );
        }

        assert!(
            normalize_absolute_workspace_glob(&root, "src/*.rs")
                .unwrap()
                .is_none()
        );
        assert!(normalize_absolute_workspace_glob(&root, "/etc/*.conf").is_err());
        assert!(normalize_absolute_workspace_glob(&root, "/*.txt").is_err());
        assert!(normalize_absolute_workspace_glob(&root, &format!("{r}/../x/*.txt")).is_err());
        assert!(normalize_absolute_workspace_glob(&root, "C:\\*.txt").is_err());
        assert!(normalize_absolute_workspace_glob(&root, "a\0*.txt").is_err());
    }

    #[test]
    fn resolve_optional_existing_and_relative_display_table() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "ok").unwrap();

        let existing = resolve_optional_existing(&root, "a.txt").unwrap();
        assert!(existing.ends_with("a.txt"), "{existing:?}");
        let created = resolve_optional_existing(&root, "new/deep/f.txt").unwrap();
        assert!(created.ends_with("new/deep/f.txt"), "{created:?}");

        assert_eq!(relative_display(&root, &root.join("a/b.txt")), "a/b.txt");
        assert_eq!(
            relative_display(&root, Path::new("/outside/x.txt")),
            "/outside/x.txt"
        );
    }

    #[test]
    fn strip_redundant_root_prefix_table() {
        let root = Path::new("/tmp/ws");
        assert_eq!(
            strip_redundant_root_prefix(root, "ws/a/b"),
            PathBuf::from("a/b")
        );
        assert_eq!(strip_redundant_root_prefix(root, "ws"), PathBuf::from("ws"));
        assert_eq!(
            strip_redundant_root_prefix(root, "other/a"),
            PathBuf::from("other/a")
        );
        assert_eq!(
            strip_redundant_root_prefix(Path::new("/"), "a/b"),
            PathBuf::from("a/b")
        );
    }

    #[test]
    fn ensure_bash_write_target_table() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "ok").unwrap();

        assert!(ensure_bash_write_target(&root, "a.txt").is_ok());
        assert!(ensure_bash_write_target(&root, "new/deep/f.txt").is_ok());
        let abs_inside = root.join("a.txt");
        assert!(ensure_bash_write_target(&root, abs_inside.to_str().unwrap()).is_ok());

        assert!(ensure_bash_write_target(&root, "/etc/hosts").is_err());
        assert!(ensure_bash_write_target(&root, "../x").is_err());
        assert!(ensure_bash_write_target(&root, "~/x").is_err());
        assert!(ensure_bash_write_target(&root, "$HOME/x").is_err());
        assert!(ensure_bash_write_target(&root, "`whoami`/x").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn ensure_bash_write_target_rejects_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("f.txt"), "x").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();

        assert!(ensure_bash_write_target(&root, "linked/f.txt").is_err());
        assert!(ensure_bash_write_target(&root, "linked/new.txt").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn ensure_bash_write_target_rejects_unreadable_parent() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let locked = root.join("locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

        if std::fs::read_dir(&locked).is_ok() {
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }

        let result = ensure_bash_write_target(&root, "locked/f.txt");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn ensure_bash_write_target_expansion_table() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/f"), "x").unwrap();
        std::fs::create_dir_all(dir.path().join("outside")).unwrap();
        std::fs::write(dir.path().join("outside/secret"), "x").unwrap();
        std::os::unix::fs::symlink(dir.path().join("outside"), root.join("linked-outside"))
            .unwrap();
        std::os::unix::fs::symlink(dir.path().join("outside"), root.join(".hidden-out")).unwrap();
        let root = root.canonicalize().unwrap();

        for raw in [
            "lin*/secret",
            "linked-outsid?/secret",
            "linked-outsid[e]/secret",
            "*/secret",
            "**/secret",
            ".hid*/f",
            ".*/f",
            ".?/f",
            "{.,}./f",
            "{.,.}./f",
            "{linked-outside,x}/f",
            "{k..m}inked-outside/f",
            "{linked-outside/f,x}",
        ] {
            assert!(ensure_bash_write_target(&root, raw).is_err(), "{raw}");
        }

        for raw in ["sub/f", "nomatch*/f", "src/new.rs", "sub/{a,b}.txt"] {
            assert!(ensure_bash_write_target(&root, raw).is_ok(), "{raw}");
        }
    }
}
