use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{Arguments, delegate, normalize_base_path, trial_access, workspace_policy};

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
    remediation: Option<&'static str>,
}

impl Report {
    pub fn run(arguments: &Arguments) -> Self {
        let mut checks = Vec::new();
        checks.push(check_static_export(arguments));
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
            if let Some(remediation) = check.remediation {
                println!("  fix: {remediation}");
            }
        }
        println!("preflight: {}", self.status);
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

fn fail(id: &'static str, detail: impl Into<String>, remediation: &'static str) -> Check {
    Check {
        id,
        status: "ng",
        detail: detail.into(),
        remediation: Some(remediation),
    }
}

fn check_static_export(arguments: &Arguments) -> Check {
    let base_path = match normalize_base_path(&arguments.base_path) {
        Ok(path) => path,
        Err(error) => {
            return fail(
                "static.base_path",
                error.to_string(),
                "use '/' or an absolute path without a trailing slash",
            );
        }
    };
    let index = arguments.static_dir.join("index.html");
    let text = match std::fs::read_to_string(&index) {
        Ok(text) => text,
        Err(error) => {
            return fail(
                "static.base_path",
                format!("cannot read {}: {error}", index.display()),
                "build the GUI export with the same GUI_BASE_PATH",
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
            "rebuild with GUI_BASE_PATH matching --base-path",
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
            "choose repository, execution, and extension roots with no overlap",
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
    match path.canonicalize() {
        Ok(canonical) if canonical.is_dir() => {
            if let Some(detail) = permission_error(&canonical, private) {
                checks.push(fail(
                    id,
                    detail,
                    "grant the owner rwx access and keep extension roots private (0700)",
                ));
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
                "provide an existing accessible directory",
            ));
            None
        }
        Err(error) => {
            checks.push(fail(
                id,
                format!("{}: {error}", path.display()),
                "create the directory and verify its permissions",
            ));
            None
        }
    }
}

#[cfg(unix)]
fn permission_error(path: &Path, private: bool) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = std::fs::metadata(path).ok()?.permissions().mode() & 0o777;
    if mode & 0o700 != 0o700 {
        return Some(format!(
            "{} owner permissions are {:03o}, expected rwx",
            path.display(),
            mode
        ));
    }
    if private && mode & 0o077 != 0 {
        return Some(format!(
            "{} permissions are {:03o}, expected private 700",
            path.display(),
            mode
        ));
    }
    None
}

#[cfg(not(unix))]
fn permission_error(path: &Path, _: bool) -> Option<String> {
    std::fs::metadata(path)
        .ok()
        .filter(|metadata| metadata.permissions().readonly())
        .map(|_| format!("{} is read-only", path.display()))
}

fn check_binary(arguments: &Arguments, repository: Option<&Path>) -> Check {
    let path = if arguments.commandagent_bin.is_absolute() {
        arguments.commandagent_bin.clone()
    } else if let Some(repository) = repository {
        repository.join(&arguments.commandagent_bin)
    } else {
        arguments.commandagent_bin.clone()
    };
    match delegate::check_binary(&path) {
        Ok(version) => pass("binary.version", version),
        Err(error) => fail(
            "binary.version",
            error.to_string(),
            "build commandagent and pass its executable path with --commandagent-bin",
        ),
    }
}

fn check_trial_access(arguments: &Arguments) -> Check {
    match trial_access::TrialAccess::validate_environment(arguments.trial_token_auth.is_enabled()) {
        Ok(()) => pass("trial.access", "token and allowed origins are valid"),
        Err(error) => fail(
            "trial.access",
            error.to_string(),
            "set a valid GUI_TRIAL_TOKEN and GUI_TRIAL_ALLOWED_ORIGINS",
        ),
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
