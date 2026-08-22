pub const CLI_HELP: &str = "Block runtime dependency setup and Bash commands containing npm/pnpm/yarn/cargo install, curl, or wget. Provider/API requests and other network-capable commands are unaffected.";

pub const BLOCKED_BASH_COMMAND_FAMILIES: &[&str] = &[
    "npm install",
    "pnpm install",
    "yarn install",
    "cargo install",
    "curl",
    "wget",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorScope {
    pub enabled: bool,
    pub runtime_dependency_setup_blocked: bool,
    pub provider_requests_blocked: bool,
}

impl DoctorScope {
    pub fn message(&self) -> &'static str {
        if self.enabled {
            "enabled for runtime dependency setup and listed Bash commands; provider/API requests remain enabled"
        } else {
            "disabled; runtime dependency setup and network-capable Bash commands may run"
        }
    }
}

pub fn doctor_scope(enabled: bool) -> DoctorScope {
    DoctorScope {
        enabled,
        runtime_dependency_setup_blocked: enabled,
        provider_requests_blocked: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_scope_never_claims_provider_is_offline() {
        let scope = doctor_scope(true);
        assert!(scope.runtime_dependency_setup_blocked);
        assert!(!scope.provider_requests_blocked);
        assert!(
            scope
                .message()
                .contains("provider/API requests remain enabled")
        );
    }
}
