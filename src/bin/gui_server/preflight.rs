use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{
    Arguments, delegate, gui_contract, normalize_base_path, resolve_commandagent_bin, trial_access,
    workspace_policy,
};

#[derive(Debug, Serialize)]
pub struct Report {
    status: &'static str,
    checks: Vec<Check>,
}

#[derive(Debug, Serialize)]
struct Check {
    id: &'static str,
    status: &'static str,
    detail: String,
    remediation: Option<String>,
}

impl Report {
    pub fn run(arguments: &Arguments) -> Self {
        let mut checks = Vec::new();
        checks.push(check_static_export(arguments));
        checks.push(check_static_contract(arguments));
        let roots = checked_roots(arguments);
        checks.extend(roots.checks);
        checks.push(check_binary(arguments, roots.repository.as_deref()));
        checks.push(check_trial_access(arguments));
        let status = if checks.iter().all(|check| check.status == "ok") {
            "ok"
        } else {
            "ng"
        };
        Self { status, checks }
    }

    pub fn passed(&self) -> bool {
        self.status == "ok"
    }

    pub fn print_human(&self) {
        for check in &self.checks {
            println!("{}: {}: {}", check.status, check.id, check.detail);
            if let Some(remediation) = &check.remediation {
                println!("  fix: {remediation}");
            }
        }
        println!("preflight: {}", self.status);
    }
}

fn check_static_contract(arguments: &Arguments) -> Check {
    let expected = gui_contract::server_contract_version();
    match gui_contract::export_contract_version(&arguments.static_dir) {
        Ok(observed) if observed == expected => pass(
            "static.contract_version",
            format!("GUI export and gui_server use contract {expected}"),
        ),
        Ok(observed) => fail(
            "static.contract_version",
            format!("GUI export contract {observed} does not match gui_server contract {expected}"),
            "rebuild both artifacts from the same checkout with `cd gui && npm run build`, then `cargo build --features gui --bin gui_server`",
        ),
        Err(error) => fail(
            "static.contract_version",
            error.to_string(),
            "rebuild the static export with `cd gui && npm run build`, then rerun gui_server --check",
        ),
    }
}

struct CheckedRoots {
    repository: Option<PathBuf>,
    checks: Vec<Check>,
}

fn pass(id: &'static str, detail: impl Into<String>) -> Check {
    Check {
        id,
        status: "ok",
        detail: detail.into(),
        remediation: None,
    }
}

fn fail(id: &'static str, detail: impl Into<String>, remediation: impl Into<String>) -> Check {
    Check {
        id,
        status: "ng",
        detail: detail.into(),
        remediation: Some(remediation.into()),
    }
}

fn check_static_export(arguments: &Arguments) -> Check {
    let base_path = match normalize_base_path(&arguments.base_path) {
        Ok(path) => path,
        Err(error) => {
            return fail(
                "static.base_path",
                error.to_string(),
                format!(
                    "replace --base-path {:?} with '/' or an absolute path without a trailing slash",
                    arguments.base_path
                ),
            );
        }
    };
    let index = arguments.static_dir.join("index.html");
    let text = match std::fs::read_to_string(&index) {
        Ok(text) => text,
        Err(error) => {
            let remediation = match error.kind() {
                std::io::ErrorKind::NotFound => format!(
                    "create the missing export with `cd gui && GUI_BASE_PATH={} npm run build`, or pass the directory containing index.html with --static-dir",
                    base_path
                ),
                std::io::ErrorKind::PermissionDenied => format!(
                    "grant the current user read access to {}, or pass a readable export with --static-dir",
                    index.display()
                ),
                std::io::ErrorKind::InvalidData => format!(
                    "rebuild {} as a UTF-8 GUI export with `cd gui && GUI_BASE_PATH={} npm run build`",
                    index.display(),
                    base_path
                ),
                _ => format!(
                    "resolve the reported I/O error for {}, then rerun --check",
                    index.display()
                ),
            };
            return fail(
                "static.base_path",
                format!("cannot read {}: {error}", index.display()),
                remediation,
            );
        }
    };
    let expected = if base_path == "/" {
        "/_next/".to_string()
    } else {
        format!("{base_path}/_next/")
    };
    let matches_export = ["\"", "'", "\\\""]
        .iter()
        .any(|quote| text.contains(&format!("{quote}{expected}")));
    if matches_export {
        pass(
            "static.base_path",
            format!("{} matches export {}", base_path, index.display()),
        )
    } else {
        fail(
            "static.base_path",
            format!(
                "{} does not contain assets for base path {}",
                index.display(),
                base_path
            ),
            format!(
                "rebuild the export with `cd gui && GUI_BASE_PATH={} npm run build` so it matches --base-path",
                base_path
            ),
        )
    }
}

fn checked_roots(arguments: &Arguments) -> CheckedRoots {
    let mut checks = Vec::new();
    let repository = canonical_directory(
        "roots.repository",
        &arguments.repository_root,
        false,
        &mut checks,
    );
    let execution = arguments
        .execution_root
        .as_deref()
        .and_then(|path| canonical_directory("roots.execution", path, false, &mut checks));
    if arguments.execution_root.is_none() {
        checks.push(pass("roots.execution", "not configured"));
    }
    let extension = arguments
        .extension_root
        .as_deref()
        .and_then(|path| canonical_directory("roots.extension", path, true, &mut checks));
    if arguments.extension_root.is_none() {
        checks.push(pass("roots.extension", "not configured"));
    }

    let roots = [
        ("repository", repository.as_deref()),
        ("execution", execution.as_deref()),
        ("extension", extension.as_deref()),
    ];
    let mut overlap = None;
    for left in 0..roots.len() {
        for right in (left + 1)..roots.len() {
            if let (Some(left_path), Some(right_path)) = (roots[left].1, roots[right].1)
                && let Err(error) = workspace_policy::ensure_disjoint(left_path, right_path)
            {
                overlap = Some(format!("{} and {}: {error}", roots[left].0, roots[right].0));
            }
        }
    }
    checks.push(match overlap {
        Some(detail) => fail(
            "roots.disjoint",
            detail,
            "move one of the named roots so repository, execution, and extension roots are pairwise disjoint",
        ),
        None => pass("roots.disjoint", "configured roots are pairwise disjoint"),
    });
    CheckedRoots { repository, checks }
}

fn canonical_directory(
    id: &'static str,
    path: &Path,
    private: bool,
    checks: &mut Vec<Check>,
) -> Option<PathBuf> {
    if private
        && std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        checks.push(fail(
            id,
            format!("{} is a symlink", path.display()),
            format!(
                "replace --extension-root {} with a private directory that is not a symlink",
                path.display()
            ),
        ));
        return None;
    }
    match path.canonicalize() {
        Ok(canonical) if canonical.is_dir() => {
            if let Some(failure) = permission_failure(&canonical, private) {
                checks.push(fail(id, failure.detail, failure.remediation));
                None
            } else {
                checks.push(pass(id, canonical.display().to_string()));
                Some(canonical)
            }
        }
        Ok(canonical) => {
            checks.push(fail(
                id,
                format!("{} is not a directory", canonical.display()),
                format!("replace {} with an existing directory path", path.display()),
            ));
            None
        }
        Err(error) => {
            let remediation = match error.kind() {
                std::io::ErrorKind::NotFound => {
                    format!("create the missing directory {}", path.display())
                }
                std::io::ErrorKind::PermissionDenied => format!(
                    "grant the current user search access to {} and its parent directories",
                    path.display()
                ),
                _ => format!(
                    "resolve the reported path error for {}, then rerun --check",
                    path.display()
                ),
            };
            checks.push(fail(
                id,
                format!("{}: {error}", path.display()),
                remediation,
            ));
            None
        }
    }
}

struct PermissionFailure {
    detail: String,
    remediation: String,
}

#[cfg(unix)]
fn permission_failure(path: &Path, private: bool) -> Option<PermissionFailure> {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::metadata(path).ok()?;
    // SAFETY: `geteuid` has no preconditions and does not dereference memory.
    let effective_uid = unsafe { libc::geteuid() };
    if private && metadata.uid() != effective_uid {
        return Some(PermissionFailure {
            detail: format!(
                "{} owner uid is {}, expected effective uid {effective_uid}",
                path.display(),
                metadata.uid()
            ),
            remediation: format!(
                "use an extension directory owned by effective uid {effective_uid}, then set it to mode 700"
            ),
        });
    }
    let mode = metadata.permissions().mode() & 0o777;
    if private && mode & 0o077 != 0 {
        return Some(PermissionFailure {
            detail: format!(
                "{} permissions are {:03o}; group/other permissions must be removed",
                path.display(),
                mode
            ),
            remediation: format!(
                "remove group/other access with `chmod 700 {}`",
                path.display()
            ),
        });
    }
    if mode & 0o700 != 0o700 {
        let remediation = if private {
            format!(
                "grant only the owner rwx access with `chmod 700 {}`",
                path.display()
            )
        } else {
            format!(
                "grant the owner rwx access with `chmod u+rwx {}`",
                path.display()
            )
        };
        return Some(PermissionFailure {
            detail: format!(
                "{} owner permissions are {:03o}, expected rwx",
                path.display(),
                mode
            ),
            remediation,
        });
    }
    None
}

#[cfg(not(unix))]
fn permission_failure(path: &Path, _: bool) -> Option<PermissionFailure> {
    std::fs::metadata(path)
        .ok()
        .filter(|metadata| metadata.permissions().readonly())
        .map(|_| PermissionFailure {
            detail: format!("{} is read-only", path.display()),
            remediation: format!(
                "clear the read-only attribute for {} so the owner can write to it",
                path.display()
            ),
        })
}

fn check_binary(arguments: &Arguments, repository: Option<&Path>) -> Check {
    let repository = repository.unwrap_or(&arguments.repository_root);
    let path = resolve_commandagent_bin(arguments, repository);
    match delegate::check_binary(&path) {
        Ok(version) => pass("binary.version", version),
        Err(error) => fail(
            "binary.version",
            error.to_string(),
            format!(
                "build an executable commandagent at {}, or pass its exact path with --commandagent-bin",
                path.display()
            ),
        ),
    }
}

fn check_trial_access(arguments: &Arguments) -> Check {
    match trial_access::TrialAccess::validate_environment(arguments.trial_token_auth.is_enabled()) {
        Ok(()) => pass("trial.access", "token and allowed origins are valid"),
        Err(error) => {
            let detail = error.to_string();
            let remediation = if detail.contains("GUI_TRIAL_TOKEN") {
                "set GUI_TRIAL_TOKEN to 32..=4096 non-whitespace characters"
            } else {
                "set GUI_TRIAL_ALLOWED_ORIGINS to comma-separated http/https origins with hosts and no paths or queries"
            };
            fail("trial.access", detail, remediation)
        }
    }
}

pub fn count_packs(root: Option<&Path>) -> usize {
    root.map_or(0, |root| count_pack_pins(&root.join("packs"), 0))
}

fn count_pack_pins(directory: &Path, depth: usize) -> usize {
    if depth > 3 {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_file() && entry.file_name() == "pack.sha256" => 1,
            Ok(kind) if kind.is_dir() => count_pack_pins(&entry.path(), depth + 1),
            _ => 0,
        })
        .sum()
}
