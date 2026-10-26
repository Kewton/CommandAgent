use std::path::Path;

use serde_json::{Value, json};

use super::RESTART_NOT_EXERCISED_REASON;
use crate::minimal_loop::behavior_evidence::arbitrate_final_acceptance;
use crate::minimal_loop::evidence::verify_runtime_acceptance_with_browser_dirs_and_hints;
use crate::planner::interaction_qualification::{
    enforce_release_gate, qualify_interaction_evidence,
};
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID as PROFILE_ID;

const NOT_EXERCISED_STATUS: &str = "unverified:restart_not_exercised";
const RESTART_MISSING: &str = "contract_instrumentation_missing:restart";
const RESTART_EVIDENCE: &str = "restart_or_recoverable_state_evidence";

#[derive(Debug)]
struct Gate {
    status: String,
    reasons: Vec<String>,
    interaction_status: String,
}

/// A contract-mode probe output with a usable primary hook, a changed state
/// and nothing about restart; each test adds the restart fields it needs.
fn base_evidence() -> Value {
    json!({
        "probe_mode": "contract",
        "contract_hook_status": "usable",
        "action_hooks": ["primary"],
        "state_dimensions_changed": ["score"],
        "start_transition": true,
        "restart_hook_count_after_start": 0,
        "restart_hook_reachable_after_start": false
    })
}

fn with_fields(fields: Value) -> Value {
    let mut evidence = base_evidence();
    for (key, value) in fields.as_object().unwrap() {
        evidence[key] = value.clone();
    }
    evidence
}

/// P1: the restart hook exists only before start; the legacy recovery moved a
/// marker but no restart control was credited.
fn pre_start_hook_only_evidence() -> Value {
    with_fields(json!({
        "action_hooks": ["primary", "restart"],
        "recovery_transition": true,
        "recovery_transition_status": "observed",
        "recovery_control": "none"
    }))
}

fn run_gate(evidence: &Value, restart_required: bool, initial_status: &str) -> Gate {
    let dir = tempfile::tempdir().unwrap();
    let evidence_path = dir.path().join("browser-interaction.json");
    std::fs::write(&evidence_path, evidence.to_string()).unwrap();
    let mut status = initial_status.to_string();
    let mut reasons = Vec::new();
    let mut interaction_status = "passed".to_string();
    enforce_release_gate(
        &mut status,
        &mut reasons,
        &mut interaction_status,
        evidence_path.to_str().unwrap(),
        restart_required,
        dir.path(),
        PROFILE_ID,
    );
    Gate {
        status,
        reasons,
        interaction_status,
    }
}

fn assert_not_exercised_qualification(evidence: &Value) {
    let qualified = qualify_interaction_evidence(evidence, true);
    assert!(!qualified.full_eligible, "{qualified:?}");
    assert_eq!(qualified.evidence_status, NOT_EXERCISED_STATUS);
    assert_eq!(
        qualified.release_gate_reasons,
        [RESTART_NOT_EXERCISED_REASON]
    );
}

fn assert_partial_gate(evidence: &Value) {
    let gate = run_gate(evidence, true, "pass");
    assert_eq!(gate.status, "partial", "{gate:?}");
    assert_eq!(gate.reasons, [RESTART_NOT_EXERCISED_REASON], "{gate:?}");
    assert_eq!(gate.interaction_status, NOT_EXERCISED_STATUS, "{gate:?}");
}

#[test]
fn pre_start_hook_only_with_unattributed_recovery_is_partial_not_full() {
    let evidence = pre_start_hook_only_evidence();

    assert_not_exercised_qualification(&evidence);
    assert_partial_gate(&evidence);
    let gate = run_gate(&evidence, true, "partial");
    assert_eq!(gate.status, "partial", "{gate:?}");
}

#[test]
fn not_exercised_partial_never_lifts_a_failed_gate() {
    let gate = run_gate(&pre_start_hook_only_evidence(), true, "failed");

    assert_eq!(gate.status, "failed", "{gate:?}");
}

#[test]
fn no_not_exercised_evidence_reaches_a_pass_gate() {
    let evidences = [
        pre_start_hook_only_evidence(),
        with_fields(json!({"restart_hook_count_after_start": 2})),
        with_fields(json!({"contract_hooks": {"restart_present": true}})),
        with_fields(json!({
            "action_hooks": ["primary", "restart"],
            "recovery_transition_status": "observed",
            "recovery_control": "keyboard_r"
        })),
        with_fields(json!({
            "action_hooks": ["primary", "restart"],
            "recovery_transition_status": "observed",
            "recovery_control": "primary_fallback"
        })),
        with_fields(json!({"action_hooks": ["primary", "restart"]})),
    ];
    for evidence in &evidences {
        for initial in ["pass", "partial", "failed"] {
            let gate = run_gate(evidence, true, initial);
            assert_ne!(gate.status, "pass", "{initial}: {evidence}: {gate:?}");
            if initial == "failed" {
                assert_eq!(gate.status, "failed", "{evidence}: {gate:?}");
            }
        }
    }
}

#[test]
fn restart_hook_reachable_after_start_stays_full() {
    let evidence = with_fields(json!({
        "restart_hook_reachable_after_start": true,
        "restart_hook_count_after_start": 1,
        "recovery_transition_status": "observed",
        "recovery_control": "none"
    }));

    let qualified = qualify_interaction_evidence(&evidence, true);
    assert!(qualified.full_eligible, "{qualified:?}");
    assert_eq!(qualified.evidence_status, "passed");
    let gate = run_gate(&evidence, true, "pass");
    assert_eq!(gate.status, "pass", "{gate:?}");
    assert!(gate.reasons.is_empty(), "{gate:?}");
}

#[test]
fn late_restart_hook_attributed_to_recovery_stays_full() {
    let evidence = with_fields(json!({
        "action_hooks": ["primary", "restart"],
        "recovery_transition": true,
        "recovery_transition_status": "observed",
        "recovery_control": "restart"
    }));

    let qualified = qualify_interaction_evidence(&evidence, true);
    assert!(qualified.full_eligible, "{qualified:?}");
    let gate = run_gate(&evidence, true, "pass");
    assert_eq!(gate.status, "pass", "{gate:?}");
}

#[test]
fn recovery_by_restart_that_was_not_observed_does_not_satisfy_the_gate() {
    let evidence = with_fields(json!({
        "action_hooks": ["primary", "restart"],
        "recovery_transition_status": "not_observed",
        "recovery_control": "restart"
    }));

    assert_not_exercised_qualification(&evidence);
}

#[test]
fn keyboard_r_recovery_without_any_hook_keeps_restart_missing() {
    let evidence = with_fields(json!({
        "recovery_transition": true,
        "recovery_transition_status": "observed",
        "recovery_control": "keyboard_r"
    }));

    let qualified = qualify_interaction_evidence(&evidence, true);
    assert!(!qualified.full_eligible, "{qualified:?}");
    assert_eq!(
        qualified.evidence_status,
        format!("failed:{RESTART_MISSING}")
    );
    assert_eq!(qualified.release_gate_reasons, [RESTART_MISSING]);
    let gate = run_gate(&evidence, true, "pass");
    assert_eq!(gate.status, "failed", "{gate:?}");
    assert_eq!(gate.reasons, [RESTART_MISSING], "{gate:?}");
    assert_eq!(
        gate.interaction_status,
        format!("failed:{RESTART_MISSING}"),
        "{gate:?}"
    );
}

#[test]
fn hidden_hook_counted_after_start_without_attributed_recovery_is_partial() {
    let evidence = with_fields(json!({
        "restart_hook_count_after_start": 1,
        "recovery_transition": false,
        "recovery_transition_status": "not_observed",
        "recovery_control": "none"
    }));

    assert_not_exercised_qualification(&evidence);
    assert_partial_gate(&evidence);
}

#[test]
fn legacy_evidence_without_recovery_control_is_not_full() {
    let evidence = with_fields(json!({
        "action_hooks": ["primary", "restart"],
        "recovery_transition": true,
        "recovery_transition_status": "observed"
    }));

    assert_not_exercised_qualification(&evidence);
    assert_partial_gate(&evidence);

    let bare = with_fields(json!({"action_hooks": ["primary", "restart"]}));
    assert_not_exercised_qualification(&bare);
}

#[test]
fn restart_not_required_ignores_the_restart_attribution() {
    let evidence = pre_start_hook_only_evidence();

    let qualified = qualify_interaction_evidence(&evidence, false);
    assert!(qualified.full_eligible, "{qualified:?}");
    assert_eq!(qualified.evidence_status, "passed");
    let gate = run_gate(&evidence, false, "pass");
    assert_eq!(gate.status, "pass", "{gate:?}");
    assert!(gate.reasons.is_empty(), "{gate:?}");
}

#[test]
fn other_missing_pieces_still_fail_the_gate_instead_of_partial() {
    let evidence = with_fields(json!({
        "action_hooks": ["restart"],
        "recovery_control": "none"
    }));

    let qualified = qualify_interaction_evidence(&evidence, true);
    assert!(!qualified.full_eligible, "{qualified:?}");
    assert_eq!(
        qualified.release_gate_reasons,
        ["contract_instrumentation_missing:primary"]
    );
    let gate = run_gate(&evidence, true, "pass");
    assert_eq!(gate.status, "failed", "{gate:?}");
}

const TODO_PAGE: &str = r#""use client";
import { useState } from "react";
export default function Page() {
  const [draft, setDraft] = useState("");
  const [items, setItems] = useState<string[]>([]);
  const state = JSON.stringify({ draft, items });
  return (
    <main data-anvil-state={state}>
      <input aria-label="Todo" value={draft} onChange={(event) => setDraft(event.target.value)} />
      <button data-anvil-action="primary" onClick={() => setItems([...items, draft])}>Add</button>
      <button data-anvil-action="restart" onClick={() => { setDraft(""); setItems([]); }}>Clear</button>
      <ul>{items.map((item) => <li key={item}>{item}</li>)}</ul>
    </main>
  );
}
"#;

fn arbitrated_restart_record(root: &Path, recovery_control: &str) -> (String, String) {
    let page = root.join("src/app/page.tsx");
    std::fs::create_dir_all(page.parent().unwrap()).unwrap();
    std::fs::write(page, TODO_PAGE).unwrap();
    let run_dir = root.join(".anvil/runs/test");
    std::fs::create_dir_all(&run_dir).unwrap();
    let evidence = json!({
        "ok": true,
        "status": "passed",
        "interaction_success": true,
        "interaction_performed": true,
        "input_event_observed": true,
        "state_changed": true,
        "probe_mode": "contract",
        "contract_hook_status": "usable",
        "contract_hooks": {
            "usable": true,
            "primary_present": true,
            "restart_present": true,
            "valid_state_count": 1
        },
        "steps": [
            "surface_visible",
            "start_transition",
            "control_input_dispatched",
            "input_state_evaluated_after_start",
            "input_state_change",
            "recovery_transition"
        ],
        "before_marker": "{\"states\":[{\"index\":0,\"state\":{\"draft\":\"\",\"items\":[]}}]}",
        "after_marker": "{\"states\":[{\"index\":0,\"state\":{\"draft\":\"\",\"items\":[\"anvil probe input\"]}}]}",
        "input_before_marker": "{\"states\":[{\"index\":0,\"state\":{\"draft\":\"\",\"items\":[\"anvil probe input\"]}}]}",
        "input_after_marker": "{\"states\":[{\"index\":0,\"state\":{\"draft\":\"x\",\"items\":[\"anvil probe input\"]}}]}",
        "recovery_transition": true,
        "recovery_transition_status": "observed",
        "recovery_control": recovery_control,
        "state_dimensions_changed": ["draft"]
    });
    std::fs::write(
        run_dir.join("browser-interaction.json"),
        evidence.to_string(),
    )
    .unwrap();
    let required = [
        "visible_interactive_surface_evidence",
        "user_input_handler_evidence",
        "stateful_update_evidence",
        RESTART_EVIDENCE,
    ]
    .map(String::from);
    let mut report = verify_runtime_acceptance_with_browser_dirs_and_hints(
        root,
        &["src/app/page.tsx".to_string()],
        &[],
        &[],
        &required,
        &[],
        &[],
        std::slice::from_ref(&run_dir),
        &[],
    );
    let arbitration = arbitrate_final_acceptance(
        &mut report,
        root,
        std::slice::from_ref(&run_dir),
        &[],
        &required,
        &[],
    );
    let record = arbitration.records.get(RESTART_EVIDENCE).unwrap();
    (
        record.behavioral_observation.clone(),
        record.final_tier.clone(),
    )
}

#[test]
fn arbitration_does_not_pass_a_recovery_without_a_restart_control() {
    let dir = tempfile::tempdir().unwrap();

    let (observation, final_tier) = arbitrated_restart_record(dir.path(), "none");

    assert_eq!(observation, "terminal_state_not_reached");
    assert_eq!(final_tier, "unverified:terminal_state_not_reached");
}

#[test]
fn arbitration_keeps_passing_a_recovery_attributed_to_restart() {
    let dir = tempfile::tempdir().unwrap();

    let (observation, final_tier) = arbitrated_restart_record(dir.path(), "restart");

    assert_eq!(observation, "recovery_transition");
    assert_eq!(final_tier, "strong");
}
