//! IC-44: a contract-mode editor that saves an existing record leaves the commit
//! keys empty, because typing and saving change the same key. The probe may then
//! promote persistence to `preserved` only on the full commit witness: a fresh
//! token, a save the probe's own press started, an independent display after the
//! reload, and a read of the saved resource that still holds the token. Every
//! scenario runs the real probe script against the fake Playwright module of the
//! `editor-save-committed-readback` corpus case, whose page emits each request,
//! response and request failure through the probe's own listeners when the app
//! would send or receive it. Each negative keeps every other part of the witness
//! satisfied, so only the part it changes can stop the promotion.

use std::path::Path;

use serde_json::Value;
use tempfile::TempDir;

use super::tests::{FAKE_PROBE_SCENARIO_BUDGET, node_available, spawn_probe_http_server};
use super::{
    BrowserInteractionObservation, BrowserInteractionProbeOptions, PlaywrightResolution,
    probe_browser_interaction_against_running_server_with_options,
    write_test_availability_override_with_resolution,
};
use crate::planner::profiles::nextjs::route_reachability;

const FAKE_COMMIT_PLAYWRIGHT: &str = include_str!(
    "../../../tests/corpus/apps/editor-save-committed-readback/fixtures/fake-playwright.cjs"
);
const SCENARIOS: &str = include_str!(
    "../../../tests/corpus/apps/editor-save-committed-readback/fixtures/scenarios.json"
);
const RECORDED_MARKERS: &str = include_str!(
    "../../../tests/corpus/apps/editor-save-committed-readback/fixtures/recorded-markers.json"
);
const UNREACHABLE_ENTRY_PAGE: &str =
    include_str!("../../../tests/corpus/apps/nextjs-route-unreachable-from-entry/src/app/page.tsx");
const UNREACHABLE_NOTE_PAGE: &str = include_str!(
    "../../../tests/corpus/apps/nextjs-route-unreachable-from-entry/src/app/notes/[id]/page.tsx"
);

const WITNESS_STEPS: [&str; 3] = [
    "persistence_commit_http_observed",
    "persistence_committed_surface_observed",
    "persistence_committed_readback_observed",
];

struct CommitRun {
    observation: BrowserInteractionObservation,
    evidence: Value,
    root: TempDir,
}

fn install_commit_playwright(root: &Path, scenario: &str) -> PlaywrightResolution {
    let node_modules = root.join("node_modules");
    let playwright_dir = node_modules.join("playwright");
    std::fs::create_dir_all(&playwright_dir).unwrap();
    std::fs::write(
        playwright_dir.join("package.json"),
        "{\"version\":\"0.0.0-fake\"}\n",
    )
    .unwrap();
    std::fs::write(
        playwright_dir.join("index.js"),
        FAKE_COMMIT_PLAYWRIGHT.replace("__SCENARIO__", scenario),
    )
    .unwrap();
    PlaywrightResolution {
        module_path: playwright_dir.join("index.js").display().to_string(),
        module_dir: playwright_dir.display().to_string(),
        node_path: Some(node_modules.display().to_string()),
        location: "fake_playwright".to_string(),
        version: "0.0.0-fake".to_string(),
    }
}

/// Runs the real probe with persistence, text entry and token echo required, in a
/// tempdir that also holds `app_files`. `None` when node or loopback networking is
/// unavailable.
fn run_commit_scenario_with_app(scenario: &str, app_files: &[(&str, &str)]) -> Option<CommitRun> {
    if !node_available() {
        eprintln!("skipping persistence commit scenario because node is unavailable");
        return None;
    }
    let root = tempfile::tempdir().unwrap();
    for (path, text) in app_files {
        let target = root.path().join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, text).unwrap();
    }
    let resolution = install_commit_playwright(root.path(), scenario);
    write_test_availability_override_with_resolution(root.path(), true, Some(&resolution));
    let (port, server) = spawn_probe_http_server()?;
    let run_dir = root.path().join(".anvil/runs/fake-playwright");
    let path = run_dir.join("browser-interaction.json");
    let outcome = probe_browser_interaction_against_running_server_with_options(
        root.path(),
        port,
        &run_dir,
        &path,
        FAKE_PROBE_SCENARIO_BUDGET,
        BrowserInteractionProbeOptions {
            persistence_required: true,
            text_entry_required: true,
            token_echo_required: true,
        },
    );
    let _ = server.join();
    let observation = outcome.observation().cloned()?;
    let evidence = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    Some(CommitRun {
        observation,
        evidence,
        root,
    })
}

fn run_commit_scenario(scenario: &str) -> Option<CommitRun> {
    run_commit_scenario_with_app(scenario, &[])
}

fn scenario_table() -> Vec<Value> {
    let value: Value = serde_json::from_str(SCENARIOS).unwrap();
    value["scenarios"].as_array().unwrap().clone()
}

/// The verdict and witness steps `scenarios.json` declares for `scenario`.
fn declared(scenario: &str) -> (String, Vec<String>) {
    let entry = scenario_table()
        .into_iter()
        .find(|entry| entry["name"] == scenario)
        .unwrap_or_else(|| panic!("{scenario} is not declared in scenarios.json"));
    let steps = entry["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step.as_str().unwrap().to_string())
        .collect();
    (entry["verdict"].as_str().unwrap().to_string(), steps)
}

fn witness_steps(observation: &BrowserInteractionObservation) -> Vec<String> {
    observation
        .steps
        .iter()
        .filter(|step| WITNESS_STEPS.contains(&step.as_str()))
        .cloned()
        .collect()
}

fn has_step(observation: &BrowserInteractionObservation, step: &str) -> bool {
    observation.steps.iter().any(|candidate| candidate == step)
}

fn has_kind(observation: &BrowserInteractionObservation, kind: &str) -> bool {
    observation
        .informational_failure_kinds
        .iter()
        .any(|candidate| candidate == kind)
}

/// Runs `scenario` and checks the verdict and witness steps it declares. Commit
/// evidence is never reported as changed state keys.
fn assert_declared_scenario(scenario: &str) -> Option<CommitRun> {
    let run = run_commit_scenario(scenario)?;
    let observation = &run.observation;
    let (verdict, steps) = declared(scenario);
    assert_eq!(
        observation.probe_mode, "contract",
        "{scenario}: {observation:?}"
    );
    assert_eq!(
        observation.text_entry, "entered",
        "{scenario}: {observation:?}"
    );
    assert_eq!(
        observation.persistence_after_reload, verdict,
        "{scenario}: {observation:?}"
    );
    assert_eq!(
        run.evidence["persistence_after_reload"], verdict,
        "{scenario}"
    );
    assert_eq!(
        witness_steps(observation),
        steps,
        "{scenario}: {observation:?}"
    );
    assert!(
        observation.persistence_changed_dimensions.is_empty(),
        "{scenario}: commit evidence must not be reported as state keys: {observation:?}"
    );
    if verdict == "preserved" {
        assert!(observation.ok, "{scenario}: {observation:?}");
        assert_eq!(observation.failure_kind, "", "{scenario}: {observation:?}");
        assert_eq!(
            observation.persistence_after_reload_reason, "",
            "{scenario}: {observation:?}"
        );
        assert!(has_step(observation, "persistence_reload"), "{scenario}");
        assert!(
            !has_kind(observation, "persistence_draft_only"),
            "{scenario}: {observation:?}"
        );
    } else {
        assert!(!observation.ok, "{scenario}: {observation:?}");
        assert_ne!(observation.failure_kind, "", "{scenario}: {observation:?}");
    }
    if verdict == "not_evaluated" {
        assert!(
            has_step(observation, "persistence_reload:not_evaluated"),
            "{scenario}: {observation:?}"
        );
    }
    Some(run)
}

/// A negative whose save press, display and read leave persistence exactly as
/// the probe reported it before the witness existed.
fn assert_not_promoted(scenario: &str) -> Option<CommitRun> {
    let run = assert_declared_scenario(scenario)?;
    let observation = &run.observation;
    assert_eq!(
        observation.persistence_after_reload, "not_evaluated",
        "{scenario}: {observation:?}"
    );
    assert_eq!(
        observation.persistence_after_reload_reason, "no_mutation_observed",
        "{scenario}: {observation:?}"
    );
    assert!(
        observation
            .failure_kind
            .starts_with("persistence_not_evaluated:")
            || observation
                .failure_kind
                .starts_with("http_mutation_failed:"),
        "{scenario}: {observation:?}"
    );
    Some(run)
}

fn recorded(source: &str, marker: &str, token: &str) -> String {
    let value: Value = serde_json::from_str(RECORDED_MARKERS).unwrap();
    value[source][marker]["value"]
        .as_str()
        .unwrap_or_else(|| panic!("{source}.{marker} is not recorded"))
        .replace("{token}", token)
}

#[test]
fn scenario_table_covers_every_fake_scenario_once() {
    let names = scenario_table()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), names.len(), "{names:?}");
    for name in &names {
        assert!(
            FAKE_COMMIT_PLAYWRIGHT.contains(&format!("\n  {name}: {{")),
            "{name} has no scenario in fake-playwright.cjs"
        );
    }
    let preserved = scenario_table()
        .iter()
        .filter(|entry| entry["verdict"] == "preserved")
        .map(|entry| entry["name"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(preserved, ["recorded_content_q", "synthetic_renamed"]);
}

#[test]
fn recorded_content_q_editor_save_is_preserved_by_the_commit_witness() {
    let Some(run) = assert_declared_scenario("recorded_content_q") else {
        return;
    };
    let observation = &run.observation;
    let token = observation.typed_token.as_str();
    assert!(token.starts_with("anvil-"), "{observation:?}");
    for (marker, actual) in [
        ("input_before_marker", &observation.input_before_marker),
        ("input_after_marker", &observation.input_after_marker),
        (
            "persistence_before_reload_marker",
            &observation.persistence_before_reload_marker,
        ),
        (
            "persistence_after_reload_marker",
            &observation.persistence_after_reload_marker,
        ),
    ] {
        assert_eq!(
            actual,
            &recorded("content_q", marker, token),
            "{marker}: {observation:?}"
        );
    }
    let mutations = observation
        .http_mutation_observations
        .iter()
        .map(|mutation| (mutation.method.as_str(), mutation.status))
        .collect::<Vec<_>>();
    assert_eq!(
        &mutations[..2],
        &[("POST", 201), ("PUT", 200)],
        "{observation:?}"
    );
}

#[test]
fn synthetic_editor_with_other_state_keys_and_paths_is_preserved_by_the_commit_witness() {
    let Some(run) = assert_declared_scenario("synthetic_renamed") else {
        return;
    };
    let markers = &run.observation.persistence_after_reload_marker;
    assert!(markers.contains("\"screen\":\"index\""), "{markers}");
    assert!(!markers.contains("noteCount"), "{markers}");
}

#[test]
fn l135_editor_that_stays_after_reload_is_not_promoted_despite_put_200_and_readback() {
    let Some(run) = assert_not_promoted("l135_editor_after_reload") else {
        return;
    };
    let observation = &run.observation;
    let token = observation.typed_token.as_str();
    for (marker, actual) in [
        ("input_before_marker", &observation.input_before_marker),
        ("input_after_marker", &observation.input_after_marker),
        (
            "persistence_before_reload_marker",
            &observation.persistence_before_reload_marker,
        ),
        (
            "persistence_after_reload_marker",
            &observation.persistence_after_reload_marker,
        ),
    ] {
        assert_eq!(
            actual,
            &recorded("content_g", marker, token),
            "{marker}: {observation:?}"
        );
    }
    assert!(
        observation
            .http_mutation_observations
            .iter()
            .any(|mutation| mutation.method == "PUT" && mutation.status == 200),
        "{observation:?}"
    );
    assert!(
        has_kind(observation, "persistence_draft_only"),
        "{observation:?}"
    );
    assert_eq!(
        observation.failure_kind,
        "persistence_not_evaluated:no_mutation_observed"
    );
}

#[test]
fn save_api_without_the_typed_change_is_not_promoted() {
    assert_not_promoted("save_without_change");
}

#[test]
fn save_press_that_only_clears_dirty_is_not_promoted() {
    assert_not_promoted("dirty_only");
}

#[test]
fn matching_record_count_alone_is_not_promoted() {
    let Some(run) = assert_not_promoted("count_only") else {
        return;
    };
    let observation = &run.observation;
    assert!(
        observation
            .persistence_before_reload_marker
            .contains("\"noteCount\":3")
            && observation
                .persistence_after_reload_marker
                .contains("\"noteCount\":3"),
        "{observation:?}"
    );
}

#[test]
fn old_probe_token_in_the_list_and_read_is_not_promoted() {
    assert_not_promoted("stale_token");
}

#[test]
fn only_the_first_creating_post_is_not_promoted() {
    assert_not_promoted("create_post_only");
}

#[test]
fn autosave_while_typing_is_not_attributed_to_the_save_press() {
    assert_not_promoted("autosave_only");
}

#[test]
fn late_response_of_a_request_started_before_the_press_is_not_attributed() {
    assert_not_promoted("late_response");
}

#[test]
fn save_to_another_origin_is_not_promoted() {
    assert_not_promoted("cross_origin");
}

#[test]
fn save_to_a_resource_the_read_does_not_name_is_not_promoted() {
    assert_not_promoted("other_resource");
}

#[test]
fn read_after_reload_without_the_token_is_not_promoted() {
    assert_not_promoted("readback_without_token");
}

#[test]
fn list_without_the_token_is_not_promoted() {
    assert_not_promoted("list_without_token");
}

#[test]
fn token_only_inside_a_contenteditable_after_reload_is_not_promoted() {
    assert_not_promoted("entry_only_after_reload");
}

#[test]
fn preview_of_a_local_draft_after_reload_is_not_promoted() {
    assert_not_promoted("preview_only");
}

#[test]
fn token_only_in_a_hidden_node_after_reload_is_not_promoted() {
    assert_not_promoted("hidden_only");
}

#[test]
fn local_storage_draft_rendered_as_a_list_is_not_promoted() {
    assert_not_promoted("local_storage_draft");
}

#[test]
fn save_answering_4xx_is_not_promoted_and_keeps_the_http_failure_first() {
    let Some(run) = assert_not_promoted("http_4xx") else {
        return;
    };
    assert_eq!(run.observation.failure_kind, "http_mutation_failed:PUT:409");
}

#[test]
fn save_answering_5xx_is_not_promoted_and_keeps_the_http_failure_first() {
    let Some(run) = assert_not_promoted("http_5xx") else {
        return;
    };
    assert_eq!(run.observation.failure_kind, "http_mutation_failed:PUT:500");
}

#[test]
fn failed_save_request_is_not_promoted() {
    assert_not_promoted("request_failed");
}

#[test]
fn failed_reload_is_not_promoted() {
    let Some(run) = assert_declared_scenario("reload_failed") else {
        return;
    };
    let observation = &run.observation;
    assert_eq!(
        observation.persistence_after_reload_reason, "reload_failed",
        "{observation:?}"
    );
    assert!(
        has_kind(observation, "persistence_reload_not_evaluated"),
        "{observation:?}"
    );
}

#[test]
fn save_that_never_answers_is_not_promoted() {
    assert_not_promoted("save_timeout");
}

#[test]
fn token_present_before_typing_is_not_promoted() {
    assert_not_promoted("token_before_entry");
}

#[test]
fn token_echoed_in_the_page_url_is_not_promoted() {
    assert_not_promoted("url_echo");
}

#[test]
fn read_of_the_saved_resource_that_lost_the_token_is_reset() {
    let Some(run) = assert_declared_scenario("readback_exact_without_token") else {
        return;
    };
    assert_eq!(
        run.observation.failure_kind, "persistence_after_reload_reset",
        "{:?}",
        run.observation
    );
}

/// The witness clears only the persistence verdict: a route the entry cannot
/// reach still fails the reachability check on the same app root.
#[test]
fn preserved_commit_witness_leaves_route_unreachability_failed() {
    let Some(run) = run_commit_scenario_with_app(
        "recorded_content_q",
        &[
            ("src/app/page.tsx", UNREACHABLE_ENTRY_PAGE),
            ("src/app/notes/[id]/page.tsx", UNREACHABLE_NOTE_PAGE),
        ],
    ) else {
        return;
    };
    assert_eq!(
        run.observation.persistence_after_reload, "preserved",
        "{:?}",
        run.observation
    );
    let report = route_reachability::evaluate(run.root.path());
    let (status, reasons) = route_reachability::merge_with_testimony(false, Vec::new(), &report);
    assert_eq!(status, "failed", "{reasons:?}");
    assert!(
        reasons
            .iter()
            .any(|reason| reason == "route_unreachable_from_entry:/notes/[id]"),
        "{reasons:?}"
    );
}
