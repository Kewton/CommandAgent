use anyhow::{Context, bail};
use serde_json::json;
use std::fs;
use std::path::Path;

use super::{
    runner,
    schema::{Intent, Workflow},
};
use crate::config::{Action, Config};

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
    loop {
        let Some(route) = workflow.routes.iter().find(|r| r.from == current) else {
            break;
        };
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
        let request = runner::NodeRunRequest {
            node: node_id.clone(),
            intent: intent.into(),
            goal,
            origin: origin.to_path_buf(),
            reproducer_lineage: None,
        };
        let node_events = origin.join(format!("evidence/{node_id}-events.jsonl"));
        emit(
            &events_path,
            json!({"event":"workflow_node_started","node":node_id,"intent":intent}),
        )?;
        let run_id = uuid::Uuid::now_v7().to_string();
        emit(
            &events_path,
            json!({"event":"workflow_node_run_created","node":node_id,"run_id":run_id,"run_dir":origin.join(format!(".anvil/runs/{run_id}"))}),
        )?;
        runner::execute_node(&request, &node_events, |req| {
            let mut child = config.clone();
            child.yes = true;
            child.action = Action::Prompt(req.goal.clone());
            child.intent_override = Some(match req.intent.as_str() {
                "investigate" => crate::config::IntentId::Investigate,
                "fix" => crate::config::IntentId::Fix,
                _ => crate::config::IntentId::Create,
            });
            crate::run_resolved_config_for_workflow(child).map_err(|e| e.to_string())
        })
        .map_err(|e| anyhow::anyhow!(e))?;
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
