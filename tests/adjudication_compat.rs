#![recursion_limit = "256"]

use commandagent::eval_events::{latest_completion_snapshot, project_completion};
use serde_json::{Value, json};

const ULTRA_FINAL_ACCEPTANCE_EVENT_KEYS: &str = concat!(
    "acceptance_layer,action_hooks,artifact_obligations,assurance_level,assurance_reason,",
    "browser_readiness_applicable,browser_readiness_evidence_path,",
    "browser_readiness_execution_status,browser_readiness_status,",
    "capability_evidence_bindings,compile_error_failure_kind,compile_errors,",
    "completion_contract_generated,completion_contract_path,",
    "completion_contract_path_merge_enabled,completion_contract_verification_enabled,",
    "contract_origin,cycle_index,echo_latency_ms,effective_profile,event,",
    "evidence_arbitration,evidence_arbitration_summary,evidence_tiers,",
    "external_contract_checked,external_contract_ok,external_contract_required,",
    "final_acceptance_status,handoff_saved_not_success,inconclusive_reasons,",
    "interaction_evidence_applicable,interaction_evidence_execution_status,",
    "interaction_evidence_path,interaction_evidence_status,missing_capabilities,",
    "missing_evidence,missing_obligations,missing_paths,next_action,",
    "obligation_repair_targets,plan_adherence_missing,plan_adherence_present,",
    "primary_reason,profile,profile_behavior_probe_evidence_path,",
    "profile_behavior_probe_reasons,profile_behavior_probe_status,",
    "profile_inference_source,profile_inferred,recovery_handoff_kind,",
    "recovery_handoff_saved,recovery_prompt_path,recovery_ultra_plan_path,",
    "release_gate_reasons,release_gate_status,release_quality_completion,",
    "requested_port,required_capabilities,required_evidence,required_obligations,",
    "required_paths,runtime_acceptance_diagnostics,runtime_acceptance_inconclusive,",
    "runtime_acceptance_passed,runtime_acceptance_status,schema_version,",
    "state_dimensions_changed,suggested_recovery_command,",
    "suggested_recovery_yaml_command,surface_fit,surface_fit_guidance,",
    "surface_fit_summary,text_entry,text_entry_target,text_input_state_change,",
    "token_echo_after_reload_latency_ms,token_echoed,token_echoed_after_reload,",
    "typed_token,unverified_evidence,weak_evidence"
);

#[test]
fn nextjs_full_adjudication_bytes_are_compatible() {
    let event = completion_event(
        "nextjs",
        "pass",
        "full_success",
        "pass",
        "full",
        "",
        &[],
        &[],
        GateFixture::performed(),
        GateFixture::performed(),
    );
    assert_compatibility(
        "nextjs-full",
        true,
        event,
        json!({
            "assurance_level": "full",
            "assurance_reason": "",
            "command_completion": "completed",
            "final_acceptance": "full_success",
            "next_action": "none",
            "release_gate": "pass",
            "release_quality_completion": "release_ready",
            "runtime_acceptance": "pass",
            "status": "complete",
            "task_status": "complete"
        }),
    );
}

#[test]
fn nextjs_build_failure_adjudication_bytes_are_compatible() {
    let reason = "production_build_failed_before_browser_probe";
    let event = completion_event(
        "nextjs",
        "failed",
        "incomplete",
        "failed",
        "partial",
        reason,
        &[reason],
        &[],
        GateFixture::disconnected(),
        GateFixture::disconnected(),
    );
    assert_compatibility(
        "nextjs-build-failed",
        false,
        event,
        json!({
            "assurance_level": "partial",
            "assurance_reason": reason,
            "command_completion": "failed",
            "final_acceptance": "incomplete",
            "next_action": "fix_command_failure",
            "release_gate": "failed",
            "release_quality_completion": "failed",
            "runtime_acceptance": "failed",
            "status": "incomplete",
            "task_status": "failed"
        }),
    );
}

#[test]
fn nextjs_interaction_partial_adjudication_bytes_are_compatible() {
    let reason = "interaction_unverified:probe_unavailable";
    let unverified = "stateful_update_evidence:unverified:probe_unavailable";
    let event = completion_event(
        "nextjs",
        "partial",
        "partial",
        "partial",
        "partial",
        reason,
        &[reason],
        &[unverified],
        GateFixture::performed(),
        GateFixture::unavailable(),
    );
    assert_compatibility(
        "nextjs-interaction-partial",
        true,
        event,
        json!({
            "assurance_level": "partial",
            "assurance_reason": reason,
            "command_completion": "completed",
            "final_acceptance": "partial",
            "next_action": "run_setup_interaction_probe_to_enable_interaction_release_checks",
            "release_gate": "partial",
            "release_quality_completion": "partial",
            "runtime_acceptance": "partial",
            "status": "complete_with_partial_release_gate",
            "task_status": "partial (interaction unverified)"
        }),
    );
}

#[test]
fn data_full_adjudication_bytes_are_compatible() {
    let event = completion_event(
        "data",
        "pass",
        "full_success",
        "not_applicable",
        "full",
        "",
        &[],
        &[],
        GateFixture::not_applicable(),
        GateFixture::not_applicable(),
    );
    assert_compatibility(
        "data-full",
        true,
        event,
        json!({
            "assurance_level": "full",
            "assurance_reason": "",
            "command_completion": "completed",
            "final_acceptance": "full_success",
            "next_action": "none",
            "release_gate": "not_applicable",
            "release_quality_completion": "release_ready",
            "runtime_acceptance": "pass",
            "status": "complete",
            "task_status": "complete"
        }),
    );
}

#[test]
fn data_static_adjudication_bytes_are_compatible() {
    let event = completion_event(
        "data",
        "static",
        "full_success",
        "not_applicable",
        "static",
        "data_profile_probe_not_run",
        &[],
        &[],
        GateFixture::not_applicable(),
        GateFixture::not_applicable(),
    );
    assert_compatibility(
        "data-static",
        true,
        event,
        json!({
            "assurance_level": "static",
            "assurance_reason": "data_profile_probe_not_run",
            "command_completion": "completed",
            "final_acceptance": "full_success",
            "next_action": "none",
            "release_gate": "not_applicable",
            "release_quality_completion": "release_ready",
            "runtime_acceptance": "static",
            "status": "complete",
            "task_status": "completed (static assurance)"
        }),
    );
}

#[test]
fn data_failed_adjudication_bytes_are_compatible() {
    let reason = "data_assurance_failed";
    let event = completion_event(
        "data",
        "failed",
        "incomplete",
        "failed",
        "failed",
        reason,
        &[reason],
        &[],
        GateFixture::not_applicable(),
        GateFixture::not_applicable(),
    );
    assert_compatibility(
        "data-failed",
        false,
        event,
        json!({
            "assurance_level": "failed",
            "assurance_reason": reason,
            "command_completion": "failed",
            "final_acceptance": "incomplete",
            "next_action": "fix_command_failure",
            "release_gate": "failed",
            "release_quality_completion": "failed",
            "runtime_acceptance": "failed",
            "status": "incomplete",
            "task_status": "failed"
        }),
    );
}

#[derive(Clone, Copy)]
struct GateFixture {
    applicable: bool,
    execution_status: &'static str,
    status: &'static str,
}

impl GateFixture {
    fn performed() -> Self {
        Self {
            applicable: true,
            execution_status: "performed",
            status: "passed",
        }
    }

    fn disconnected() -> Self {
        Self {
            applicable: true,
            execution_status: "disconnected",
            status: "not_applicable",
        }
    }

    fn unavailable() -> Self {
        Self {
            applicable: true,
            execution_status: "unavailable",
            status: "unavailable:interaction_evidence_missing",
        }
    }

    fn not_applicable() -> Self {
        Self {
            applicable: false,
            execution_status: "not_applicable",
            status: "not_applicable",
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn completion_event(
    profile: &str,
    runtime_acceptance_status: &str,
    final_acceptance_status: &str,
    release_gate_status: &str,
    assurance_level: &str,
    assurance_reason: &str,
    release_gate_reasons: &[&str],
    unverified_evidence: &[&str],
    browser: GateFixture,
    interaction: GateFixture,
) -> Value {
    json!({
        "acceptance_layer": "",
        "action_hooks": [],
        "artifact_obligations": [],
        "assurance_level": assurance_level,
        "assurance_reason": assurance_reason,
        "browser_readiness_applicable": browser.applicable,
        "browser_readiness_evidence_path": "",
        "browser_readiness_execution_status": browser.execution_status,
        "browser_readiness_status": browser.status,
        "capability_evidence_bindings": [],
        "compile_error_failure_kind": "",
        "compile_errors": [],
        "completion_contract_generated": false,
        "completion_contract_path": "contract.json",
        "completion_contract_path_merge_enabled": true,
        "completion_contract_verification_enabled": true,
        "contract_origin": "initial",
        "cycle_index": 0,
        "echo_latency_ms": null,
        "effective_profile": profile,
        "event": "ultra_final_acceptance",
        "evidence_arbitration": {},
        "evidence_arbitration_summary": "",
        "evidence_tiers": {},
        "external_contract_checked": true,
        "external_contract_ok": true,
        "external_contract_required": true,
        "final_acceptance_status": final_acceptance_status,
        "handoff_saved_not_success": false,
        "inconclusive_reasons": [],
        "interaction_evidence_applicable": interaction.applicable,
        "interaction_evidence_execution_status": interaction.execution_status,
        "interaction_evidence_path": "",
        "interaction_evidence_status": interaction.status,
        "missing_capabilities": [],
        "missing_evidence": [],
        "missing_obligations": [],
        "missing_paths": [],
        "next_action": "none",
        "obligation_repair_targets": [],
        "plan_adherence_missing": [],
        "plan_adherence_present": [],
        "primary_reason": assurance_reason,
        "profile": profile,
        "profile_behavior_probe_evidence_path": "",
        "profile_behavior_probe_reasons": [],
        "profile_behavior_probe_status": if runtime_acceptance_status == "failed" { "failed" } else { "pass" },
        "profile_inference_source": "",
        "profile_inferred": "",
        "recovery_handoff_kind": "",
        "recovery_handoff_saved": false,
        "recovery_prompt_path": "",
        "recovery_ultra_plan_path": "",
        "release_gate_reasons": release_gate_reasons,
        "release_gate_status": release_gate_status,
        "release_quality_completion": "",
        "requested_port": null,
        "required_capabilities": [],
        "required_evidence": [],
        "required_obligations": [],
        "required_paths": [],
        "runtime_acceptance_diagnostics": [],
        "runtime_acceptance_inconclusive": false,
        "runtime_acceptance_passed": runtime_acceptance_status == "pass",
        "runtime_acceptance_status": runtime_acceptance_status,
        "schema_version": "1",
        "state_dimensions_changed": [],
        "suggested_recovery_command": "",
        "suggested_recovery_yaml_command": "",
        "surface_fit": {},
        "surface_fit_guidance": "",
        "surface_fit_summary": "",
        "text_entry": "not_applicable",
        "text_entry_target": "",
        "text_input_state_change": false,
        "token_echo_after_reload_latency_ms": null,
        "token_echoed": false,
        "token_echoed_after_reload": false,
        "typed_token": "",
        "unverified_evidence": unverified_evidence,
        "weak_evidence": []
    })
}

fn assert_compatibility(name: &str, ok: bool, event: Value, expected: Value) {
    let keys = event
        .as_object()
        .expect("completion event object")
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        keys, ULTRA_FINAL_ACCEPTANCE_EVENT_KEYS,
        "{name} event shape"
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let events = dir.path().join("events.jsonl");
    let event_bytes = serde_json::to_vec(&event).expect("serialize completion event");
    let mut persisted = event_bytes.clone();
    persisted.push(b'\n');
    std::fs::write(&events, &persisted).expect("write completion event fixture");
    assert_eq!(
        std::fs::read(&events).expect("read completion event fixture"),
        persisted,
        "{name} completion event bytes"
    );

    let snapshot = latest_completion_snapshot(Some(&events));
    let projection = project_completion(ok, &snapshot);
    let actual = json!({
        "assurance_level": projection.assurance_level,
        "assurance_reason": projection.assurance_reason,
        "command_completion": projection.command_completion,
        "final_acceptance": projection.final_acceptance,
        "next_action": projection.next_action,
        "release_gate": projection.release_gate,
        "release_quality_completion": projection.release_quality_completion,
        "runtime_acceptance": projection.runtime_acceptance,
        "status": projection.status,
        "task_status": projection.task_status
    });
    assert_eq!(
        serde_json::to_vec(&actual).expect("serialize actual adjudication"),
        serde_json::to_vec(&expected).expect("serialize expected adjudication"),
        "{name} adjudication bytes"
    );
}
