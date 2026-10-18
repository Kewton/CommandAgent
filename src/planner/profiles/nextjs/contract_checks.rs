//! Individual Next.js contract checks shared by the final verifier and the
//! invariant verifier. Each check returns the failure reason, already shaped
//! the way the verifier reports it; the verifiers keep the checks that exist in
//! only one mode (package.json reads, entrypoint, layout, `@/` alias).

use std::path::Path;

use serde_json::Value;

use super::{
    ProjectRoot, api_contract, client_component_contract_failure,
    css_side_effect_import_contract_failure, dependency_family, is_weakened_script,
    missing_app_relative_import_contract_failure, response_shape, scaffold_mode,
    script_runs_next_dev_on_port, script_runs_next_start_on_port, tailwind_contract_failure,
    tsconfig_contract_failure,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    /// The workspace is claimed complete: build and dev scripts are required.
    Final,
    /// The workspace may still be unfinished: absent build/dev scripts are allowed.
    Invariant,
}

pub(super) fn dependency_failure(project_path: &Path, package: &Value) -> Option<String> {
    let deps = package.get("dependencies").and_then(Value::as_object);
    for dep in ["next", "react", "react-dom"] {
        if deps.is_none_or(|deps| !deps.contains_key(dep)) {
            return Some(format!("dependency missing: {dep}"));
        }
    }
    let typescript_required = scaffold_mode::detect(project_path).uses_typescript();
    dependency_family::coherence_failure(package, typescript_required)
}

pub(super) fn scripts_failure(package: &Value, port: u16, mode: Mode) -> Option<String> {
    let scripts = package.get("scripts").and_then(Value::as_object);
    let script = |name: &str| {
        scripts
            .and_then(|scripts| scripts.get(name))
            .and_then(Value::as_str)
    };
    let build = script("build");
    let build_ok = match mode {
        Mode::Final => build == Some("next build") && !build.is_some_and(is_weakened_script),
        Mode::Invariant => {
            !build.is_some_and(|build| build != "next build" || is_weakened_script(build))
        }
    };
    if !build_ok {
        return Some("scripts.build must be next build".to_string());
    }
    let dev = script("dev");
    if mode == Mode::Final && dev.is_some_and(is_weakened_script) {
        return Some("scripts.dev must run next dev".to_string());
    }
    let dev = dev.unwrap_or("");
    let dev_required = mode == Mode::Final || !dev.is_empty();
    if dev_required && !script_runs_next_dev_on_port(dev, port) {
        return Some(format!("dev script must run next dev on port {port}"));
    }
    let start = script("start").unwrap_or("");
    if !start.is_empty() && !script_runs_next_start_on_port(start, port) {
        return Some(format!("start script must run next start on port {port}"));
    }
    None
}

/// The source-level contract checks that follow the scripts and the
/// mode-specific entrypoint checks, in reporting order.
pub(super) fn source_failure(project: &ProjectRoot, package: &Value, mode: Mode) -> Option<String> {
    if let Some(reason) = tsconfig_contract_failure(&project.path) {
        return Some(reason);
    }
    if let Some(reason) = css_side_effect_import_contract_failure(&project.path) {
        return Some(reason);
    }
    if let Some(reason) = missing_app_relative_import_contract_failure(&project.path) {
        return Some(project.rel_path(&reason));
    }
    let api = api_contract::failure(&project.path);
    let api = match mode {
        Mode::Final => api.or_else(|| response_shape::request_failure(&project.path)),
        Mode::Invariant => api,
    };
    if let Some(reason) = api {
        return Some(project.rel_path(&reason));
    }
    if let Some(reason) = client_component_contract_failure(&project.path) {
        return Some(reason);
    }
    tailwind_contract_failure(&project.path, package)
}
