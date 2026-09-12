mod compound_hook;

/// These exact scaffold assertions duplicate verify/verify_invariant's
/// package-script checks. Keep executing them at the step boundary; the final
/// profile gate enforces the same constraints without treating configuration
/// probes as additional business-success observations. Never exempt arbitrary
/// Node commands or assertions about a different requested port.
pub(crate) fn final_verifier_covers_command(goal: &str, command: &str) -> bool {
    command == super::package_build_script_verify_command()
        || super::package_script_port_verify_commands(super::requested_or_default_port(goal))
            .iter()
            .any(|check| check == command)
}

/// Exact product predicates can be structural evidence while remaining in the
/// final executable registry. Browser evidence does not prove which source file
/// owns a hook, and the client directive invariant is conditional on client APIs;
/// neither justifies dropping an arbitrary path-specific source assertion.
pub(crate) fn is_generated_hook_check(command: &str) -> bool {
    if compound_hook::checked_path(command).is_some() {
        return true;
    }
    let Some((_, rest)) = command.split_once("readFileSync(\"") else {
        return false;
    };
    let Some((path, _)) = rest.split_once('"') else {
        return false;
    };
    for pattern in [
        "data-anvil-state".to_string(),
        "primary".into(),
        "restart".into(),
        "input".into(),
        "search".into(),
        "submit".into(),
    ] {
        let pattern = if pattern == "data-anvil-state" {
            pattern
        } else {
            format!("data-anvil-action=\"{pattern}\"")
        };
        let grep = format!("grep -q '{pattern}' {path}");
        if let Ok(normalized) = crate::planner::verify::normalize_verify_command(&grep)
            && normalized.as_str() == command
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod issue439_tests;
