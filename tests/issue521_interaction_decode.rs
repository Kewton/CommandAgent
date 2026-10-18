//! Issue #725 (parent #521, S05): pin the JSON-to-observation decision of the
//! browser interaction probe through its public entry point only.
//!
//! A fake `node` shell script is installed under `<root>/node_modules/.bin`
//! (the first entry of the sanitized probe PATH). It answers the Playwright
//! module resolution and then copies a fixture JSON into the evidence path, so
//! no real browser, network, or terminal is touched. Every file lives inside a
//! tempdir. The expectations were recorded on the pre-extraction baseline and
//! must hold unchanged after the decode logic moves into a leaf module.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use commandagent::minimal_loop::interaction_probe::{
    BrowserInteractionObservation, InteractionProbeOutcome,
    probe_browser_interaction_against_running_server,
};
use serde_json::Value;

const SETUP_REMEDIATION: &str = "run /setup-interaction-probe (or commandagent --setup-interaction-probe) to enable interaction release checks";

struct Run {
    observation: BrowserInteractionObservation,
    evidence: Value,
}

/// Runs the probe against a fake node. `fixture` is the raw evidence text the
/// fake script writes (None: write nothing); `exit_code` is its exit status.
fn run_probe(fixture: Option<&str>, exit_code: i32) -> Run {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("workspace");
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).expect("bin dir");
    let run_dir = dir.path().join("run");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    let evidence_path = run_dir.join("browser-interaction.json");

    let fixture_path = dir.path().join("fixture.json");
    let copy = match fixture {
        Some(text) => {
            std::fs::write(&fixture_path, text).expect("fixture");
            format!("cp '{}' \"$3\"\n", fixture_path.display())
        }
        None => String::new(),
    };
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"-e\" ]; then\n  printf '%s\\n' '{{\"path\":\"/fake/node_modules/playwright/index.js\",\"version\":\"0.0.0-fake\"}}'\n  exit 0\nfi\n{copy}exit {exit_code}\n"
    );
    let node = bin.join("node");
    std::fs::write(&node, script).expect("fake node");
    std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let outcome = probe_browser_interaction_against_running_server(
        &root,
        4173,
        &run_dir,
        &evidence_path,
        Duration::from_secs(20),
    );
    let InteractionProbeOutcome::Observation(observation) = outcome else {
        panic!("expected an observation, got {outcome:?}");
    };
    let evidence = std::fs::read_to_string(&evidence_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .expect("mirrored evidence json");
    Run {
        observation: *observation,
        evidence,
    }
}

fn remediation_label(remediation: &str) -> &str {
    if remediation == SETUP_REMEDIATION {
        "SETUP"
    } else {
        remediation
    }
}

/// One line per case: the decision, wording, and the mirrored evidence fields.
fn core(run: &Run) -> String {
    let o = &run.observation;
    let text = |key: &str| {
        run.evidence
            .get(key)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "absent".to_string())
    };
    format!(
        "ok={} status={:?} kind={:?} stage={:?} error={:?} rem={:?} steps={:?} \
chg={}/{} eval={} ctl={} prim={} rec={}/{} spawned={} reaped={} \
ev[status={} cat={} rec={} prim_missing={}]",
        o.ok,
        o.status,
        o.failure_kind,
        o.stage,
        o.error,
        remediation_label(&o.remediation),
        o.steps,
        o.input_state_changed,
        o.input_contract_state_changed,
        o.input_state_evaluated_after_start,
        o.start_control_found,
        o.primary_transition_observed,
        o.recovery_transition_observed,
        o.recovery_transition_not_observed,
        o.child_spawned,
        o.child_reaped,
        text("status"),
        text("failure_category"),
        text("recovery_transition_status"),
        text("primary_start_transition_missing"),
    )
}

/// Case: (name, evidence text written by the fake node, its exit status).
type Case<'a> = (&'a str, Option<&'a str>, i32);

fn table(cases: &[Case]) -> String {
    cases
        .iter()
        .map(|(name, fixture, exit_code)| {
            format!("{name}: {}", core(&run_probe(*fixture, *exit_code)))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

const STEPS_OK: &str =
    r#""steps":["start_transition","input_state_evaluated_after_start","input_state_change"]"#;

#[test]
fn complete_observation_passes() {
    let fixture = format!(r#"{{"ok":true,{STEPS_OK},"before_marker":"a","after_marker":"b"}}"#);
    let run = run_probe(Some(&fixture), 0);
    assert_eq!(
        core(&run),
        "ok=true status=\"passed\" kind=\"\" stage=\"\" error=\"\" rem=\"\" \
steps=[\"start_transition\", \"input_state_evaluated_after_start\", \"input_state_change\"] \
chg=true/true eval=true ctl=true prim=true rec=false/false spawned=true reaped=true \
ev[status=\"passed\" cat=\"\" rec=\"unknown\" prim_missing=false]"
    );
    assert_eq!(run.observation.before_marker, "a");
    assert_eq!(run.observation.after_marker, "b");
    assert_eq!(run.observation.probe_mode, "heuristic");
    assert_eq!(run.observation.contract_hook_status, "unknown");
    assert_eq!(run.observation.url, "http://127.0.0.1:4173/");
}

#[test]
fn missing_null_false_and_non_bool_ok_are_never_success() {
    let missing = format!("{{{STEPS_OK}}}");
    let null = format!(r#"{{"ok":null,{STEPS_OK}}}"#);
    let string = format!(r#"{{"ok":"true",{STEPS_OK}}}"#);
    let number = format!(r#"{{"ok":1,{STEPS_OK}}}"#);
    let falsy = format!(r#"{{"ok":false,{STEPS_OK}}}"#);
    let actual = table(&[
        ("missing_ok", Some(&missing), 0),
        ("null_ok", Some(&null), 0),
        ("string_ok", Some(&string), 0),
        ("number_ok", Some(&number), 0),
        ("false_ok", Some(&falsy), 0),
        ("empty_object", Some("{}"), 0),
    ]);
    assert_eq!(actual, EXPECT_OK_NEGATIVES);
    assert!(
        !actual.contains("ok=true"),
        "a non-true ok must never decode as success"
    );
}

#[test]
fn unobserved_start_or_input_state_is_never_success() {
    let actual = table(&[
        (
            "no_start_transition",
            Some(
                r#"{"ok":true,"steps":["input_state_evaluated_after_start","input_state_change"]}"#,
            ),
            0,
        ),
        (
            "start_transition_false",
            Some(
                r#"{"ok":true,"start_transition":false,"steps":["input_state_evaluated_after_start","input_state_change"]}"#,
            ),
            0,
        ),
        (
            "not_evaluated_after_start",
            Some(r#"{"ok":true,"steps":["start_transition","input_state_change"]}"#),
            0,
        ),
        (
            "evaluated_false",
            Some(
                r#"{"ok":true,"input_state_evaluated_after_start":false,"steps":["start_transition","input_state_change"]}"#,
            ),
            0,
        ),
        (
            "state_missing",
            Some(r#"{"ok":true,"steps":["start_transition","input_state_evaluated_after_start"]}"#),
            0,
        ),
        (
            "state_false",
            Some(
                r#"{"ok":true,"input_state_change":false,"steps":["start_transition","input_state_evaluated_after_start"]}"#,
            ),
            0,
        ),
        (
            "state_null",
            Some(
                r#"{"ok":true,"input_state_change":null,"steps":["start_transition","input_state_evaluated_after_start"]}"#,
            ),
            0,
        ),
        (
            "alias_visible_state_false",
            Some(
                r#"{"ok":true,"visible_state_changed":false,"steps":["start_transition","input_state_evaluated_after_start"]}"#,
            ),
            0,
        ),
        (
            "contract_state_false_overrides_change",
            Some(&format!(
                r#"{{"ok":true,"input_contract_state_change":false,{STEPS_OK}}}"#
            )),
            0,
        ),
    ]);
    assert_eq!(actual, EXPECT_UNOBSERVED);
    // Only the contract-state override case observes everything it needs.
    assert_eq!(
        actual.matches(": ok=true").count(),
        1,
        "an unobserved start or input state must never decode as success"
    );
}

#[test]
fn raw_failure_kinds_are_replaced_by_taxonomy_only_when_generic() {
    let specific_complete =
        format!(r#"{{"ok":true,"failure_kind":"custom_app_failure",{STEPS_OK}}}"#);
    let actual = table(&[
        (
            "generic_kind",
            Some(
                r#"{"ok":true,"failure_kind":"input_state_change_missing","steps":["start_transition"]}"#,
            ),
            0,
        ),
        (
            "specific_kind_kept",
            Some(r#"{"ok":true,"failure_kind":"custom_app_failure","steps":["start_transition"]}"#),
            0,
        ),
        ("specific_kind_complete_steps", Some(&specific_complete), 0),
        (
            "generic_kind_complete_steps",
            Some(&format!(
                r#"{{"ok":true,"failure_kind":"interaction_state_change_missing",{STEPS_OK}}}"#
            )),
            0,
        ),
        (
            "browser_failure_kind_alias",
            Some(r#"{"ok":false,"browser_failure_kind":"app_route_unstable","steps":[]}"#),
            0,
        ),
        (
            "failure_kind_wins_over_alias",
            Some(
                r#"{"ok":false,"failure_kind":"app_route_unresponsive","browser_failure_kind":"app_route_unstable","steps":[]}"#,
            ),
            0,
        ),
    ]);
    assert_eq!(actual, EXPECT_KIND_REPLACEMENT);
}

#[test]
fn recovery_transition_counts_as_start_transition() {
    let actual = table(&[
        (
            "recovery_step",
            Some(
                r#"{"ok":true,"steps":["recovery_transition","input_state_evaluated_after_start","input_state_change"]}"#,
            ),
            0,
        ),
        (
            "recovery_flag_true",
            Some(
                r#"{"ok":true,"recovery_transition":true,"steps":["input_state_evaluated_after_start","input_state_change"]}"#,
            ),
            0,
        ),
        (
            "recovery_flag_false",
            Some(r#"{"ok":true,"recovery_transition":false,"steps":["input_state_change"]}"#),
            0,
        ),
        (
            "recovery_flag_null",
            Some(r#"{"ok":true,"recovery_transition":null,"steps":["input_state_change"]}"#),
            0,
        ),
        (
            "recovery_status_not_observed",
            Some(r#"{"ok":true,"recovery_transition_status":"not_observed","steps":[]}"#),
            0,
        ),
        (
            "recovery_step_not_observed",
            Some(r#"{"ok":true,"steps":["recovery_transition:not_observed"]}"#),
            0,
        ),
    ]);
    assert_eq!(actual, EXPECT_RECOVERY);
}

#[test]
fn startless_input_needs_visible_surface_and_state_change() {
    let actual = table(&[
        (
            "startless_ok",
            Some(
                r#"{"ok":true,"start_control_found":false,"surface_visible":true,"steps":["input_state_change"]}"#,
            ),
            0,
        ),
        (
            "startless_alias",
            Some(
                r#"{"ok":true,"start_control_present":false,"interactive_surface":true,"input_state_change":true}"#,
            ),
            0,
        ),
        (
            "startless_no_surface",
            Some(r#"{"ok":true,"start_control_found":false,"steps":["input_state_change"]}"#),
            0,
        ),
        (
            "startless_surface_false",
            Some(
                r#"{"ok":true,"start_control_found":false,"surface_visible":false,"steps":["input_state_change"]}"#,
            ),
            0,
        ),
        (
            "startless_surface_null",
            Some(
                r#"{"ok":true,"start_control_found":false,"surface_visible":null,"steps":["input_state_change"]}"#,
            ),
            0,
        ),
        (
            "startless_no_state_change",
            Some(r#"{"ok":true,"start_control_found":false,"surface_visible":true,"steps":[]}"#),
            0,
        ),
        (
            "text_entry_entered",
            Some(
                r#"{"ok":true,"start_control_found":false,"text_entry":"entered","text_input_state_change":true}"#,
            ),
            0,
        ),
        (
            "text_entry_not_entered",
            Some(
                r#"{"ok":true,"start_control_found":false,"text_entry":"skipped","text_input_state_change":true}"#,
            ),
            0,
        ),
        (
            "text_entry_without_state_change",
            Some(r#"{"ok":true,"start_control_found":false,"text_entry":"entered"}"#),
            0,
        ),
    ]);
    assert_eq!(actual, EXPECT_STARTLESS);
}

#[test]
fn input_markers_can_stand_in_for_evaluated_step() {
    let actual = table(&[
        (
            "markers_both",
            Some(
                r#"{"ok":true,"input_before_marker":"x","input_after_marker":"y","steps":["start_transition","input_state_change"]}"#,
            ),
            0,
        ),
        (
            "markers_after_empty",
            Some(
                r#"{"ok":true,"input_before_marker":"x","input_after_marker":"","steps":["start_transition","input_state_change"]}"#,
            ),
            0,
        ),
        (
            "markers_before_null",
            Some(
                r#"{"ok":true,"input_before_marker":null,"input_after_marker":"y","steps":["start_transition","input_state_change"]}"#,
            ),
            0,
        ),
    ]);
    assert_eq!(actual, EXPECT_MARKERS);
}

#[test]
fn failure_classification_pins_kind_and_remediation() {
    let actual = table(&[
        (
            "playwright_module_missing",
            Some(r#"{"ok":false,"error":"Cannot find module 'playwright'","steps":[]}"#),
            0,
        ),
        (
            "browser_binaries_missing",
            Some(r#"{"ok":false,"error":"Executable doesn't exist at /x","steps":[]}"#),
            0,
        ),
        (
            "dependency_prefix_kept",
            Some(r#"{"ok":false,"failure_kind":"probe_dependency_missing:custom","steps":[]}"#),
            0,
        ),
        (
            "navigation_failed_with_status",
            Some(
                r#"{"ok":false,"failure_kind":"probe_navigation_failed","server_http_status":500,"steps":[]}"#,
            ),
            0,
        ),
        (
            "navigation_failed_with_server_error",
            Some(
                r#"{"ok":false,"failure_kind":"probe_navigation_failed","server_check":{"status":null,"error":"refused"},"steps":[]}"#,
            ),
            0,
        ),
        (
            "script_error_navigating",
            Some(
                r#"{"ok":false,"failure_kind":"probe_script_error","stage":"navigating","error":"page.goto timed out","server_check":{"status":200,"error":""},"steps":[]}"#,
            ),
            0,
        ),
        (
            "script_error_other",
            Some(
                r#"{"ok":false,"failure_kind":"probe_script_error","stage":"clicking","steps":[]}"#,
            ),
            0,
        ),
        (
            "stage_from_probe_object",
            Some(r#"{"ok":false,"probe":{"stage":"launching"},"steps":[]}"#),
            0,
        ),
        (
            "empty_kind_pre_observation_stage",
            Some(r#"{"ok":false,"failure_kind":"","stage":"resolving","steps":[]}"#),
            0,
        ),
        (
            "app_failure_kind_kept",
            Some(
                r#"{"ok":false,"failure_kind":"custom_app_failure","stage":"clicking","steps":[]}"#,
            ),
            0,
        ),
        (
            "interaction_state_change_missing",
            Some(
                r#"{"ok":false,"failure_kind":"interaction_state_change_missing","stage":"clicking","steps":["start_transition","input_state_evaluated_after_start"]}"#,
            ),
            0,
        ),
    ]);
    assert_eq!(actual, EXPECT_CLASSIFICATION);
}

#[test]
fn script_outcomes_without_usable_evidence_fail_honestly() {
    let actual = table(&[
        ("evidence_missing", None, 0),
        ("evidence_not_object", Some("[1,2,3]"), 0),
        ("evidence_not_json", Some("not json"), 0),
        ("command_failed_no_evidence", None, 3),
        (
            "command_failed_with_failure_evidence",
            Some(
                r#"{"ok":false,"failure_kind":"probe_script_error","stage":"clicking","steps":[]}"#,
            ),
            3,
        ),
    ]);
    assert_eq!(actual, EXPECT_NO_EVIDENCE);
}

#[test]
fn field_defaults_are_pinned() {
    let run = run_probe(Some(r#"{"ok":true,"steps":["start_transition"]}"#), 0);
    let o = &run.observation;
    assert_eq!(o.text_entry, "not_evaluated");
    assert_eq!(o.text_entry_target, "");
    assert_eq!(o.typed_token, "");
    assert_eq!(o.persistence_after_reload, "not_evaluated");
    assert_eq!(o.persistence_after_reload_reason, "no_text_entry_surface");
    assert_eq!(o.probe_mode, "heuristic");
    assert_eq!(o.contract_hook_status, "unknown");
    assert_eq!(o.server_http_status, None);
    assert_eq!(o.server_http_error, "");
    assert_eq!(o.navigation_failure_kind, "");
    assert_eq!(o.has_canvas, None);
    assert_eq!(o.interactive_control_count, None);
    assert_eq!(o.restart_hook_count_after_start, 0);
    assert_eq!(o.surface_fit, None);
    assert!(o.candidate_table.is_empty());
    assert!(o.http_mutation_observations.is_empty());
    assert!(!o.token_echoed);
    assert!(!o.token_echoed_after_reload);
    assert!(!o.text_input_state_change);
    assert!(!o.canvas_not_redrawn_after_start);
}

#[test]
fn structured_fields_are_decoded_exactly() {
    let fixture = r#"{
      "ok": true,
      "steps": ["start_transition","input_state_evaluated_after_start","input_state_change","token_echoed_after_reload","canvas_not_redrawn_after_start","restart_hook_reachable_after_start"],
      "duration_ms": 1234,
      "cold_start_ms": 10,
      "measured_navigation_ms": 20,
      "probe_mode": "contract",
      "contract_hooks": {"status": "nested"},
      "candidate_table": [{"rank":1,"index":2,"text_excerpt":"Start","changed":true},{"rank":"x"}],
      "input_dispatches": ["a", 1, "b"],
      "canvas_snapshots": [{"name":"before_start","canvas_count":2,"canvas_blank":true,"blank_canvas_count":1,"pixel_hashes":["h1","h2",3]},{"canvas_count":1}],
      "canvas_blank_after_start": false,
      "state_dimensions_changed": ["score","level"],
      "surface_fit": {"surface":"canvas","fits_viewport":false,"overflow_top_px":1.6,"overflow_right_px":"3","viewport_width_px":800,"viewport_height_px":600,"rect_width_px":900.4,"rect_height_px":null},
      "restart_hook_count_after_start": 3,
      "action_hooks": ["b","a","b"],
      "text_entry": "entered",
      "text_entry_target": "input#name",
      "typed_token": "tok",
      "token_echoed": true,
      "echo_latency_ms": 7,
      "token_echo_after_reload_latency_ms": 9,
      "text_input_state_change": true,
      "primary_transition_observed": false,
      "informational_failure_kinds": ["x","y"],
      "http_mutation_observations": [{"method":"POST","status":201,"ok":true,"url":"/api"},{"method":"PUT"},{"method":"GET","status":200,"ok":false,"url":"/r"}],
      "post_js_has_canvas": true,
      "interactive_controls": "4",
      "server_http_status": 200,
      "navigation_failure_kind": "nav_x"
    }"#;
    let run = run_probe(Some(fixture), 0);
    let o = &run.observation;
    assert!(o.ok);
    assert_eq!(o.duration_ms, 1234);
    assert_eq!(o.cold_start_ms, Some(10));
    assert_eq!(o.measured_navigation_ms, Some(20));
    assert_eq!(o.probe_mode, "contract");
    assert_eq!(o.contract_hook_status, "nested");
    assert_eq!(format!("{:?}", o.candidate_table), EXPECT_CANDIDATES);
    assert_eq!(o.input_dispatches, vec!["a", "b"]);
    assert_eq!(format!("{:?}", o.canvas_snapshots), EXPECT_CANVAS);
    assert_eq!(o.canvas_blank_before_start, Some(true));
    assert_eq!(o.canvas_blank_after_start, Some(false));
    assert_eq!(o.canvas_blank_after_inputs, None);
    assert!(o.canvas_not_redrawn_after_start);
    assert_eq!(o.state_dimensions_changed, vec!["score", "level"]);
    assert_eq!(format!("{:?}", o.surface_fit), EXPECT_SURFACE);
    assert!(o.restart_hook_reachable_after_start);
    assert_eq!(o.restart_hook_count_after_start, 3);
    assert_eq!(o.persistence_after_reload, "not_evaluated");
    assert_eq!(o.persistence_after_reload_reason, "no_mutation_observed");
    assert_eq!(o.persistence_changed_dimensions, Vec::<String>::new());
    assert_eq!(o.action_hooks, vec!["a", "b"]);
    assert_eq!(o.text_entry, "entered");
    assert_eq!(o.text_entry_target, "input#name");
    assert_eq!(o.typed_token, "tok");
    assert!(o.token_echoed);
    assert_eq!(o.echo_latency_ms, Some(7));
    assert!(o.token_echoed_after_reload);
    assert_eq!(o.token_echo_after_reload_latency_ms, Some(9));
    assert!(o.text_input_state_change);
    assert!(!o.primary_transition_observed);
    assert_eq!(o.informational_failure_kinds, vec!["x", "y"]);
    assert_eq!(
        format!("{:?}", o.http_mutation_observations),
        EXPECT_HTTP_MUTATIONS
    );
    assert_eq!(o.has_canvas, Some(true));
    assert_eq!(o.interactive_control_count, Some(4));
    assert_eq!(o.server_http_status, Some(200));
    assert_eq!(o.navigation_failure_kind, "nav_x");
}

#[test]
fn http_mutation_observations_are_capped_at_twenty() {
    let rows = (0..25)
        .map(|i| format!(r#"{{"method":"POST","status":200,"ok":true,"url":"/{i}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    let fixture = format!(r#"{{"ok":true,"http_mutation_observations":[{rows}],{STEPS_OK}}}"#);
    let run = run_probe(Some(&fixture), 0);
    let urls = run
        .observation
        .http_mutation_observations
        .iter()
        .map(|row| row.url.clone())
        .collect::<Vec<_>>();
    assert_eq!(urls.len(), 20);
    assert_eq!(urls.first().map(String::as_str), Some("/0"));
    assert_eq!(urls.last().map(String::as_str), Some("/19"));
}

#[test]
fn canvas_count_and_control_count_fallbacks_are_pinned() {
    let alias = run_probe(
        Some(r#"{"ok":true,"canvas_found":false,"interactive_control_count":"7","steps":["start_transition"]}"#),
        0,
    )
    .observation;
    assert_eq!(alias.has_canvas, Some(false));
    assert_eq!(alias.interactive_control_count, Some(7));
    let bad = run_probe(
        Some(r#"{"ok":true,"has_canvas":"yes","interactive_controls":"many","steps":["start_transition"]}"#),
        0,
    )
    .observation;
    assert_eq!(bad.has_canvas, None);
    assert_eq!(bad.interactive_control_count, None);
}

#[test]
fn persistence_reconciliation_is_pinned() {
    let base = |mode: &str, before: &str, after: &str, dims: &str| {
        format!(
            r#"{{"ok":true,"probe_mode":"{mode}","text_entry":"entered","state_dimensions_changed":{dims},"persistence_before_reload_marker":"{before}","persistence_after_reload_marker":"{after}",{STEPS_OK}}}"#
        )
    };
    let cases = [
        ("preserved", base("heuristic", "m", "m", r#"["b","a","a"]"#)),
        ("reset", base("heuristic", "m", "n", r#"["a"]"#)),
        ("contract_mode", base("contract", "m", "m", r#"["a"]"#)),
        ("blank_marker", base("heuristic", "m", " ", r#"["a"]"#)),
        ("no_dims", base("heuristic", "m", "m", "[]")),
        ("blank_dims", base("heuristic", "m", "m", r#"[" "]"#)),
    ];
    let actual = cases
        .iter()
        .map(|(name, fixture)| {
            let o = run_probe(Some(fixture), 0).observation;
            format!(
                "{name}: {} / {:?} / {:?}",
                o.persistence_after_reload,
                o.persistence_after_reload_reason,
                o.persistence_changed_dimensions
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(actual, EXPECT_PERSISTENCE_RECONCILE);
}

#[test]
fn persistence_not_evaluated_reasons_are_pinned() {
    let cases = [
        (
            "explicit_reason",
            r#"{"ok":true,"persistence_after_reload_reason":"reload_failed","text_entry":"entered"}"#,
        ),
        (
            "unknown_reason",
            r#"{"ok":true,"persistence_after_reload_reason":"bogus","text_entry":"entered"}"#,
        ),
        (
            "failure_kind_reload",
            r#"{"ok":false,"failure_kind":"probe_reload_error"}"#,
        ),
        ("entered", r#"{"ok":true,"text_entry":"entered"}"#),
        ("target_only", r#"{"ok":true,"text_entry_target":"input"}"#),
        ("blank_target", r#"{"ok":true,"text_entry_target":"  "}"#),
        (
            "step_marks_not_evaluated",
            r#"{"ok":true,"persistence_after_reload":"preserved","steps":["persistence_reload:not_evaluated"]}"#,
        ),
        (
            "evaluated",
            r#"{"ok":true,"persistence_after_reload":"preserved"}"#,
        ),
    ];
    let actual = cases
        .iter()
        .map(|(name, fixture)| {
            let o = run_probe(Some(fixture), 0).observation;
            format!(
                "{name}: {} / {:?}",
                o.persistence_after_reload, o.persistence_after_reload_reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(actual, EXPECT_PERSISTENCE_REASONS);
}

const EXPECT_OK_NEGATIVES: &str = r##"missing_ok: ok=false status="failed" kind="" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="" rec="unknown" prim_missing=false]
null_ok: ok=false status="failed" kind="" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="" rec="unknown" prim_missing=false]
string_ok: ok=false status="failed" kind="" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="" rec="unknown" prim_missing=false]
number_ok: ok=false status="failed" kind="" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="" rec="unknown" prim_missing=false]
false_ok: ok=false status="failed" kind="" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="" rec="unknown" prim_missing=false]
empty_object: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]"##;
const EXPECT_UNOBSERVED: &str = r##"no_start_transition: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=["input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
start_transition_false: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=["input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
not_evaluated_after_start: ok=false status="failed" kind="input_state_change_not_evaluated_after_start" stage="" error="" rem="" steps=["start_transition", "input_state_change"] chg=true/true eval=false ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
evaluated_false: ok=false status="failed" kind="input_state_change_not_evaluated_after_start" stage="" error="" rem="" steps=["start_transition", "input_state_change"] chg=true/true eval=false ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
state_missing: ok=false status="failed" kind="input_state_change_missing_after_start" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start"] chg=false/false eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
state_false: ok=false status="failed" kind="input_state_change_missing_after_start" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start"] chg=false/false eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
state_null: ok=false status="failed" kind="input_state_change_missing_after_start" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start"] chg=false/false eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
alias_visible_state_false: ok=false status="failed" kind="input_state_change_missing_after_start" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start"] chg=false/false eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
contract_state_false_overrides_change: ok=true status="passed" kind="" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start", "input_state_change"] chg=true/false eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="passed" cat="" rec="unknown" prim_missing=false]"##;
const EXPECT_KIND_REPLACEMENT: &str = r##"generic_kind: ok=false status="failed" kind="input_state_change_not_evaluated_after_start" stage="" error="" rem="" steps=["start_transition"] chg=false/false eval=false ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
specific_kind_kept: ok=false status="failed" kind="custom_app_failure" stage="" error="" rem="" steps=["start_transition"] chg=false/false eval=false ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
specific_kind_complete_steps: ok=true status="passed" kind="" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="passed" cat="" rec="unknown" prim_missing=false]
generic_kind_complete_steps: ok=true status="passed" kind="" stage="" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="passed" cat="" rec="unknown" prim_missing=false]
browser_failure_kind_alias: ok=false status="failed" kind="app_route_unstable" stage="" error="" rem="" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
failure_kind_wins_over_alias: ok=false status="failed" kind="app_route_unresponsive" stage="" error="" rem="" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]"##;
const EXPECT_RECOVERY: &str = r##"recovery_step: ok=true status="passed" kind="" stage="" error="" rem="" steps=["recovery_transition", "input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=true rec=true/false spawned=true reaped=true ev[status="passed" cat="" rec="observed" prim_missing=false]
recovery_flag_true: ok=true status="passed" kind="" stage="" error="" rem="" steps=["input_state_evaluated_after_start", "input_state_change"] chg=true/true eval=true ctl=true prim=true rec=true/false spawned=true reaped=true ev[status="passed" cat="" rec="observed" prim_missing=false]
recovery_flag_false: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=["input_state_change"] chg=true/true eval=false ctl=true prim=false rec=false/true spawned=true reaped=true ev[status="failed" cat="app" rec="not_observed" prim_missing=false]
recovery_flag_null: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=["input_state_change"] chg=true/true eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
recovery_status_not_observed: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/true spawned=true reaped=true ev[status="failed" cat="app" rec="not_observed" prim_missing=false]
recovery_step_not_observed: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=["recovery_transition:not_observed"] chg=false/false eval=false ctl=true prim=false rec=false/true spawned=true reaped=true ev[status="failed" cat="app" rec="not_observed" prim_missing=false]"##;
const EXPECT_STARTLESS: &str = r##"startless_ok: ok=true status="passed" kind="" stage="" error="" rem="" steps=["input_state_change"] chg=true/true eval=false ctl=false prim=false rec=false/false spawned=true reaped=true ev[status="passed" cat="" rec="unknown" prim_missing=false]
startless_alias: ok=true status="passed" kind="" stage="" error="" rem="" steps=[] chg=true/true eval=false ctl=false prim=false rec=false/false spawned=true reaped=true ev[status="passed" cat="" rec="unknown" prim_missing=false]
startless_no_surface: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=["input_state_change"] chg=true/true eval=false ctl=false prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
startless_surface_false: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=["input_state_change"] chg=true/true eval=false ctl=false prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
startless_surface_null: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=["input_state_change"] chg=true/true eval=false ctl=false prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
startless_no_state_change: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=[] chg=false/false eval=false ctl=false prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
text_entry_entered: ok=true status="passed" kind="" stage="" error="" rem="" steps=[] chg=true/true eval=false ctl=false prim=false rec=false/false spawned=true reaped=true ev[status="passed" cat="" rec="unknown" prim_missing=false]
text_entry_not_entered: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=[] chg=true/true eval=false ctl=false prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
text_entry_without_state_change: ok=false status="failed" kind="start_transition_missing" stage="" error="" rem="" steps=[] chg=false/false eval=false ctl=false prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]"##;
const EXPECT_MARKERS: &str = r##"markers_both: ok=true status="passed" kind="" stage="" error="" rem="" steps=["start_transition", "input_state_change"] chg=true/true eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="passed" cat="" rec="unknown" prim_missing=false]
markers_after_empty: ok=false status="failed" kind="input_state_change_not_evaluated_after_start" stage="" error="" rem="" steps=["start_transition", "input_state_change"] chg=true/true eval=false ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
markers_before_null: ok=false status="failed" kind="input_state_change_not_evaluated_after_start" stage="" error="" rem="" steps=["start_transition", "input_state_change"] chg=true/true eval=false ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]"##;
const EXPECT_CLASSIFICATION: &str = r##"playwright_module_missing: ok=false status="failed" kind="probe_dependency_missing:playwright_module_missing" stage="" error="Cannot find module 'playwright'" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
browser_binaries_missing: ok=false status="failed" kind="probe_dependency_missing:browser_binaries_missing" stage="" error="Executable doesn't exist at /x" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
dependency_prefix_kept: ok=false status="failed" kind="probe_dependency_missing:custom" stage="" error="" rem="" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
navigation_failed_with_status: ok=false status="failed" kind="app_route_unresponsive" stage="" error="" rem="" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
navigation_failed_with_server_error: ok=false status="failed" kind="probe_infrastructure_failed:server_unreachable" stage="" error="" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
script_error_navigating: ok=false status="failed" kind="app_route_unresponsive" stage="navigating" error="page.goto timed out" rem="" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
script_error_other: ok=false status="failed" kind="probe_infrastructure_failed:probe_script_error" stage="clicking" error="" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
stage_from_probe_object: ok=false status="failed" kind="probe_infrastructure_failed:start_transition_missing" stage="launching" error="" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
empty_kind_pre_observation_stage: ok=false status="failed" kind="probe_infrastructure_failed:start_transition_missing" stage="resolving" error="" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
app_failure_kind_kept: ok=false status="failed" kind="custom_app_failure" stage="clicking" error="" rem="" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]
interaction_state_change_missing: ok=false status="failed" kind="input_state_change_missing_after_start" stage="clicking" error="" rem="" steps=["start_transition", "input_state_evaluated_after_start"] chg=false/false eval=true ctl=true prim=true rec=false/false spawned=true reaped=true ev[status="failed" cat="app" rec="unknown" prim_missing=false]"##;
const EXPECT_NO_EVIDENCE: &str = r##"evidence_missing: ok=false status="failed" kind="probe_infrastructure_failed:probe_evidence_missing" stage="" error="" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
evidence_not_object: ok=false status="failed" kind="probe_infrastructure_failed:probe_evidence_missing" stage="" error="" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
evidence_not_json: ok=false status="failed" kind="probe_infrastructure_failed:probe_evidence_missing" stage="" error="" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
command_failed_no_evidence: ok=false status="failed" kind="probe_infrastructure_failed:probe_command_failed" stage="" error="" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]
command_failed_with_failure_evidence: ok=false status="failed" kind="probe_infrastructure_failed:probe_script_error" stage="clicking" error="" rem="SETUP" steps=[] chg=false/false eval=false ctl=true prim=false rec=false/false spawned=true reaped=true ev[status="failed" cat="infrastructure" rec="unknown" prim_missing=false]"##;
const EXPECT_CANDIDATES: &str = r##"[InteractionProbeCandidateEvidence { rank: 1, index: 2, text_excerpt: "Start", changed: true }, InteractionProbeCandidateEvidence { rank: 0, index: 0, text_excerpt: "", changed: false }]"##;
const EXPECT_CANVAS: &str = r##"[CanvasSnapshotEvidence { name: "before_start", canvas_count: 2, canvas_blank: Some(true), blank_canvas_count: 1, pixel_hashes: ["h1", "h2"] }]"##;
const EXPECT_SURFACE: &str = r##"Some(SurfaceFitEvidence { surface: "canvas", fits_viewport: false, overflow_top_px: 2, overflow_right_px: 0, overflow_bottom_px: 0, overflow_left_px: 0, viewport_width_px: 800, viewport_height_px: 600, rect_width_px: 900, rect_height_px: 0 })"##;
const EXPECT_HTTP_MUTATIONS: &str = r##"[HttpMutationObservation { method: "POST", status: 201, ok: true, url: "/api" }, HttpMutationObservation { method: "GET", status: 200, ok: false, url: "/r" }]"##;
const EXPECT_PERSISTENCE_RECONCILE: &str = r##"preserved: preserved / "" / ["a", "b"]
reset: reset / "" / ["a"]
contract_mode: not_evaluated / "no_mutation_observed" / []
blank_marker: not_evaluated / "no_mutation_observed" / []
no_dims: not_evaluated / "no_mutation_observed" / []
blank_dims: not_evaluated / "no_mutation_observed" / []"##;
const EXPECT_PERSISTENCE_REASONS: &str = r##"explicit_reason: not_evaluated / "reload_failed"
unknown_reason: not_evaluated / "no_mutation_observed"
failure_kind_reload: not_evaluated / "reload_failed"
entered: not_evaluated / "no_mutation_observed"
target_only: not_evaluated / "no_mutation_observed"
blank_target: not_evaluated / "no_text_entry_surface"
step_marks_not_evaluated: preserved / "no_text_entry_surface"
evaluated: preserved / """##;
