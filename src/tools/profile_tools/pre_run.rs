//! The CLI's pre-run admission: before the request pattern settings, the OS
//! sandbox, the artifact preflight, `run_start`, and any provider or classifier
//! call. It reads nothing when no pack is selected and no profile declares tools.
//! A pack the CLI could not resolve stays that resolution's refusal, at its
//! existing place.

use super::ProfileToolRefusal;
use super::admission::{AdmittedProfileTools, admit_with};
use super::selection::PackSource;
use crate::cli_pack::ResolvedPack;
use crate::config::Config;

pub(crate) fn refuse_before_cli_run(
    config: &Config,
    resolved_pack: &anyhow::Result<Option<ResolvedPack>>,
) -> Result<(), ProfileToolRefusal> {
    let pack = match resolved_pack {
        Ok(Some(pack)) => PackSource::Selected {
            selection: pack.selection(),
            declared: pack.declared.clone(),
        },
        Ok(None) => PackSource::InheritedBeforeGates,
        Err(_) => PackSource::None,
    };
    let admitted = admit_with(&config.workspace_root, &config.profile, None, pack)?;
    preflight(admitted, config)
}

/// A workflow child runs under the sandbox and the pack selection its outer run
/// installed, already reconciled with the child's workspace.
pub(crate) fn refuse_before_workflow_node(config: &Config) -> Result<(), ProfileToolRefusal> {
    let admitted = super::admit_selected(&config.workspace_root, &config.profile, None)?;
    preflight(admitted, config)
}

/// A REPL's free text, before its route is proposed. The route classifier is a provider
/// call, and the REPL's provider, profile, and inputs can all change after it started.
/// Only a schema tool's and a proposal tool's reasons stop here. Every other refusal keeps
/// its place at the confirmed command, so a set without one proposes a route as it always
/// did. Returns the block a refused command shows; the REPL goes on.
pub(crate) fn refused_before_gate_one(
    config: &Config,
    execution: &dyn crate::providers::ChatClient,
) -> Option<String> {
    let admitted = super::admit_selected(&config.workspace_root, &config.profile, None);
    let refused = refusal_before_gate_one(admitted, config, execution)?;
    Some(crate::tui::status::render_task_failure_block(
        &crate::tui::status::TaskFailureBlock {
            task_status: "failed".to_string(),
            failed_phase: String::new(),
            completed_phases: None,
            total_phases: None,
            primary_stop_reason: refused.to_string(),
            recovery_prompt_command: String::new(),
            recovery_ultra_plan_command: String::new(),
            summary_path: String::new(),
        },
    ))
}

fn refusal_before_gate_one(
    admitted: Result<Option<AdmittedProfileTools>, ProfileToolRefusal>,
    config: &Config,
    execution: &dyn crate::providers::ChatClient,
) -> Option<ProfileToolRefusal> {
    match admitted {
        Ok(None) => None,
        Ok(Some(admitted)) => admitted
            .preflight_schema_tools(config, execution.profile_tool_schema_wire())
            .and_then(|()| admitted.preflight_stage_run(config))
            .err(),
        Err(refusal)
            if refusal.kind.is_schema_tool_reason() || refusal.kind.is_effect_tool_reason() =>
        {
            Some(refusal)
        }
        Err(_) => None,
    }
}

fn preflight(
    admitted: Option<AdmittedProfileTools>,
    config: &Config,
) -> Result<(), ProfileToolRefusal> {
    match admitted {
        Some(admitted) => admitted.preflight_schema_tools(config, super::configured_wire(config)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::refusal_before_gate_one;
    use crate::config::Config;
    use crate::providers::{AssistantReply, ChatClient};
    use crate::state::ConversationMessage;
    use crate::tools::os_sandbox::{
        SandboxAvailability, SandboxMode, SandboxPolicy, SandboxTestState, install_for_test,
    };
    use crate::tools::profile_tools::RefusalKind;
    use crate::tools::profile_tools::admission::AdmittedProfileTools;
    use crate::tools::registry::ToolSpec;

    /// Counts what would be the route classifier's provider call.
    struct CountingClient(Arc<AtomicUsize>);

    impl ChatClient for CountingClient {
        fn label(&self) -> &str {
            "counting"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(Self(self.0.clone()))
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            self.0.fetch_add(1, Ordering::SeqCst);
            anyhow::bail!("no provider call belongs here")
        }
    }

    fn config_outside(root: &Path) -> Config {
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let mut config = crate::tools::profile_tools::effects::config_for_test(&workspace);
        config.state_dir = root.join("state");
        config
    }

    fn install(root: &Path, mode: SandboxMode) {
        let tmp = root.join("sandbox/run");
        std::fs::create_dir_all(&tmp).unwrap();
        install_for_test(
            mode,
            SandboxPolicy::new(&root.join("workspace"), &tmp),
            SandboxAvailability::Available,
        );
    }

    /// The spellings `--profile` keeps, against the canonical name a preset's plan has.
    #[test]
    fn the_same_name_in_two_spellings() {
        let same = crate::tools::profile_tools::names_the_same_profile;
        assert!(same("next.js", "nextjs"));
        assert!(same("py-cli", "python-cli"));
        assert!(same(" Stage-Named-Profile ", "stage-named-profile"));
        assert!(!same("nextjs", "python-cli"));
        assert!(!same("stage-named-profile", "stage-another-profile"));
    }

    /// The filter stands in front of the route classifier: refusing here is what keeps that
    /// provider call from happening. A REPL needs a terminal, so the filter itself is what runs.
    #[test]
    fn free_text_under_no_os_boundary_stops_at_the_filter_with_no_provider_call() {
        let _sandbox = SandboxTestState::acquire();
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let config = config_outside(&root);
        let calls = Arc::new(AtomicUsize::new(0));
        let client = CountingClient(calls.clone());
        install(&root, SandboxMode::Off);

        let staged = AdmittedProfileTools::with_stage_set_for_test(&config.workspace_root);
        let refused = refusal_before_gate_one(Ok(Some(staged)), &config, &client)
            .expect("the filter refuses");
        assert_eq!(refused.kind, RefusalKind::StageContractRefused);
        assert!(refused.to_string().contains("stage_contract_refused"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        // The same set under an OS boundary is not this filter's to stop.
        install(&root, SandboxMode::Auto);
        let staged = AdmittedProfileTools::with_stage_set_for_test(&config.workspace_root);
        assert_eq!(
            refusal_before_gate_one(Ok(Some(staged)), &config, &client),
            None
        );

        // No declared stage: the same as ever, under any sandbox.
        install(&root, SandboxMode::Off);
        assert_eq!(refusal_before_gate_one(Ok(None), &config, &client), None);
        let plain = AdmittedProfileTools::without_stages_for_test();
        assert_eq!(
            refusal_before_gate_one(Ok(Some(plain)), &config, &client),
            None
        );
    }

    /// B4: a run that may recover by itself stops here too, before the classifier's call. A set
    /// with no stage is not this filter's to stop, whatever the recovery setting.
    #[test]
    fn free_text_that_may_recover_by_itself_stops_at_the_filter_with_no_provider_call() {
        let _sandbox = SandboxTestState::acquire();
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let mut config = config_outside(&root);
        config.recovery_plan_auto_runs = 1;
        let calls = Arc::new(AtomicUsize::new(0));
        let client = CountingClient(calls.clone());
        install(&root, SandboxMode::Auto);

        let staged = AdmittedProfileTools::with_stage_set_for_test(&config.workspace_root);
        let refused = refusal_before_gate_one(Ok(Some(staged)), &config, &client)
            .expect("the filter refuses");
        assert_eq!(refused.kind, RefusalKind::StageContractRefused);
        assert!(
            refused.to_string().contains("RecoversByItself"),
            "{refused}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let plain = AdmittedProfileTools::without_stages_for_test();
        assert_eq!(
            refusal_before_gate_one(Ok(Some(plain)), &config, &client),
            None
        );
    }
}
