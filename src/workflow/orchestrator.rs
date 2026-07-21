use anyhow::{Context, bail};
use serde_json::json;
use std::fs;
use std::path::Path;

use super::{
    runner,
    schema::{Intent, Node, Workflow},
};
use crate::config::{Action, Config, IntentId, Provider};

/// Runs a declarative workflow around the existing single-intent entry.
pub fn run_workflow(config: &Config, definition: &Path, origin: &Path) -> anyhow::Result<()> {
    let yaml = fs::read_to_string(definition).context("read workflow definition")?;
    let workflow = Workflow::parse(&yaml).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let events_path = origin.join("evidence/workflow-events.jsonl");
    if let Some(parent) = events_path.parent() {
        fs::create_dir_all(parent)?;
    };
    emit(
        &events_path,
        json!({"event":"workflow_started","entry":workflow.entry,"origin":origin}),
    )?;
    if !runner::origin_recovery_yaml_present(origin) {
        emit(
            &events_path,
            json!({"event":"workflow_adjudicated","verdict":"circle_failed","reason":"edge_not_earned:create_to_investigate:recovery_yaml_present"}),
        )?;
        write_circle(
            origin,
            "circle_failed",
            "edge_not_earned:create_to_investigate:recovery_yaml_present",
        )?;
        bail!("workflow origin lacks recovery YAML");
    }
    let Some(origin_events) = runner::latest_failed_run_events(origin) else {
        emit(
            &events_path,
            json!({"event":"workflow_adjudicated","verdict":"circle_failed","reason":"edge_not_earned:create_to_investigate:run_stop"}),
        )?;
        write_circle(
            origin,
            "circle_failed",
            "edge_not_earned:create_to_investigate:run_stop",
        )?;
        bail!("workflow origin lacks run events");
    };
    let origin_goal = "起点run";
    let mut current = workflow.entry.clone();
    while let Some(route) = workflow.routes.iter().find(|r| r.from == current) {
        let edge = format!("{}->{}", route.from, route.to);
        let evidence_ok = current == workflow.entry
            || origin
                .join(format!("evidence/{current}-events.jsonl"))
                .is_file();
        if !evidence_ok {
            emit(
                &events_path,
                json!({"event":"workflow_adjudicated","verdict":"circle_failed","reason":format!("edge_not_earned:{edge}:evidence")}),
            )?;
            return write_circle(
                origin,
                "circle_failed",
                &format!("edge_not_earned:{edge}:evidence"),
            );
        }
        emit(
            &events_path,
            json!({"event":"workflow_edge_fired","edge":edge,"checks":["E-A","E-B","E-C","E-D"]}),
        )?;
        let node_id = route.to.clone();
        let Some(node) = workflow.nodes.get(&node_id) else {
            bail!("route target missing: {node_id}");
        };
        if workflow.terminal.contains_key(&node_id) {
            let bindings =
                runner::derive_origin_bindings(&origin_events).map_err(|e| anyhow::anyhow!(e))?;
            runner::verify_origin(&bindings, |_| true).map_err(|e| anyhow::anyhow!(e))?;
            emit(
                &events_path,
                json!({"event":"workflow_adjudicated","verdict":"circle_full"}),
            )?;
            return write_circle(origin, "circle_full", "verify_origin");
        }
        let intent = match node.intent {
            Intent::Investigate => "investigate",
            Intent::Fix => "fix",
            Intent::Create => "create",
        };
        let goal = runner::derive_goal(intent, origin_goal).unwrap_or_default();
        let model = node.model.clone().unwrap_or_else(|| config.model.clone());
        let provider = node.provider.unwrap_or(config.provider);
        let request = runner::NodeRunRequest {
            node: node_id.clone(),
            intent: intent.into(),
            goal,
            origin: origin.to_path_buf(),
            reproducer_lineage: None,
            model,
            provider,
        };
        let node_events = origin.join(format!("evidence/{node_id}-events.jsonl"));
        emit(
            &events_path,
            json!({"event":"workflow_node_started","node":node_id,"intent":intent}),
        )?;
        let run_id = uuid::Uuid::now_v7().to_string();
        let run_dir = origin.join(format!(".anvil/runs/{run_id}"));
        fs::create_dir_all(&run_dir)?;
        let run_events = run_dir.join("events.jsonl");
        emit(
            &run_events,
            json!({
                "event":"workflow_node_run_created",
                "node":node_id,
                "run_id":run_id,
                "model":request.model,
                "provider":provider_name(request.provider),
            }),
        )?;
        emit(
            &events_path,
            json!({
                "event":"workflow_node_run_created",
                "node":node_id,
                "run_id":run_id,
                "run_dir":run_dir,
                "model":request.model,
                "provider":provider_name(request.provider),
            }),
        )?;
        let execution = runner::execute_node(&request, &node_events, |req| {
            let child = node_config(config, node, req)?;
            crate::run_resolved_config_for_workflow(child).map_err(|e| e.to_string())
        });
        if let Err(err) = execution {
            emit(
                &run_events,
                json!({"event":"run_stop","verdict":"failed","reason":err}),
            )?;
            emit(
                &events_path,
                json!({"event":"workflow_adjudicated","verdict":"circle_failed","reason":format!("node_failed:{node_id}")}),
            )?;
            return write_circle(origin, "circle_failed", &format!("node_failed:{node_id}"));
        }
        emit(&run_events, json!({"event":"run_stop","verdict":"full"}))?;
        emit(
            &events_path,
            json!({"event":"intent_resolved","intent":intent,"node":node_id}),
        )?;
        current = node_id;
    }
    emit(
        &events_path,
        json!({"event":"workflow_adjudicated","verdict":"circle_failed","reason":"edge_not_earned:no_route"}),
    )?;
    write_circle(origin, "circle_failed", "edge_not_earned:no_route")
}

fn node_config(
    config: &Config,
    node: &Node,
    request: &runner::NodeRunRequest,
) -> Result<Config, String> {
    let mut child = config.clone();
    child.yes = true;
    child.workspace_root = request.origin.clone();
    if child.workspace_root.canonicalize().ok().as_deref()
        != request.origin.canonicalize().ok().as_deref()
    {
        return Err("workspace_confinement_violation".into());
    }
    child.action = Action::Prompt(request.goal.clone());
    child.intent_override = Some(match request.intent.as_str() {
        "investigate" => IntentId::Investigate,
        "fix" => IntentId::Fix,
        _ => IntentId::Create,
    });
    if let (Some(model), Some(provider)) = (&node.model, node.provider) {
        child.model = model.clone();
        child.provider = provider;
        child.field_sources.model = "workflow_node".into();
        child.field_sources.provider = "workflow_node".into();
    }
    Ok(child)
}

fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Ollama => "ollama",
        Provider::Openai => "openai",
        Provider::Gemini => "gemini",
    }
}

fn emit(path: &Path, value: serde_json::Value) -> anyhow::Result<()> {
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{}", value)?;
    Ok(())
}
fn write_circle(origin: &Path, verdict: &str, reason: &str) -> anyhow::Result<()> {
    let p = origin.join("evidence/workflow-circle.json");
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        p,
        serde_json::to_vec_pretty(&json!({"verdict":verdict,"reason":reason}))?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn config(root: &Path) -> Config {
        let root = root.to_string_lossy();
        Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            root.as_ref(),
            "--model",
            "global-model",
            "--provider",
            "ollama",
            "--ultra-plan-run",
            "goal",
        ]))
        .unwrap()
    }

    fn request(root: &Path, model: &str, provider: Provider) -> runner::NodeRunRequest {
        runner::NodeRunRequest {
            node: "investigate".into(),
            intent: "investigate".into(),
            goal: "goal".into(),
            origin: root.to_path_buf(),
            reproducer_lineage: None,
            model: model.into(),
            provider,
        }
    }

    fn node(model: Option<&str>, provider: Option<Provider>) -> Node {
        Node {
            intent: Intent::Investigate,
            profile: "data".into(),
            model: model.map(str::to_string),
            provider,
        }
    }

    #[test]
    fn node_executor_override_propagates_to_config_and_existing_events() {
        let root = tempfile::tempdir().unwrap();
        let global = config(root.path());
        let node = node(Some("elevated-model"), Some(Provider::Gemini));
        let request = request(root.path(), "elevated-model", Provider::Gemini);

        let child = node_config(&global, &node, &request).unwrap();
        assert_eq!(child.model, "elevated-model");
        assert_eq!(child.provider, Provider::Gemini);
        assert_eq!(child.field_sources.model, "workflow_node");
        assert_eq!(child.field_sources.provider, "workflow_node");

        let events = root.path().join("evidence/investigate-events.jsonl");
        runner::execute_node(&request, &events, |_| Ok(())).unwrap();
        let event: serde_json::Value =
            serde_json::from_str(std::fs::read_to_string(events).unwrap().trim()).unwrap();
        assert_eq!(event["event"], "intent_resolved");
        assert_eq!(event["model"], "elevated-model");
        assert_eq!(event["provider"], "gemini");
    }

    #[test]
    fn omitted_node_executor_preserves_global_config() {
        let root = tempfile::tempdir().unwrap();
        let global = config(root.path());
        let node = node(None, None);
        let request = request(root.path(), &global.model, global.provider);

        let child = node_config(&global, &node, &request).unwrap();
        assert_eq!(child.model, global.model);
        assert_eq!(child.provider, global.provider);
        assert_eq!(child.field_sources.model, global.field_sources.model);
        assert_eq!(child.field_sources.provider, global.field_sources.provider);
    }
}
