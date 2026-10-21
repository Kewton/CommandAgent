pub const CLI_HELP: &str = "Refuse runtime dependency setup and Bash commands containing npm/pnpm/yarn/cargo install, curl, or wget (lexical check, always). When --sandbox applies an OS backend, the OS also refuses outbound network connections from sandboxed commands: macOS allows only loopback (TCP and UDP) and unix sockets; Linux needs Landlock ABI4 (kernel 6.7) or later and then allows TCP only to the dev-server port(s) without restricting UDP, while below ABI4 the network is not confined and only the lexical check applies. Without a backend only the lexical check applies. Provider/API requests are unaffected.";

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
            "enabled: runtime dependency setup and listed Bash commands are refused by a lexical check; when a sandbox backend applies, the OS also refuses outbound network connections from sandboxed commands (macOS always; Linux only with Landlock ABI4, kernel 6.7 or later, below that the network is checked lexically only); without a backend only the lexical check applies; provider/API requests remain enabled"
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

    #[test]
    fn enabled_message_separates_the_os_layer_from_the_lexical_check() {
        assert_eq!(
            doctor_scope(true).message(),
            "enabled: runtime dependency setup and listed Bash commands are refused by a lexical check; when a sandbox backend applies, the OS also refuses outbound network connections from sandboxed commands (macOS always; Linux only with Landlock ABI4, kernel 6.7 or later, below that the network is checked lexically only); without a backend only the lexical check applies; provider/API requests remain enabled"
        );
    }
}
