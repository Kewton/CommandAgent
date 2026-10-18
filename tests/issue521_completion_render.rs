//! Pins the byte output of the completion summary renderer (issue #521 S04).
//!
//! Every case goes through the public `eval_events` entry points
//! (`project_completion`, `append_completion_summary`,
//! `write_tui_command_completion_summary`) and compares the machine section of
//! the written `summary.md` as a whole string. The fixtures live in a tempdir;
//! nothing dynamic reaches the machine section, so no normalization is needed
//! beyond dropping the environment-dependent `Host env:` line.

use std::path::Path;

use commandagent::eval_events::{
    CompletionProjection, CompletionSnapshot, append_completion_summary, project_completion,
    write_tui_command_completion_summary,
};

const MACHINE_HEADING: &str = "## Machine details\n\n";

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

/// Read the machine section of the summary next to `events`.
fn machine_section(events: &Path) -> String {
    let document = std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap();
    let (_, body) = document
        .split_once(MACHINE_HEADING)
        .expect("summary.md has a machine details section");
    // `NODE_ENV` / `NODE_OPTIONS` in the host environment add this one line.
    body.lines()
        .filter(|line| !line.starts_with("Host env:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn events_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join(".anvil/runs/render/events.jsonl")
}

struct Case {
    name: &'static str,
    snapshot: CompletionSnapshot,
    ok: bool,
    stage: &'static str,
    action: Option<&'static str>,
    command: Option<&'static str>,
    stop_reason: &'static str,
    failure_kind: &'static str,
}

fn full_snapshot() -> CompletionSnapshot {
    let mut snapshot = CompletionSnapshot::empty();
    snapshot.profile = "web".to_string();
    snapshot.effective_profile = "web".to_string();
    snapshot.prompt_layout = "balanced".to_string();
    snapshot.contract_origin = "promoted_union".to_string();
    snapshot.assurance_level = "behavioral".to_string();
    snapshot.assurance_reason = "probe passed".to_string();
    snapshot.profile_inferred = "web".to_string();
    snapshot.profile_inference_source = "prompt".to_string();
    snapshot.requested_port = "3000".to_string();
    snapshot.runtime_acceptance_status = "pass".to_string();
    snapshot.final_acceptance_status = "pass".to_string();
    snapshot.release_gate_status = "pass".to_string();
    snapshot.completion_contract_verification_enabled = true;
    snapshot.completion_contract_path_merge_enabled = true;
    snapshot.completion_contract_path = "contract.json".to_string();
    snapshot.completion_contract_generated = true;
    snapshot.external_contract_checked = true;
    snapshot.external_contract_ok = true;
    snapshot.browser_readiness_applicable = true;
    snapshot.browser_readiness_execution_status = "executed".to_string();
    snapshot.browser_readiness_status = "ready".to_string();
    snapshot.browser_readiness_evidence_path = "evidence/browser.json".to_string();
    snapshot.interaction_evidence_applicable = true;
    snapshot.interaction_evidence_execution_status = "executed".to_string();
    snapshot.interaction_evidence_status = "pass".to_string();
    snapshot.interaction_evidence_path = "evidence/interaction.json".to_string();
    snapshot.state_dimensions_changed = strings(&["score", "level"]);
    snapshot.action_hooks = strings(&["start", "restart"]);
    snapshot.surface_fit_summary = "fits".to_string();
    snapshot.surface_fit_guidance = "keep the canvas visible".to_string();
    snapshot.text_entry_target = "#name".to_string();
    snapshot.typed_token = "tok-1".to_string();
    snapshot.token_echoed = "yes".to_string();
    snapshot.text_input_state_change = "changed".to_string();
    snapshot.evidence_arbitration_summary = "static (agree)".to_string();
    snapshot.depth_profile_summary = "standard".to_string();
    snapshot.planner_verify_normalization_count = 2;
    snapshot.planner_retry_count = 1;
    snapshot.planner_quality_warning_count = 3;
    snapshot.planner_quality_issue_count = 4;
    snapshot.planner_repaired = true;
    snapshot.planner_release_risk = true;
    snapshot.context_truncation_warning_count = 2;
    snapshot.plan_adherence_present = strings(&["alpha", "beta"]);
    snapshot.plan_adherence_missing = strings(&["gamma"]);
    snapshot.compile_rollback_summaries = strings(&["rolled back src/a.rs"]);
    snapshot.recovery_prompt_path = "recovery/prompt.md".to_string();
    snapshot.recovery_ultra_plan_path = "recovery/plan.yaml".to_string();
    snapshot.suggested_recovery_command = "commandagent --resume".to_string();
    snapshot.suggested_recovery_yaml_command = "commandagent --plan recovery/plan.yaml".to_string();
    snapshot
}

fn partial_probe_snapshot() -> CompletionSnapshot {
    let mut snapshot = CompletionSnapshot::empty();
    snapshot.profile = "game".to_string();
    snapshot.release_gate_status = "partial".to_string();
    snapshot.final_acceptance_status = "partial".to_string();
    snapshot.release_gate_reasons = strings(&["interaction_unverified:probe_unavailable"]);
    snapshot.unverified_evidence = strings(&[
        "restart:unverified:terminal_state_not_reached",
        "fire:unverified:probe_unavailable",
    ]);
    snapshot.probe_preflight_status = "failed".to_string();
    snapshot.probe_preflight_reason = "chromium missing".to_string();
    snapshot
}

fn infra_failure_snapshot(remediation: &str) -> CompletionSnapshot {
    let mut snapshot = CompletionSnapshot::empty();
    snapshot.release_gate_status = "failed".to_string();
    snapshot.final_acceptance_status = "failed".to_string();
    snapshot.release_gate_reasons = strings(&["probe_infrastructure_failed", "other reason"]);
    snapshot.evidence_arbitration_summary =
        "static (probe infrastructure failure: no browser)".to_string();
    snapshot.probe_preflight_status = "failed".to_string();
    snapshot.probe_preflight_remediation = remediation.to_string();
    snapshot
}

fn cases() -> Vec<Case> {
    let case = |name, snapshot, ok, stage, action, command, stop_reason, failure_kind| Case {
        name,
        snapshot,
        ok,
        stage,
        action,
        command,
        stop_reason,
        failure_kind,
    };
    vec![
        case(
            "complete_process_full",
            full_snapshot(),
            true,
            "process",
            Some("run"),
            None,
            "done cleanly",
            "",
        ),
        case(
            "missing_everything",
            CompletionSnapshot::empty(),
            true,
            "process",
            None,
            None,
            "",
            "",
        ),
        case(
            "failed_tui_command",
            CompletionSnapshot::empty(),
            false,
            "tui_command",
            None,
            Some("/run-ultra"),
            "verify failed\nPaths:\n- src/lib.rs\nCommands:\n- cargo test",
            "verification_failed",
        ),
        case(
            "partial_probe_unavailable",
            partial_probe_snapshot(),
            true,
            "tui_command",
            Some("create"),
            Some("/create game"),
            "probe unavailable",
            "",
        ),
        case(
            "invalid_infra_failure_default_remediation",
            infra_failure_snapshot(""),
            false,
            "other_stage",
            None,
            None,
            "infra",
            "probe_infrastructure",
        ),
        case(
            "invalid_infra_failure_custom_remediation",
            infra_failure_snapshot("  install chromium  "),
            false,
            "process",
            None,
            None,
            "infra",
            "probe_infrastructure",
        ),
    ]
}

fn render_case(case: &Case) -> String {
    let dir = tempfile::tempdir().unwrap();
    let events = events_path(&dir);
    std::fs::create_dir_all(events.parent().unwrap()).unwrap();
    std::fs::write(&events, "").unwrap();
    let projection = project_completion(case.ok, &case.snapshot);
    append_completion_summary(
        Some(&events),
        case.stage,
        case.action,
        case.command,
        case.stop_reason,
        case.failure_kind,
        &projection,
    );
    assert_eq!(
        std::fs::read_to_string(&events).unwrap(),
        "",
        "{}: rendering a summary must not append events",
        case.name
    );
    machine_section(&events)
}

fn render_tui(projection: &CompletionProjection, status: &str, events_text: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let events = events_path(&dir);
    std::fs::create_dir_all(events.parent().unwrap()).unwrap();
    std::fs::write(&events, events_text).unwrap();
    write_tui_command_completion_summary(
        Some(&events),
        "/run-ultra",
        "command stopped\nPaths:\n- src/main.rs",
        "command_failure",
        status,
        projection,
    );
    assert_eq!(
        std::fs::read_to_string(&events).unwrap(),
        events_text,
        "tui summary must leave the event stream untouched"
    );
    machine_section(&events)
}

const TUI_EVENTS: &str = concat!(
    "{\"event\":\"profile_reinferred\",\"from_profile\":\"generic\",\"to_profile\":\"web\",\"from\":\"workspace\",\"at_phase\":2}\n",
    "{\"event\":\"ultra_phase_start\",\"phase_id\":\"setup\",\"phase_index\":1,\"total_phases\":2}\n",
);

#[test]
fn append_completion_summary_bytes_are_pinned() {
    for case in cases() {
        let actual = render_case(&case);
        let expected = GOLDEN
            .iter()
            .find(|(name, _)| *name == case.name)
            .unwrap_or_else(|| panic!("no golden for {}", case.name))
            .1;
        assert_eq!(actual, expected, "case {}", case.name);
    }
}

#[test]
fn tui_command_completion_summary_bytes_are_pinned() {
    let mut snapshot = full_snapshot();
    snapshot.release_gate_status = "pass".to_string();
    let projection = project_completion(true, &snapshot);
    for (status, expected) in TUI_GOLDEN {
        let actual = render_tui(&projection, status, TUI_EVENTS);
        assert_eq!(&actual, expected, "terminal status {status}");
    }
}

const GOLDEN: &[(&str, &str)] = &[
    (
        "complete_process_full",
        r####"Status: complete
Lifecycle: process
Process: REPL exited cleanly (not task status)
Session/REPL status: process_exited
Action: run
Command status: completed
Command completion: completed
Task status: complete
Effective profile: web
Prompt layout: balanced
Contract origin: promoted_union
Runtime acceptance: pass
Final acceptance: pass
Release gate: pass
Requested port: 3000
Evidence arbitration: static (agree)
Depth profile: standard
completion_contract_verification_enabled=true
completion_contract_path_merge_enabled=true
completion_contract_path=contract.json
completion_contract_generated=true
external_contract_checked=true
external_contract_ok=true
browser_readiness_applicable=true
browser_readiness_execution_status=executed
interaction_evidence_applicable=true
interaction_evidence_execution_status=executed
Planner repaired: true
Planner release risk: true
Planner diagnostics: normalizations=2 retries=1 quality_warnings=3 quality_issues=4
Context truncation warning: suspected (warnings=2)
Release quality completion: release_ready
Release gate reasons:
- none
Browser readiness: ready
Browser readiness evidence: evidence/browser.json
Interaction evidence: pass
Interaction evidence path: evidence/interaction.json
State dimensions changed: score, level
Action hooks: start, restart
Surface fit: fits
Text entry target: #name
Typed token: tok-1
Token echoed: yes
Text input state change: changed
Next action: none
Recovery next action: none
Stop reason: done cleanly
Profile: web
profile_inferred: web (from: prompt)
Assurance: behavioral (probe passed)
Surface fit guidance: keep the canvas visible
Plan adherence:
Present tokens:
- alpha
- beta
Missing tokens:
- gamma
Compile rollback applied:
- rolled back src/a.rs
Recovery handoff:
- Recovery prompt saved: recovery/prompt.md
- Recovery UltraPlan YAML saved: recovery/plan.yaml
- Suggested command: commandagent --resume
- Suggested YAML command: commandagent --plan recovery/plan.yaml"####,
    ),
    (
        "missing_everything",
        r####"Status: complete
Lifecycle: process
Process: REPL exited cleanly (not task status)
Session/REPL status: process_exited
Command status: completed
Command completion: completed
Task status: complete
Effective profile: missing
Prompt layout: missing
Contract origin: initial
Runtime acceptance: not_checked
Final acceptance: not_checked
Release gate: not_applicable
Requested port: missing
Evidence arbitration: missing
Depth profile: missing
completion_contract_verification_enabled=false
completion_contract_path_merge_enabled=false
completion_contract_path=missing
completion_contract_generated=false
external_contract_checked=false
external_contract_ok=false
browser_readiness_applicable=false
browser_readiness_execution_status=not_applicable
interaction_evidence_applicable=false
interaction_evidence_execution_status=not_applicable
Planner repaired: false
Planner release risk: false
Planner diagnostics: normalizations=0 retries=0 quality_warnings=0 quality_issues=0
Context truncation warning: none
Release quality completion: not_checked
Release gate reasons:
- none
Browser readiness: not_applicable
Browser readiness evidence: missing
Interaction evidence: not_applicable
Interaction evidence path: missing
State dimensions changed: none
Action hooks: none
Surface fit: missing
Text entry target: missing
Typed token: missing
Token echoed: missing
Text input state change: missing
Next action: run_acceptance_or_review_changes
Recovery next action: run_acceptance_or_review_changes
Stop reason: unknown"####,
    ),
    (
        "failed_tui_command",
        r####"Status: incomplete
Lifecycle: tui_command
Process: REPL exited cleanly (not task status)
Session/REPL status: repl_ready
Command: /run-ultra
Command status: failed
Command completion: failed
Task status: failed
Effective profile: missing
Prompt layout: missing
Contract origin: initial
Runtime acceptance: not_checked
Final acceptance: not_checked
Release gate: not_applicable
Requested port: missing
Evidence arbitration: missing
Depth profile: missing
completion_contract_verification_enabled=false
completion_contract_path_merge_enabled=false
completion_contract_path=missing
completion_contract_generated=false
external_contract_checked=false
external_contract_ok=false
browser_readiness_applicable=false
browser_readiness_execution_status=not_applicable
interaction_evidence_applicable=false
interaction_evidence_execution_status=not_applicable
Planner repaired: false
Planner release risk: false
Planner diagnostics: normalizations=0 retries=0 quality_warnings=0 quality_issues=0
Context truncation warning: none
Release quality completion: not_checked
Release gate reasons:
- none
Browser readiness: not_applicable
Browser readiness evidence: missing
Interaction evidence: not_applicable
Interaction evidence path: missing
State dimensions changed: none
Action hooks: none
Surface fit: missing
Text entry target: missing
Typed token: missing
Token echoed: missing
Text input state change: missing
Next action: fix_command_failure
Recovery next action: fix_command_failure
Stop reason: verify failed
Paths:
- src/lib.rs
Commands:
- cargo test
Failure kind: verification_failed
TUI command failed: verify failed"####,
    ),
    (
        "partial_probe_unavailable",
        r####"Status: complete_with_partial_release_gate
Lifecycle: tui_command
Process: REPL exited cleanly (not task status)
Session/REPL status: repl_ready
Action: create
Command: /create game
Command status: completed
Command completion: completed
Task status: partial (interaction unverified)
Effective profile: game
Prompt layout: missing
Contract origin: initial
Runtime acceptance: not_checked
Final acceptance: partial
Release gate: partial
Requested port: missing
Evidence arbitration: missing
Depth profile: missing
completion_contract_verification_enabled=false
completion_contract_path_merge_enabled=false
completion_contract_path=missing
completion_contract_generated=false
external_contract_checked=false
external_contract_ok=false
browser_readiness_applicable=false
browser_readiness_execution_status=not_applicable
interaction_evidence_applicable=false
interaction_evidence_execution_status=not_applicable
Planner repaired: false
Planner release risk: false
Planner diagnostics: normalizations=0 retries=0 quality_warnings=0 quality_issues=0
Context truncation warning: none
Release quality completion: partial
Release gate reasons:
- interaction_unverified:probe_unavailable
Browser readiness: not_applicable
Browser readiness evidence: missing
Interaction evidence: not_applicable
Interaction evidence path: missing
State dimensions changed: none
Action hooks: none
Surface fit: missing
Text entry target: missing
Typed token: missing
Token echoed: missing
Text input state change: missing
Next action: run_setup_interaction_probe_to_enable_interaction_release_checks
Recovery next action: run_setup_interaction_probe_to_enable_interaction_release_checks
Stop reason: probe unavailable
Profile: game
Unverified (probe required):
- restart:unverified:terminal_state_not_reached
- fire:unverified:probe_unavailable
run /setup-interaction-probe (or commandagent --setup-interaction-probe) to enable interaction release checks
Restart verification: either expose an in-play restart control, or accept the partial classification (the restart exists but cannot be behaviorally verified by the generic probe).
Interaction verification: interaction_unverified:probe_unavailable; run /setup-interaction-probe (or commandagent --setup-interaction-probe) to enable interaction release checks.
Interaction probe preflight: failed; preflight時点から基盤未準備: chromium missing; run /setup-interaction-probe (or commandagent --setup-interaction-probe) to enable interaction release checks."####,
    ),
    (
        "invalid_infra_failure_default_remediation",
        r####"Status: incomplete
Lifecycle: other_stage
Process: REPL exited cleanly (not task status)
Session/REPL status: unknown
Command status: failed
Command completion: failed
Task status: failed
Effective profile: missing
Prompt layout: missing
Contract origin: initial
Runtime acceptance: not_checked
Final acceptance: failed
Release gate: failed
Requested port: missing
Evidence arbitration: static (probe infrastructure failure: no browser)
Depth profile: missing
completion_contract_verification_enabled=false
completion_contract_path_merge_enabled=false
completion_contract_path=missing
completion_contract_generated=false
external_contract_checked=false
external_contract_ok=false
browser_readiness_applicable=false
browser_readiness_execution_status=not_applicable
interaction_evidence_applicable=false
interaction_evidence_execution_status=not_applicable
Planner repaired: false
Planner release risk: false
Planner diagnostics: normalizations=0 retries=0 quality_warnings=0 quality_issues=0
Context truncation warning: none
Release quality completion: failed
Release gate reasons:
- probe_infrastructure_failed
- other reason
Browser readiness: not_applicable
Browser readiness evidence: missing
Interaction evidence: not_applicable
Interaction evidence path: missing
State dimensions changed: none
Action hooks: none
Surface fit: missing
Text entry target: missing
Typed token: missing
Token echoed: missing
Text input state change: missing
Next action: fix_command_failure
Recovery next action: fix_command_failure
Stop reason: infra
Interaction verification: app interaction untested (probe infrastructure failure).
Interaction probe preflight: failed; preflight時点から基盤未準備: missing; run /setup-interaction-probe (or commandagent --setup-interaction-probe) to enable interaction release checks.
Failure kind: probe_infrastructure"####,
    ),
    (
        "invalid_infra_failure_custom_remediation",
        r####"Status: incomplete
Lifecycle: process
Process: REPL exited cleanly (not task status)
Session/REPL status: process_exited
Command status: failed
Command completion: failed
Task status: failed
Effective profile: missing
Prompt layout: missing
Contract origin: initial
Runtime acceptance: not_checked
Final acceptance: failed
Release gate: failed
Requested port: missing
Evidence arbitration: static (probe infrastructure failure: no browser)
Depth profile: missing
completion_contract_verification_enabled=false
completion_contract_path_merge_enabled=false
completion_contract_path=missing
completion_contract_generated=false
external_contract_checked=false
external_contract_ok=false
browser_readiness_applicable=false
browser_readiness_execution_status=not_applicable
interaction_evidence_applicable=false
interaction_evidence_execution_status=not_applicable
Planner repaired: false
Planner release risk: false
Planner diagnostics: normalizations=0 retries=0 quality_warnings=0 quality_issues=0
Context truncation warning: none
Release quality completion: failed
Release gate reasons:
- probe_infrastructure_failed
- other reason
Browser readiness: not_applicable
Browser readiness evidence: missing
Interaction evidence: not_applicable
Interaction evidence path: missing
State dimensions changed: none
Action hooks: none
Surface fit: missing
Text entry target: missing
Typed token: missing
Token echoed: missing
Text input state change: missing
Next action: fix_command_failure
Recovery next action: fix_command_failure
Stop reason: infra
Interaction verification: app interaction untested (probe infrastructure failure).
Interaction probe preflight: failed; preflight時点から基盤未準備: missing; install chromium.
Failure kind: probe_infrastructure"####,
    ),
];

const TUI_GOLDEN: &[(&str, &str)] = &[
    (
        "completed",
        r####"Status: completed
Completion status: complete
Lifecycle: tui_command
Process: REPL exited cleanly (not task status)
Session/REPL status: repl_ready
Command: /run-ultra
Command status: completed
Command completion: completed
Task status: complete
Effective profile: web
Prompt layout: balanced
Contract origin: promoted_union
Runtime acceptance: pass
Final acceptance: pass
Release gate: pass
Requested port: 3000
Evidence arbitration: static (agree)
Depth profile: standard
completion_contract_verification_enabled=true
completion_contract_path_merge_enabled=true
completion_contract_path=contract.json
completion_contract_generated=true
external_contract_checked=true
external_contract_ok=true
browser_readiness_applicable=true
browser_readiness_execution_status=executed
interaction_evidence_applicable=true
interaction_evidence_execution_status=executed
Planner repaired: true
Planner release risk: true
Planner diagnostics: normalizations=2 retries=1 quality_warnings=3 quality_issues=4
Context truncation warning: suspected (warnings=2)
Release quality completion: release_ready
Release gate reasons:
- none
Browser readiness: ready
Browser readiness evidence: evidence/browser.json
Interaction evidence: pass
Interaction evidence path: evidence/interaction.json
State dimensions changed: score, level
Action hooks: start, restart
Surface fit: fits
Text entry target: #name
Typed token: tok-1
Token echoed: yes
Text input state change: changed
Next action: none
Recovery next action: none
Stop reason: command stopped
Paths:
- src/main.rs
Profile promoted: generic -> web (workspace evidence, phase 2)
Profile: web
profile_inferred: web (from: prompt)
Assurance: behavioral (probe passed)
Surface fit guidance: keep the canvas visible
Plan adherence:
Present tokens:
- alpha
- beta
Missing tokens:
- gamma
Compile rollback applied:
- rolled back src/a.rs
Recovery handoff:
- Recovery prompt saved: recovery/prompt.md
- Recovery UltraPlan YAML saved: recovery/plan.yaml
- Suggested command: commandagent --resume
- Suggested YAML command: commandagent --plan recovery/plan.yaml
Failure kind: command_failure

Completed phases:
- none

Failed phases:
- none

Pending phases:
- setup (pending)
- phase 2 (pending)"####,
    ),
    (
        "partial",
        r####"Status: partial
Completion status: complete
Lifecycle: tui_command
Process: REPL exited cleanly (not task status)
Session/REPL status: repl_ready
Command: /run-ultra
Command status: partial
Command completion: partial
Task status: complete
Effective profile: web
Prompt layout: balanced
Contract origin: promoted_union
Runtime acceptance: pass
Final acceptance: pass
Release gate: pass
Requested port: 3000
Evidence arbitration: static (agree)
Depth profile: standard
completion_contract_verification_enabled=true
completion_contract_path_merge_enabled=true
completion_contract_path=contract.json
completion_contract_generated=true
external_contract_checked=true
external_contract_ok=true
browser_readiness_applicable=true
browser_readiness_execution_status=executed
interaction_evidence_applicable=true
interaction_evidence_execution_status=executed
Planner repaired: true
Planner release risk: true
Planner diagnostics: normalizations=2 retries=1 quality_warnings=3 quality_issues=4
Context truncation warning: suspected (warnings=2)
Release quality completion: release_ready
Release gate reasons:
- none
Browser readiness: ready
Browser readiness evidence: evidence/browser.json
Interaction evidence: pass
Interaction evidence path: evidence/interaction.json
State dimensions changed: score, level
Action hooks: start, restart
Surface fit: fits
Text entry target: #name
Typed token: tok-1
Token echoed: yes
Text input state change: changed
Next action: none
Recovery next action: none
Stop reason: command stopped
Paths:
- src/main.rs
Profile promoted: generic -> web (workspace evidence, phase 2)
Profile: web
profile_inferred: web (from: prompt)
Assurance: behavioral (probe passed)
Surface fit guidance: keep the canvas visible
Plan adherence:
Present tokens:
- alpha
- beta
Missing tokens:
- gamma
Compile rollback applied:
- rolled back src/a.rs
Recovery handoff:
- Recovery prompt saved: recovery/prompt.md
- Recovery UltraPlan YAML saved: recovery/plan.yaml
- Suggested command: commandagent --resume
- Suggested YAML command: commandagent --plan recovery/plan.yaml
Failure kind: command_failure

Completed phases:
- none

Failed phases:
- setup (failed)

Pending phases:
- phase 2 (pending)"####,
    ),
    (
        "failed",
        r####"Status: failed
Completion status: complete
Lifecycle: tui_command
Process: REPL exited cleanly (not task status)
Session/REPL status: repl_ready
Command: /run-ultra
Command status: failed
Command completion: failed
Task status: failed
Effective profile: web
Prompt layout: balanced
Contract origin: promoted_union
Runtime acceptance: pass
Final acceptance: pass
Release gate: pass
Requested port: 3000
Evidence arbitration: static (agree)
Depth profile: standard
completion_contract_verification_enabled=true
completion_contract_path_merge_enabled=true
completion_contract_path=contract.json
completion_contract_generated=true
external_contract_checked=true
external_contract_ok=true
browser_readiness_applicable=true
browser_readiness_execution_status=executed
interaction_evidence_applicable=true
interaction_evidence_execution_status=executed
Planner repaired: true
Planner release risk: true
Planner diagnostics: normalizations=2 retries=1 quality_warnings=3 quality_issues=4
Context truncation warning: suspected (warnings=2)
Release quality completion: release_ready
Release gate reasons:
- none
Browser readiness: ready
Browser readiness evidence: evidence/browser.json
Interaction evidence: pass
Interaction evidence path: evidence/interaction.json
State dimensions changed: score, level
Action hooks: start, restart
Surface fit: fits
Text entry target: #name
Typed token: tok-1
Token echoed: yes
Text input state change: changed
Next action: fix_command_failure
Recovery next action: fix_command_failure
Stop reason: command stopped
Paths:
- src/main.rs
Profile promoted: generic -> web (workspace evidence, phase 2)
Profile: web
profile_inferred: web (from: prompt)
Assurance: behavioral (probe passed)
Surface fit guidance: keep the canvas visible
Plan adherence:
Present tokens:
- alpha
- beta
Missing tokens:
- gamma
Compile rollback applied:
- rolled back src/a.rs
Recovery handoff:
- Recovery prompt saved: recovery/prompt.md
- Recovery UltraPlan YAML saved: recovery/plan.yaml
- Suggested command: commandagent --resume
- Suggested YAML command: commandagent --plan recovery/plan.yaml
Failure kind: command_failure
TUI command failed: command stopped

Completed phases:
- none

Failed phases:
- setup (failed)

Pending phases:
- phase 2 (pending)"####,
    ),
    (
        "aborted",
        r####"Status: aborted
Completion status: complete
Lifecycle: tui_command
Process: aborted
Session/REPL status: repl_ready
Command: /run-ultra
Command status: aborted
Command completion: aborted
Task status: aborted
Effective profile: web
Prompt layout: balanced
Contract origin: promoted_union
Runtime acceptance: pass
Final acceptance: pass
Release gate: pass
Requested port: 3000
Evidence arbitration: static (agree)
Depth profile: standard
completion_contract_verification_enabled=true
completion_contract_path_merge_enabled=true
completion_contract_path=contract.json
completion_contract_generated=true
external_contract_checked=true
external_contract_ok=true
browser_readiness_applicable=true
browser_readiness_execution_status=executed
interaction_evidence_applicable=true
interaction_evidence_execution_status=executed
Planner repaired: true
Planner release risk: true
Planner diagnostics: normalizations=2 retries=1 quality_warnings=3 quality_issues=4
Context truncation warning: suspected (warnings=2)
Release quality completion: release_ready
Release gate reasons:
- none
Browser readiness: ready
Browser readiness evidence: evidence/browser.json
Interaction evidence: pass
Interaction evidence path: evidence/interaction.json
State dimensions changed: score, level
Action hooks: start, restart
Surface fit: fits
Text entry target: #name
Typed token: tok-1
Token echoed: yes
Text input state change: changed
Next action: inspect_summary_and_resume_or_rerun
Recovery next action: inspect_summary_and_resume_or_rerun
Stop reason: command stopped
Paths:
- src/main.rs
Profile promoted: generic -> web (workspace evidence, phase 2)
Profile: web
profile_inferred: web (from: prompt)
Assurance: behavioral (probe passed)
Surface fit guidance: keep the canvas visible
Plan adherence:
Present tokens:
- alpha
- beta
Missing tokens:
- gamma
Compile rollback applied:
- rolled back src/a.rs
Recovery handoff:
- Recovery prompt saved: recovery/prompt.md
- Recovery UltraPlan YAML saved: recovery/plan.yaml
- Suggested command: commandagent --resume
- Suggested YAML command: commandagent --plan recovery/plan.yaml
Failure kind: command_failure

Completed phases:
- none

Failed phases:
- setup (aborted)

Pending phases:
- phase 2 (pending)"####,
    ),
    (
        "interrupted",
        r####"Status: interrupted
Completion status: complete
Lifecycle: tui_command
Process: interrupted
Session/REPL status: repl_ready
Command: /run-ultra
Command status: interrupted
Command completion: interrupted
Task status: interrupted
Effective profile: web
Prompt layout: balanced
Contract origin: promoted_union
Runtime acceptance: pass
Final acceptance: pass
Release gate: pass
Requested port: 3000
Evidence arbitration: static (agree)
Depth profile: standard
completion_contract_verification_enabled=true
completion_contract_path_merge_enabled=true
completion_contract_path=contract.json
completion_contract_generated=true
external_contract_checked=true
external_contract_ok=true
browser_readiness_applicable=true
browser_readiness_execution_status=executed
interaction_evidence_applicable=true
interaction_evidence_execution_status=executed
Planner repaired: true
Planner release risk: true
Planner diagnostics: normalizations=2 retries=1 quality_warnings=3 quality_issues=4
Context truncation warning: suspected (warnings=2)
Release quality completion: release_ready
Release gate reasons:
- none
Browser readiness: ready
Browser readiness evidence: evidence/browser.json
Interaction evidence: pass
Interaction evidence path: evidence/interaction.json
State dimensions changed: score, level
Action hooks: start, restart
Surface fit: fits
Text entry target: #name
Typed token: tok-1
Token echoed: yes
Text input state change: changed
Next action: resume_or_rerun_command
Recovery next action: resume_or_rerun_command
Stop reason: command stopped
Paths:
- src/main.rs
Profile promoted: generic -> web (workspace evidence, phase 2)
Profile: web
profile_inferred: web (from: prompt)
Assurance: behavioral (probe passed)
Surface fit guidance: keep the canvas visible
Plan adherence:
Present tokens:
- alpha
- beta
Missing tokens:
- gamma
Compile rollback applied:
- rolled back src/a.rs
Recovery handoff:
- Recovery prompt saved: recovery/prompt.md
- Recovery UltraPlan YAML saved: recovery/plan.yaml
- Suggested command: commandagent --resume
- Suggested YAML command: commandagent --plan recovery/plan.yaml
Failure kind: command_failure

Completed phases:
- none

Failed phases:
- setup (interrupted)

Pending phases:
- phase 2 (pending)"####,
    ),
];
