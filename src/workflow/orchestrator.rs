use anyhow::{Context, bail};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    evidence::{
        EdgeCheck, EdgeChecks, EdgeRecord, NodeRunReference, OriginReference,
        WorkflowCircleEvidence,
    },
    runner,
    schema::{Carry, Intent, Node, Route, Workflow},
};
use crate::config::{Action, Config, IntentId, PlanPreset, Provider};

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
    let recovery_yaml_paths = runner::origin_recovery_yamls(origin);
    if recovery_yaml_paths.is_empty() {
        emit(
            &events_path,
            json!({"event":"workflow_adjudicated","verdict":"circle_failed","reason":"edge_not_earned:create_to_investigate:recovery_yaml_present"}),
        )?;
        bail!("workflow origin lacks recovery YAML");
    }
    let Some(origin_events) = runner::latest_failed_run_events(origin) else {
        emit(
            &events_path,
            json!({"event":"workflow_adjudicated","verdict":"circle_failed","reason":"edge_not_earned:create_to_investigate:run_stop"}),
        )?;
        bail!("workflow origin lacks run events");
    };
    let origin_run_id = origin_events
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("workflow origin run id is underivable"))?
        .to_string();
    let mut circle = WorkflowCircleEvidence::new(
        workflow.workflow.clone(),
        OriginReference {
            workspace_root: origin.to_path_buf(),
            run_id: origin_run_id,
            events_path: origin_events.clone(),
            recovery_yaml_paths,
            goal: None,
        },
    );
    let origin_goal = match runner::derive_origin_goal(&origin_events) {
        Ok(goal) => goal,
        Err(_) => {
            emit(
                &events_path,
                json!({"event":"workflow_adjudicated","verdict":"circle_failed","reason":"origin_goal_underivable"}),
            )?;
            circle.adjudicate("circle_failed", Some("origin_goal_underivable"));
            return write_circle(origin, &circle);
        }
    };
    circle.origin.goal = Some(origin_goal.clone());
    let reproducer_record = super::origin_reproducer::derive_from_origin(
        config,
        origin,
        &origin_events,
        &origin_goal,
        &events_path,
    );
    circle.record_reproducer_suggestion(reproducer_record);
    let mut current = workflow.entry.clone();
    while let Some(route) = workflow.routes.iter().find(|r| r.from == current) {
        let edge = format!("{}->{}", route.from, route.to);
        let edge_record = evaluate_edge(
            route,
            &edge,
            &current,
            &workflow.entry,
            origin,
            &origin_events,
            &circle,
        );
        let fired = edge_record.fired;
        let failed_check = first_failed_check(&edge_record.checks);
        circle.record_edge(edge_record);
        if !fired {
            let reason = format!("edge_not_earned:{edge}:{failed_check}");
            emit(
                &events_path,
                json!({"event":"workflow_adjudicated","verdict":"circle_failed","reason":reason}),
            )?;
            circle.adjudicate("circle_failed", Some(&reason));
            return write_circle(origin, &circle);
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
            circle.adjudicate("circle_full", Some("verify_origin"));
            return write_circle(origin, &circle);
        }
        let intent = match node.intent {
            Intent::Investigate => "investigate",
            Intent::Fix => "fix",
            Intent::Create => "create",
        };
        let goal = runner::derive_goal(intent, &origin_goal).unwrap_or_default();
        let model = node.model.clone().unwrap_or_else(|| config.model.clone());
        let provider = node.provider.unwrap_or(config.provider);
        let reproducer = match intent {
            "investigate" => circle
                .reproducer_suggestion
                .as_ref()
                .and_then(|record| record.bound.clone()),
            "fix" => Some(
                super::origin_reproducer::binding_from_investigation(origin)
                    .map_err(anyhow::Error::msg)?,
            ),
            _ => None,
        };
        let request = runner::NodeRunRequest {
            node: node_id.clone(),
            intent: intent.into(),
            profile: node.profile.clone(),
            goal,
            origin: origin.to_path_buf(),
            reproducer,
            model,
            provider,
        };
        let node_events = origin.join(format!("evidence/{node_id}-events.jsonl"));
        emit(
            &events_path,
            json!({"event":"workflow_node_started","node":node_id,"intent":intent}),
        )?;
        let identity = NodeRunIdentity::allocate(origin)?;
        emit_node_run_created(&identity, &node_id, &request)?;
        circle
            .record_node(
                node_id.clone(),
                NodeRunReference {
                    intent: intent.to_string(),
                    run_id: identity.run_id.clone(),
                    run_dir: identity.run_dir.clone(),
                    events_path: identity.events_path.clone(),
                },
            )
            .map_err(anyhow::Error::msg)?;
        emit(
            &events_path,
            json!({
                "event":"workflow_node_run_created",
                "node":node_id,
                "run_id":identity.run_id,
                "run_dir":identity.run_dir,
                "model":request.model,
                "provider":provider_name(request.provider),
            }),
        )?;
        let execution = runner::execute_node(&request, &node_events, |req| {
            execute_configured_node(config, node, req, &identity, |child| {
                crate::run_resolved_config_for_workflow(child).map_err(|e| e.to_string())
            })
        });
        if let Err(err) = execution {
            emit_node_run_stop_if_absent(&identity.events_path, "failed", Some(&err))?;
            emit(
                &events_path,
                json!({"event":"workflow_adjudicated","verdict":"circle_failed","reason":format!("node_failed:{node_id}")}),
            )?;
            let reason = format!("node_failed:{node_id}");
            circle.adjudicate("circle_failed", Some(&reason));
            return write_circle(origin, &circle);
        }
        emit_node_run_stop_if_absent(&identity.events_path, "full", None)?;
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
    circle.adjudicate("circle_failed", Some("edge_not_earned:no_route"));
    write_circle(origin, &circle)
}

fn evaluate_edge(
    route: &Route,
    edge: &str,
    current: &str,
    entry: &str,
    origin: &Path,
    origin_events: &Path,
    circle: &WorkflowCircleEvidence,
) -> EdgeRecord {
    let source_evidence = if current == entry {
        origin_events.to_path_buf()
    } else {
        circle
            .nodes
            .get(current)
            .map(|node| node.events_path.clone())
            .unwrap_or_else(|| origin.join(format!("evidence/{current}-events.jsonl")))
    };
    let verdict = if current == entry {
        EdgeCheck::passed(format!(
            "origin selector verified failed run_stop in {}",
            origin_events.display()
        ))
    } else if terminal_status_matches(&source_evidence, route) {
        EdgeCheck::passed(format!(
            "source run terminal verdict matches route in {}",
            source_evidence.display()
        ))
    } else {
        EdgeCheck::failed(format!(
            "source run terminal verdict does not match route in {}",
            source_evidence.display()
        ))
    };
    let evidence_complete = if current == entry {
        source_evidence.is_file()
            && circle
                .origin
                .recovery_yaml_paths
                .iter()
                .all(|path| path.is_file())
    } else {
        circle.nodes.get(current).is_some_and(|node| {
            source_adjudication_and_evidence_exist(&node.intent, origin, &source_evidence)
        })
    };
    let evidence = if evidence_complete {
        EdgeCheck::passed(format!(
            "source evidence and adjudication are complete: {}",
            source_evidence.display()
        ))
    } else {
        EdgeCheck::failed(format!(
            "source evidence or adjudication is missing: {}",
            source_evidence.display()
        ))
    };
    let epoch = if current == entry || circle.nodes.contains_key(current) {
        EdgeCheck::passed(
            "source is in the sequential route history; target run is allocated only after firing",
        )
    } else {
        EdgeCheck::failed("source node has no recorded predecessor run")
    };
    let missing_carries = route
        .carry
        .iter()
        .filter(|carry| !carry_present(carry, origin, circle))
        .map(|carry| format!("{carry:?}").to_ascii_lowercase())
        .collect::<Vec<_>>();
    let carry = if missing_carries.is_empty() {
        EdgeCheck::passed(format!("declared carries present: {:?}", route.carry))
    } else {
        EdgeCheck::failed(format!("missing carries: {}", missing_carries.join(",")))
    };
    let checks = EdgeChecks {
        verdict,
        evidence,
        epoch,
        carry,
    };
    let fired = checks.verdict.passed
        && checks.evidence.passed
        && checks.epoch.passed
        && checks.carry.passed;
    EdgeRecord {
        edge: edge.to_string(),
        fired,
        checks,
    }
}

fn terminal_status_matches(events_path: &Path, route: &Route) -> bool {
    let Ok(events) = fs::read_to_string(events_path) else {
        return false;
    };
    events.lines().rev().find_map(|line| {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        (value["event"] == "run_stop").then(|| {
            let status = value["status"]
                .as_str()
                .or_else(|| value["verdict"].as_str())
                .unwrap_or_default();
            match route.on {
                super::schema::Verdict::Full => matches!(status, "full" | "completed"),
                super::schema::Verdict::Failed => status == "failed",
            }
        })
    }) == Some(true)
}

fn source_adjudication_and_evidence_exist(node: &str, origin: &Path, events_path: &Path) -> bool {
    let Ok(events) = fs::read_to_string(events_path) else {
        return false;
    };
    match node {
        "investigate" => {
            let adjudicated = events.lines().any(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .map(|value| {
                        value["event"] == "investigation_adjudicated"
                            && value["assurance_level"] == "full"
                    })
                    .unwrap_or(false)
            });
            adjudicated
                && origin.join("evidence/investigation-run.json").is_file()
                && origin.join("evidence/investigation-binding.json").is_file()
        }
        "fix" => {
            let adjudicated = events.lines().any(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .map(|value| {
                        value["event"] == "ultra_final_acceptance"
                            && value["intent"] == "fix"
                            && value["verdict"] == "full"
                    })
                    .unwrap_or(false)
            });
            adjudicated
                && fs::read_dir(origin.join("evidence"))
                    .map(|entries| {
                        entries.flatten().any(|entry| {
                            let name = entry.file_name();
                            let name = name.to_string_lossy();
                            name.starts_with("fix-") && name.ends_with("-adjudication.json")
                        })
                    })
                    .unwrap_or(false)
        }
        _ => false,
    }
}

fn carry_present(carry: &Carry, origin: &Path, circle: &WorkflowCircleEvidence) -> bool {
    match carry {
        Carry::Workspace => origin.is_dir(),
        Carry::RecoveryYaml => circle
            .origin
            .recovery_yaml_paths
            .iter()
            .all(|path| path.is_file()),
        Carry::ReproducerSuggestion => circle.reproducer_suggestion.is_some(),
        Carry::ReproducerLineage => {
            super::origin_reproducer::binding_from_investigation(origin).is_ok()
        }
    }
}

fn first_failed_check(checks: &EdgeChecks) -> &'static str {
    if !checks.verdict.passed {
        "verdict"
    } else if !checks.evidence.passed {
        "evidence"
    } else if !checks.epoch.passed {
        "epoch"
    } else {
        "carry"
    }
}

#[derive(Debug, Clone)]
struct NodeRunIdentity {
    run_id: String,
    run_dir: PathBuf,
    events_path: PathBuf,
    state_dir: PathBuf,
}

impl NodeRunIdentity {
    fn allocate(origin: &Path) -> anyhow::Result<Self> {
        let origin = origin
            .canonicalize()
            .context("canonicalize workflow origin")?;
        let runs_dir = origin.join(".anvil/runs");
        fs::create_dir_all(&runs_dir)?;
        let runs_dir = runs_dir.canonicalize()?;
        if !runs_dir.starts_with(&origin) {
            bail!("workspace_confinement_violation");
        }
        let run_id = uuid::Uuid::now_v7().to_string();
        let run_dir = runs_dir.join(&run_id);
        let state_dir = run_dir.join("state");
        fs::create_dir_all(&state_dir)?;
        let run_dir = run_dir.canonicalize()?;
        let state_dir = state_dir.canonicalize()?;
        if !run_dir.starts_with(&origin) || !state_dir.starts_with(&origin) {
            bail!("workspace_confinement_violation");
        }
        let events_path = run_dir.join("events.jsonl");
        Ok(Self {
            run_id,
            run_dir,
            events_path,
            state_dir,
        })
    }
}

fn emit_node_run_created(
    identity: &NodeRunIdentity,
    node_id: &str,
    request: &runner::NodeRunRequest,
) -> anyhow::Result<()> {
    emit(
        &identity.events_path,
        json!({
            "event":"workflow_node_run_created",
            "node":node_id,
            "run_id":identity.run_id,
            "run_dir":identity.run_dir,
            "model":request.model,
            "provider":provider_name(request.provider),
        }),
    )
}

fn emit_node_run_stop_if_absent(
    events_path: &Path,
    verdict: &str,
    reason: Option<&str>,
) -> anyhow::Result<()> {
    let has_terminal = fs::read_to_string(events_path)
        .map(|events| {
            events
                .lines()
                .any(|line| line.contains("\"event\":\"run_stop\""))
        })
        .unwrap_or(false);
    if !has_terminal {
        emit(
            events_path,
            json!({"event":"run_stop","verdict":verdict,"reason":reason}),
        )?;
    }
    Ok(())
}

// Workflow child Config provenance audit (D-3a-3c, audited at e977ce6).
// This table is intentionally exhaustive: every top-level Config field and
// every ConfigFieldSources member has one owner. `wrong` records the pre-fix
// state found by circle-001; later commits must satisfy the stated invariant.
//
// Config field                       source/invariant                         audit
// workspace_root                     origin-derived                           correct
// state_dir                          origin-derived, inside node run           wrong
// eval_events_path                   origin-derived, exact node events path    wrong
// completion_contract_path           global inheritance                       correct
// yes                                fixed true                               correct
// offline                            global inheritance                       correct
// context_budget                     global inheritance                       correct
// model                              node declaration or global inheritance   correct
// provider                           node declaration or global inheritance   correct
// prompt_layout                      global inheritance                       correct
// plan_preset                        node intent/profile default; explicit     wrong
//                                    global override is preserved
// intent_override                    node declaration                         correct
// planner_model                      global inheritance                       correct
// planner_provider                   global inheritance                       correct
// ollama_host                        global inheritance                       correct
// num_predict                        global inheritance                       correct
// max_iterations                     global inheritance                       correct
// chat_timeout_secs                  global inheritance                       correct
// chat_timeout_source                global inheritance                       correct
// field_sources                      fixed from the member rules below        wrong
// chat_retries                       global inheritance                       correct
// stream                             global inheritance                       correct
// resume                             fixed None                               wrong
// fresh_session                      fixed true                               wrong
// no_footer                          global inheritance                       correct
// narration                          global inheritance                       correct
// profile                            node declaration                         wrong
// profile_explicit                   fixed true (node declaration)            wrong
// profile_inference                  fixed None (node declaration)            wrong
// style                              global inheritance                       correct
// action                             origin-derived goal + node intent Prompt wrong
// field_sources.model                node declaration or global inheritance   correct
// field_sources.provider             node declaration or global inheritance   correct
// field_sources.planner_model        global inheritance                       correct
// field_sources.planner_provider     global inheritance                       correct
// field_sources.context_budget       global inheritance                       correct
// field_sources.chat_timeout_secs    global inheritance                       correct
// field_sources.prompt_layout        global inheritance                       correct
// field_sources.plan_preset          node default or explicit global source   wrong
// field_sources.profile              fixed workflow_node                      wrong
// field_sources.narration            global inheritance                       correct
// field_sources.footer               global inheritance                       correct
// field_sources.stream               global inheritance                       correct
//
// Audit total: 43 leaf/container rows; 31 correct, 0 missing, 12 wrong.
//
// Invocation surface outside Config (D-3a-3d, audited after circle-002):
// surface/parameter                 source/invariant                         audit
// action selector / ultra_plan_run fixed Action::UltraPlanRun               correct
// single-intent execution entry    run_resolved_config_for_workflow          correct
// panic boundary                   outer workflow CLI boundary only          correct
// node request                     route-derived goal/intent/carry/config    correct
// external reproducer binding      route carry -> origin-confined state file correct
// workflow child marker            fixed origin-confined state file          correct
// node identity                    origin-confined allocated UUID/run paths  correct
// interaction mode                 fixed non-interactive via Config.yes      correct
// workflow timeout                 none; node budget remains sole bound      correct
// extra scalar invocation args     none; the production entry takes Config  correct
//
// This section is part of the permanent exhaustive audit boundary. Any change
// that adds or changes a parameter outside Config at the workflow child call
// surface must update this table in the same change.
fn node_config(
    config: &Config,
    node: &Node,
    request: &runner::NodeRunRequest,
    identity: &NodeRunIdentity,
) -> Result<Config, String> {
    let mut child = config.clone();
    child.yes = true;
    child.workspace_root = request.origin.clone();
    if child.workspace_root.canonicalize().ok().as_deref()
        != request.origin.canonicalize().ok().as_deref()
    {
        return Err("workspace_confinement_violation".into());
    }
    child.eval_events_path = Some(identity.events_path.clone());
    child.state_dir = identity.state_dir.clone();
    child.resume = None;
    child.fresh_session = true;
    let canonical_origin = request
        .origin
        .canonicalize()
        .map_err(|_| "workspace_confinement_violation".to_string())?;
    let event_parent = identity
        .events_path
        .parent()
        .and_then(|path| path.canonicalize().ok())
        .ok_or_else(|| "workspace_confinement_violation".to_string())?;
    let state_dir = child
        .state_dir
        .canonicalize()
        .map_err(|_| "workspace_confinement_violation".to_string())?;
    if !event_parent.starts_with(&canonical_origin) || !state_dir.starts_with(&canonical_origin) {
        return Err("workspace_confinement_violation".into());
    }
    child.action = Action::UltraPlanRun(request.goal.clone());
    child.intent_override = Some(match request.intent.as_str() {
        "investigate" => IntentId::Investigate,
        "fix" => IntentId::Fix,
        _ => IntentId::Create,
    });
    child.profile = node.profile.clone();
    child.profile_explicit = true;
    child.profile_inference = None;
    child.field_sources.profile = "workflow_node".into();
    if child.field_sources.plan_preset.starts_with("default") {
        match (node.intent, node.profile.as_str()) {
            (Intent::Investigate, "data") => {
                child.plan_preset = PlanPreset::Profile;
                child.field_sources.plan_preset = "default_investigate_data".into();
            }
            (Intent::Fix, "data") => {
                child.plan_preset = PlanPreset::Profile;
                child.field_sources.plan_preset = "default_fix_data".into();
            }
            _ => {}
        }
    }
    if let (Some(model), Some(provider)) = (&node.model, node.provider) {
        child.model = model.clone();
        child.provider = provider;
        child.field_sources.model = "workflow_node".into();
        child.field_sources.provider = "workflow_node".into();
    }
    Ok(child)
}

fn execute_configured_node<F>(
    config: &Config,
    node: &Node,
    request: &runner::NodeRunRequest,
    identity: &NodeRunIdentity,
    execute: F,
) -> Result<(), String>
where
    F: FnOnce(Config) -> Result<(), String>,
{
    let child = node_config(config, node, request, identity)?;
    crate::planner::external_reproducer::mark_workflow_node(&child)?;
    if let Some(binding) = &request.reproducer {
        crate::planner::external_reproducer::write(&child, binding)?;
        crate::eval_events::emit(
            child.eval_events_path.as_deref(),
            json!({
                "event":"workflow_reproducer_bound",
                "basis":binding.basis,
                "command":binding.command,
                "lineage":binding.lineage,
            }),
        );
    }
    execute(child)
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
fn write_circle(origin: &Path, evidence: &WorkflowCircleEvidence) -> anyhow::Result<()> {
    let p = origin.join("evidence/workflow-circle.json");
    evidence.write_to(&p).map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::adjudication::contract::ProbeOutcome;
    use crate::planner::external_reproducer::ExternalReproducerBinding;
    use crate::providers::{AssistantReply, ChatClient};
    use crate::state::ConversationMessage;
    use crate::tools::registry::ToolSpec;
    use clap::Parser;

    #[derive(Clone)]
    struct FailingClient;

    impl ChatClient for FailingClient {
        fn label(&self) -> &str {
            "workflow-mode-test"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            anyhow::bail!("stop after reaching the UltraPlan phase seam")
        }
    }

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
            profile: "data".into(),
            goal: "goal".into(),
            origin: root.to_path_buf(),
            reproducer: None,
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

    fn identity(root: &Path) -> NodeRunIdentity {
        NodeRunIdentity::allocate(root).unwrap()
    }

    #[test]
    fn node_executor_override_propagates_to_config_and_existing_events() {
        let root = tempfile::tempdir().unwrap();
        let global = config(root.path());
        let node = node(Some("elevated-model"), Some(Provider::Gemini));
        let request = request(root.path(), "elevated-model", Provider::Gemini);
        let identity = identity(root.path());

        let child = node_config(&global, &node, &request, &identity).unwrap();
        assert_eq!(child.model, "elevated-model");
        assert_eq!(child.provider, Provider::Gemini);
        assert_eq!(child.field_sources.model, "workflow_node");
        assert_eq!(child.field_sources.provider, "workflow_node");
        assert_eq!(child.profile, "data");
        assert!(child.profile_explicit);
        assert!(child.profile_inference.is_none());
        assert_eq!(child.field_sources.profile, "workflow_node");
        assert_eq!(child.plan_preset, PlanPreset::Profile);
        assert_eq!(child.field_sources.plan_preset, "default_investigate_data");

        let events = root.path().join("evidence/investigate-events.jsonl");
        runner::execute_node(&request, &events, |_| Ok(())).unwrap();
        let event: serde_json::Value =
            serde_json::from_str(std::fs::read_to_string(events).unwrap().trim()).unwrap();
        assert_eq!(event["event"], "intent_resolved");
        assert_eq!(event["model"], "elevated-model");
        assert_eq!(event["provider"], "gemini");
        assert_eq!(event["profile"], "data");
    }

    #[test]
    fn omitted_node_executor_preserves_global_config() {
        let root = tempfile::tempdir().unwrap();
        let global = config(root.path());
        let node = node(None, None);
        let request = request(root.path(), &global.model, global.provider);
        let identity = identity(root.path());

        let child = node_config(&global, &node, &request, &identity).unwrap();
        assert_eq!(child.model, global.model);
        assert_eq!(child.provider, global.provider);
        assert_eq!(child.field_sources.model, global.field_sources.model);
        assert_eq!(child.field_sources.provider, global.field_sources.provider);
    }

    #[test]
    fn workflow_node_execution_seam_uses_ultra_plan_run_and_reaches_investigation_synthesis() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::write(
            root.path().join("pipeline/main.py"),
            "raise ValueError('origin failure')\n",
        )
        .unwrap();
        let global = config(root.path());
        let node = node(None, None);
        let mut request = request(root.path(), &global.model, global.provider);
        request.goal = "derived workflow goal without reproducer vocabulary".into();
        request.reproducer = Some(
            ExternalReproducerBinding::new(
                "origin_workspace:pipeline_probe",
                "python3 -B pipeline/main.py",
            )
            .unwrap(),
        );
        let identity = identity(root.path());

        runner::execute_node(
            &request,
            &root.path().join("evidence/investigate-events.jsonl"),
            |req| {
                execute_configured_node(&global, &node, req, &identity, |child| {
                    assert!(matches!(
                        &child.action,
                        Action::UltraPlanRun(goal) if goal == &request.goal
                    ));
                    let plan = crate::planner::intent::explicit_investigation_plan(
                        &request.goal,
                        "data",
                        "default",
                    );
                    let mut planner = FailingClient;
                    let mut execution = FailingClient;
                    let error = crate::planner::runner::run_ultra_plan(
                        &mut planner,
                        &mut execution,
                        &plan,
                        &child,
                    )
                    .unwrap_err();
                    assert!(
                        error
                            .to_string()
                            .contains("stop after reaching the UltraPlan phase seam")
                    );
                    Ok(())
                })
            },
        )
        .unwrap();

        let events = std::fs::read_to_string(identity.events_path).unwrap();
        assert!(events.contains("\"event\":\"workflow_reproducer_bound\""));
        assert!(events.contains("\"event\":\"ultra_phase_start\""));
        assert!(events.contains("\"event\":\"investigation_plan_synthesized\""));
        assert!(events.contains("\"r_basis\":\"origin_workspace:pipeline_probe\""));
        assert!(events.contains("\"profile\":\"data\""));
        let run: crate::planner::adjudication::investigate::InvestigationRunEvidence =
            serde_json::from_slice(
                &std::fs::read(root.path().join("evidence/investigation-run.json")).unwrap(),
            )
            .unwrap();
        assert_eq!(run.reproducer, "python3 -B pipeline/main.py");
        assert_eq!(run.reproducer_lineage, request.reproducer.unwrap().lineage);
        assert_eq!(run.outcome, ProbeOutcome::Failure);
    }

    #[test]
    fn node_run_identity_is_the_actual_origin_event_directory() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let origin = temp.path().join("origin");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&origin).unwrap();
        let global = config(&repo);
        let node = node(None, None);
        let request = request(&origin, &global.model, global.provider);
        let identity = identity(&origin);

        emit_node_run_created(&identity, "investigate", &request).unwrap();
        let child = node_config(&global, &node, &request, &identity).unwrap();

        assert_eq!(
            child.eval_events_path.as_deref(),
            Some(identity.events_path.as_path())
        );
        assert_eq!(child.state_dir, identity.state_dir);
        assert!(child.fresh_session);
        assert!(child.resume.is_none());
        assert_eq!(
            identity.run_dir.file_name().unwrap().to_string_lossy(),
            identity.run_id
        );
        assert!(uuid::Uuid::parse_str(&identity.run_id).is_ok());
        let created: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(&identity.events_path)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(created["run_id"], identity.run_id);
        assert_eq!(
            created["run_dir"],
            identity.run_dir.to_string_lossy().as_ref()
        );
    }

    #[test]
    fn node_output_roots_are_confined_and_repo_tree_stays_unchanged() {
        fn files_under(root: &Path) -> Vec<PathBuf> {
            fn visit(path: &Path, out: &mut Vec<PathBuf>) {
                let Ok(entries) = std::fs::read_dir(path) else {
                    return;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        visit(&path, out);
                    } else {
                        out.push(path);
                    }
                }
            }
            let mut files = Vec::new();
            visit(root, &mut files);
            files.sort();
            files
        }

        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let origin = temp.path().join("origin");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&origin).unwrap();
        let global = config(&repo);
        let node = node(None, None);
        let request = request(&origin, &global.model, global.provider);
        let identity = identity(&origin);
        let child = node_config(&global, &node, &request, &identity).unwrap();
        let repo_before = files_under(&repo);

        runner::execute_node(
            &request,
            &origin.join("evidence/investigate-events.jsonl"),
            |_| {
                crate::eval_events::emit(
                    child.eval_events_path.as_deref(),
                    json!({"event":"test_node_event"}),
                );
                std::fs::write(child.state_dir.join("session.json"), "{}").unwrap();
                let plan_dir = child.workspace_root.join(".anvil/plans");
                std::fs::create_dir_all(&plan_dir).unwrap();
                std::fs::write(plan_dir.join("node-plan.yaml"), "version: 1\n").unwrap();
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(files_under(&repo), repo_before);
        for path in files_under(&origin) {
            assert!(path.starts_with(&origin));
        }
        assert!(identity.events_path.is_file());
        assert!(child.state_dir.join("session.json").is_file());
        assert!(origin.join(".anvil/plans/node-plan.yaml").is_file());
    }

    #[test]
    fn node_config_rejects_event_or_state_roots_outside_origin() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let origin = temp.path().join("origin");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&origin).unwrap();
        let global = config(&repo);
        let node = node(None, None);
        let request = request(&origin, &global.model, global.provider);
        let outside = identity(&repo);

        assert_eq!(
            node_config(&global, &node, &request, &outside).unwrap_err(),
            "workspace_confinement_violation"
        );
    }

    #[test]
    fn underivable_origin_goal_adjudicates_without_starting_a_node() {
        let root = tempfile::tempdir().unwrap();
        let origin = root.path().join("origin");
        let run_dir = origin.join(".anvil/runs/origin-run");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::create_dir_all(origin.join(".anvil/plans")).unwrap();
        std::fs::write(
            run_dir.join("events.jsonl"),
            r#"{"event":"run_start","action":"Repl"}
{"event":"run_stop","status":"failed"}"#,
        )
        .unwrap();
        std::fs::write(
            origin.join(".anvil/plans/recovery-origin.yaml"),
            "version: 1\n",
        )
        .unwrap();
        let definition =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("workflows/recovery-circle-data.yaml");

        run_workflow(&config(&origin), &definition, &origin).unwrap();

        let circle: serde_json::Value = serde_json::from_slice(
            &std::fs::read(origin.join("evidence/workflow-circle.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(circle["verdict"], "circle_failed");
        assert_eq!(circle["reason"], "origin_goal_underivable");
        let workflow_events =
            std::fs::read_to_string(origin.join("evidence/workflow-events.jsonl")).unwrap();
        assert!(workflow_events.contains("\"reason\":\"origin_goal_underivable\""));
        assert!(!workflow_events.contains("workflow_node_started"));
        assert_eq!(
            std::fs::read_dir(origin.join(".anvil/runs"))
                .unwrap()
                .count(),
            1
        );
    }
}
