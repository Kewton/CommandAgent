//! Distinguish a closed package-value comparison from actual setup/server work.
use super::{dependency_install_verify_segment, is_localhost_reference};

pub(super) fn is_setup_or_dev_server_verify_command(command: &str) -> bool {
    if crate::minimal_loop::evidence::package_script_check::comparison(command).is_some() {
        return false;
    }
    let lower = command.to_ascii_lowercase();
    let lower = lower.as_str();
    if lower.starts_with("node -p ") || lower.starts_with("node --print ") {
        return false;
    }
    dependency_install_verify_segment(lower).is_some()
        || lower.contains("cargo install")
        || lower.contains("npm run dev")
        || lower.contains("pnpm dev")
        || lower.contains("yarn dev")
        || lower.contains("next dev")
        || lower.contains("vite --host")
        || lower.contains("vite --port")
        || (lower.contains("curl ") && is_localhost_reference(lower))
        || (lower.contains("wget ") && is_localhost_reference(lower))
        || lower.contains("python -m http.server")
        || lower.contains("python3 -m http.server")
        || lower.contains("server start")
        || lower.contains("serve ")
}
