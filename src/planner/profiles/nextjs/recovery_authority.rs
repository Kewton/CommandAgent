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
