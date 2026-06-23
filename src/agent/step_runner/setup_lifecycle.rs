//! Typed setup-job lifecycle evidence.
//!
//! This module is intentionally a record/rendering boundary. It does not run
//! setup commands, read manifests, select recovery jobs, or broaden tool
//! policy. Runtime setup code owns execution; recovery orchestration owns final
//! active-job dispatch.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetupJobLifecycle {
    pub(crate) setup_job_kind: String,
    pub(crate) setup_job_state: String,
    pub(crate) setup_target: Option<String>,
    pub(crate) setup_manifest_kind: Option<String>,
    pub(crate) setup_manifest_path: Option<String>,
    pub(crate) setup_artifact_validation_status: Option<String>,
    pub(crate) setup_readiness: Option<String>,
    pub(crate) setup_command_authority: Option<String>,
    pub(crate) setup_command: Option<String>,
    pub(crate) setup_attempt_key: Option<String>,
    pub(crate) setup_attempt_key_before: Option<String>,
    pub(crate) setup_attempt_key_after: Option<String>,
    pub(crate) setup_manifest_fingerprint: Option<String>,
    pub(crate) setup_stale_reason: Option<String>,
    pub(crate) setup_result: Option<String>,
    pub(crate) setup_failure_signature: Option<String>,
    pub(crate) verifier_command: Option<String>,
    pub(crate) verifier_rerun_result: Option<String>,
    pub(crate) rerun_authority: Vec<String>,
    pub(crate) runtime_job_outcome: Option<String>,
    pub(crate) explicit_stop_reason: Option<String>,
}

impl SetupJobLifecycle {
    pub(crate) fn new(
        setup_job_kind: impl Into<String>,
        setup_job_state: impl Into<String>,
    ) -> Self {
        Self {
            setup_job_kind: setup_job_kind.into(),
            setup_job_state: setup_job_state.into(),
            setup_target: None,
            setup_manifest_kind: None,
            setup_manifest_path: None,
            setup_artifact_validation_status: None,
            setup_readiness: None,
            setup_command_authority: None,
            setup_command: None,
            setup_attempt_key: None,
            setup_attempt_key_before: None,
            setup_attempt_key_after: None,
            setup_manifest_fingerprint: None,
            setup_stale_reason: None,
            setup_result: None,
            setup_failure_signature: None,
            verifier_command: None,
            verifier_rerun_result: None,
            rerun_authority: Vec::new(),
            runtime_job_outcome: None,
            explicit_stop_reason: None,
        }
    }

    pub(crate) fn with_setup_target(mut self, value: impl Into<String>) -> Self {
        self.setup_target = Some(value.into());
        self
    }

    pub(crate) fn with_manifest(
        mut self,
        kind: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        self.setup_manifest_kind = Some(kind.into());
        self.setup_manifest_path = Some(path.into());
        self
    }

    pub(crate) fn with_artifact_validation_status(mut self, value: impl Into<String>) -> Self {
        self.setup_artifact_validation_status = Some(value.into());
        self
    }

    pub(crate) fn with_readiness(mut self, value: impl Into<String>) -> Self {
        self.setup_readiness = Some(value.into());
        self
    }

    pub(crate) fn with_command_authority(mut self, value: impl Into<String>) -> Self {
        self.setup_command_authority = Some(value.into());
        self
    }

    pub(crate) fn with_command(mut self, value: impl Into<String>) -> Self {
        self.setup_command = Some(value.into());
        self
    }

    pub(crate) fn with_attempt_key(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.setup_attempt_key = Some(value.clone());
        self.setup_attempt_key_before = Some(value);
        self
    }

    pub(crate) fn with_attempt_key_after(mut self, value: impl Into<String>) -> Self {
        self.setup_attempt_key_after = Some(value.into());
        self
    }

    pub(crate) fn with_manifest_fingerprint(mut self, value: impl Into<String>) -> Self {
        self.setup_manifest_fingerprint = Some(value.into());
        self
    }

    pub(crate) fn with_stale_reason(mut self, value: impl Into<String>) -> Self {
        self.setup_stale_reason = Some(value.into());
        self
    }

    pub(crate) fn with_setup_result(mut self, value: impl Into<String>) -> Self {
        self.setup_result = Some(value.into());
        self
    }

    pub(crate) fn with_failure_signature(mut self, value: impl Into<String>) -> Self {
        self.setup_failure_signature = Some(value.into());
        self
    }

    pub(crate) fn with_verifier_command(mut self, value: impl Into<String>) -> Self {
        self.verifier_command = Some(value.into());
        self
    }

    pub(crate) fn with_verifier_rerun_result(mut self, value: impl Into<String>) -> Self {
        self.verifier_rerun_result = Some(value.into());
        self
    }

    pub(crate) fn with_rerun_authority<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.rerun_authority = values.into_iter().map(Into::into).collect();
        self
    }

    pub(crate) fn with_runtime_job_outcome(mut self, value: impl Into<String>) -> Self {
        self.runtime_job_outcome = Some(value.into());
        self
    }

    pub(crate) fn with_explicit_stop_reason(mut self, value: impl Into<String>) -> Self {
        self.explicit_stop_reason = Some(value.into());
        self
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        push(&mut lines, "runtime_job_kind", Some(&self.setup_job_kind));
        push(
            &mut lines,
            "runtime_job_outcome",
            self.runtime_job_outcome.as_deref(),
        );
        push(&mut lines, "setup_job_kind", Some(&self.setup_job_kind));
        push(&mut lines, "setup_job_state", Some(&self.setup_job_state));
        push(&mut lines, "setup_state", Some(&self.setup_job_state));
        push(&mut lines, "setup_target", self.setup_target.as_deref());
        push(
            &mut lines,
            "setup_manifest_kind",
            self.setup_manifest_kind.as_deref(),
        );
        push(
            &mut lines,
            "setup_manifest_path",
            self.setup_manifest_path.as_deref(),
        );
        push(
            &mut lines,
            "setup_artifact_validation_status",
            self.setup_artifact_validation_status.as_deref(),
        );
        push(
            &mut lines,
            "setup_readiness",
            self.setup_readiness.as_deref(),
        );
        push(
            &mut lines,
            "setup_command_authority",
            self.setup_command_authority.as_deref(),
        );
        push(&mut lines, "setup_command", self.setup_command.as_deref());
        push(
            &mut lines,
            "setup_attempt_key",
            self.setup_attempt_key.as_deref(),
        );
        push(
            &mut lines,
            "setup_attempt_key_before",
            self.setup_attempt_key_before.as_deref(),
        );
        push(
            &mut lines,
            "setup_attempt_key_after",
            self.setup_attempt_key_after.as_deref(),
        );
        push(
            &mut lines,
            "setup_manifest_fingerprint",
            self.setup_manifest_fingerprint.as_deref(),
        );
        push(
            &mut lines,
            "setup_stale_reason",
            self.setup_stale_reason.as_deref(),
        );
        push(&mut lines, "setup_result", self.setup_result.as_deref());
        push(
            &mut lines,
            "setup_failure_signature",
            self.setup_failure_signature.as_deref(),
        );
        push(
            &mut lines,
            "verifier_command",
            self.verifier_command.as_deref(),
        );
        push(
            &mut lines,
            "verifier_rerun_result",
            self.verifier_rerun_result.as_deref(),
        );
        if !self.rerun_authority.is_empty() {
            lines.push(format!(
                "rerun_authority={}",
                self.rerun_authority.join("|")
            ));
        }
        push(
            &mut lines,
            "explicit_stop_reason",
            self.explicit_stop_reason.as_deref(),
        );
        lines
    }
}

fn push(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    if value.trim().is_empty() {
        return;
    }
    lines.push(format!("{key}={}", value.trim()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_setup_lifecycle_fields() {
        let lines = SetupJobLifecycle::new("setup_bootstrap", "blocked")
            .with_setup_target("package.json")
            .with_manifest("node_package", "package.json")
            .with_artifact_validation_status("passed")
            .with_readiness("missing_dependency_artifact")
            .with_command_authority("blocked_offline")
            .with_command("npm install --include=dev")
            .with_attempt_key("step=verify;command=npm install --include=dev;manifest=x")
            .with_manifest_fingerprint("package_json=1:abc")
            .with_setup_result("blocked_by_policy")
            .with_verifier_rerun_result("not_run")
            .with_rerun_authority(["npm run build"])
            .with_runtime_job_outcome("blocked")
            .render_lines();

        assert!(lines.contains(&"runtime_job_kind=setup_bootstrap".to_string()));
        assert!(lines.contains(&"setup_job_state=blocked".to_string()));
        assert!(lines.contains(&"setup_manifest_path=package.json".to_string()));
        assert!(lines.contains(&"setup_command_authority=blocked_offline".to_string()));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("setup_attempt_key="))
        );
        assert!(lines.contains(&"verifier_rerun_result=not_run".to_string()));
    }

    #[test]
    fn renders_phase26_setup_result_and_stale_failure_ledger() {
        let lines = SetupJobLifecycle::new("setup_bootstrap", "failed")
            .with_setup_target("Cargo.toml")
            .with_manifest("rust_cargo", "Cargo.toml")
            .with_artifact_validation_status("failed")
            .with_readiness("toolchain_or_manifest_blocked")
            .with_command_authority("verifier_owned_setup_only")
            .with_attempt_key("profile=rust;command=cargo test;manifest=old")
            .with_attempt_key_after("profile=rust;command=cargo test;manifest=new")
            .with_manifest_fingerprint("cargo_toml=2:def")
            .with_stale_reason("manifest fingerprint changed after setup evidence")
            .with_setup_result("failed")
            .with_failure_signature("setup|rust|cargo test|setup_manifest_invalid_cargo_toml")
            .with_verifier_command("cargo test")
            .with_verifier_rerun_result("not_run")
            .with_rerun_authority(["cargo test"])
            .with_runtime_job_outcome("failed")
            .render_lines();

        assert!(lines.contains(&"setup_target=Cargo.toml".to_string()));
        assert!(lines.contains(&"setup_manifest_kind=rust_cargo".to_string()));
        assert!(lines.contains(&"setup_readiness=toolchain_or_manifest_blocked".to_string()));
        assert!(lines.contains(&"setup_command_authority=verifier_owned_setup_only".to_string()));
        assert!(lines.contains(&"setup_result=failed".to_string()));
        assert!(lines.iter().any(|line| line
            == "setup_failure_signature=setup|rust|cargo test|setup_manifest_invalid_cargo_toml"));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("setup_attempt_key_after="))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("setup_stale_reason=manifest fingerprint changed"))
        );
    }
}
