//! Leaf projection for workflow node planner provider/model pins.

use serde_json::json;

use crate::config::Config;

use super::runner::NodeRunRequest;

pub(super) fn apply_to_config(config: &mut Config, request: &NodeRunRequest) {
    if let (Some(model), Some(provider)) = (&request.planner_model, request.planner_provider) {
        config.planner_model = model.clone();
        config.planner_provider = provider;
        config.field_sources.planner_model = "workflow_node".into();
        config.field_sources.planner_provider = "workflow_node".into();
    }
}

pub(super) fn add_to_event(value: &mut serde_json::Value, request: &NodeRunRequest) {
    if let (Some(model), Some(provider)) = (&request.planner_model, request.planner_provider) {
        value["planner_model"] = json!(model);
        value["planner_provider"] = json!(provider.as_str());
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use clap::Parser;

    use super::*;
    use crate::config::Provider;

    fn config(root: &Path) -> Config {
        Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            root.to_str().unwrap(),
            "--model",
            "global-model",
            "--provider",
            "ollama",
            "--ultra-plan-run",
            "goal",
        ]))
        .unwrap()
    }

    fn request(root: &Path) -> NodeRunRequest {
        NodeRunRequest {
            node: "investigate".into(),
            intent: "investigate".into(),
            profile: "data".into(),
            goal: "goal".into(),
            origin: root.to_path_buf(),
            reproducer: None,
            model: "global-model".into(),
            provider: Provider::Ollama,
            planner_model: Some("small-planner".into()),
            planner_provider: Some(Provider::Gemini),
            diagnosis: None,
        }
    }

    #[test]
    fn explicit_pair_updates_config_provenance_and_event() {
        let root = tempfile::tempdir().unwrap();
        let mut config = config(root.path());
        let request = request(root.path());
        let mut event = json!({"event":"workflow_node_run_created"});

        apply_to_config(&mut config, &request);
        add_to_event(&mut event, &request);

        assert_eq!(config.planner_model, "small-planner");
        assert_eq!(config.planner_provider, Provider::Gemini);
        assert_eq!(config.field_sources.planner_model, "workflow_node");
        assert_eq!(config.field_sources.planner_provider, "workflow_node");
        assert_eq!(event["planner_model"], "small-planner");
        assert_eq!(event["planner_provider"], "gemini");
    }

    #[test]
    fn omitted_pair_preserves_config_and_event_shape() {
        let root = tempfile::tempdir().unwrap();
        let mut config = config(root.path());
        let original_model = config.planner_model.clone();
        let original_provider = config.planner_provider;
        let original_model_source = config.field_sources.planner_model.clone();
        let original_provider_source = config.field_sources.planner_provider.clone();
        let mut request = request(root.path());
        request.planner_model = None;
        request.planner_provider = None;
        let mut event = json!({"event":"workflow_node_run_created"});

        apply_to_config(&mut config, &request);
        add_to_event(&mut event, &request);

        assert_eq!(config.planner_model, original_model);
        assert_eq!(config.planner_provider, original_provider);
        assert_eq!(config.field_sources.planner_model, original_model_source);
        assert_eq!(
            config.field_sources.planner_provider,
            original_provider_source
        );
        assert!(event.get("planner_model").is_none());
        assert!(event.get("planner_provider").is_none());
    }
}
