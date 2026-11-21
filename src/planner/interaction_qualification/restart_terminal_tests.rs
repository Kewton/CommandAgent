use serde_json::{Value, json};

use super::{TERMINAL_STATE_NOT_REACHED_REASON, source_has_restart_hook};
use crate::planner::adjudication::{
    final_acceptance_status_from_release_gate, release_quality_from_gate_status,
};
use crate::planner::interaction_qualification::{
    enforce_release_gate, qualify_interaction_evidence,
};
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID as PROFILE_ID;

const OVERLAY_PAGE: &str =
    include_str!("../../../tests/corpus/apps/restart-overlay-only-partial/src/app/page.tsx");
const OVERLAY_EVIDENCE: &str = include_str!(
    "../../../tests/corpus/apps/restart-overlay-only-partial/browser-interaction.json"
);
const GAME_LOGIC: &str =
    include_str!("../../../tests/corpus/apps/restart-overlay-only-partial/src/lib/game-logic.ts");
const GAME_TYPES: &str =
    include_str!("../../../tests/corpus/apps/restart-overlay-only-partial/src/lib/types.ts");
const RESTART_HOOK: &str = "data-anvil-action=\"restart\"";
const RESTART_MISSING: &str = "contract_instrumentation_missing:restart";

#[derive(Debug)]
struct Gate {
    status: String,
    reasons: Vec<String>,
    interaction_status: String,
}

fn files_with_page(page: &str) -> Vec<(&str, &str)> {
    vec![
        ("src/app/page.tsx", page),
        ("src/lib/game-logic.ts", GAME_LOGIC),
        ("src/lib/types.ts", GAME_TYPES),
    ]
}

fn evidence() -> Value {
    serde_json::from_str(OVERLAY_EVIDENCE).unwrap()
}

fn run_gate(
    files: &[(&str, &str)],
    evidence: &Value,
    restart_required: bool,
    initial_status: &str,
) -> Gate {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
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

fn assert_failed_on_restart(gate: &Gate) {
    assert_eq!(gate.status, "failed", "{gate:?}");
    assert_eq!(gate.reasons, [RESTART_MISSING], "{gate:?}");
    assert_eq!(
        gate.interaction_status,
        format!("failed:{RESTART_MISSING}"),
        "{gate:?}"
    );
}

fn assert_never_full(gate: &Gate) {
    assert_ne!(gate.status, "pass", "{gate:?}");
    assert_ne!(
        final_acceptance_status_from_release_gate(&gate.status),
        "full_success",
        "{gate:?}"
    );
    assert_ne!(
        release_quality_from_gate_status(&gate.status, "partial"),
        "release_ready",
        "{gate:?}"
    );
}

#[test]
fn overlay_only_restart_with_strong_reset_handler_is_partial_not_full() {
    let value = evidence();
    let qualified = qualify_interaction_evidence(&value, true);
    assert!(!qualified.full_eligible, "{qualified:?}");

    let gate = run_gate(&files_with_page(OVERLAY_PAGE), &value, true, "pass");

    assert_eq!(gate.status, "partial", "{gate:?}");
    assert_eq!(
        gate.reasons,
        [TERMINAL_STATE_NOT_REACHED_REASON],
        "{gate:?}"
    );
    assert_eq!(
        gate.interaction_status, "unverified:terminal_state_not_reached",
        "{gate:?}"
    );
    assert_eq!(
        final_acceptance_status_from_release_gate(&gate.status),
        "partial"
    );
    assert_eq!(
        release_quality_from_gate_status(&gate.status, "partial"),
        "partial"
    );
    assert_never_full(&gate);
}

#[test]
fn already_partial_gate_stays_partial_and_keeps_its_reasons() {
    let value = evidence();
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
    std::fs::create_dir_all(dir.path().join("src/lib")).unwrap();
    std::fs::write(dir.path().join("src/app/page.tsx"), OVERLAY_PAGE).unwrap();
    std::fs::write(dir.path().join("src/lib/game-logic.ts"), GAME_LOGIC).unwrap();
    let evidence_path = dir.path().join("browser-interaction.json");
    std::fs::write(&evidence_path, value.to_string()).unwrap();
    let mut status = "partial".to_string();
    let mut reasons = vec!["interaction_unverified:other".to_string()];
    let mut interaction_status = "passed".to_string();

    enforce_release_gate(
        &mut status,
        &mut reasons,
        &mut interaction_status,
        evidence_path.to_str().unwrap(),
        true,
        dir.path(),
        PROFILE_ID,
    );

    assert_eq!(status, "partial");
    assert_eq!(
        reasons,
        [
            "interaction_unverified:other",
            TERMINAL_STATE_NOT_REACHED_REASON
        ]
    );
}

#[test]
fn source_without_restart_hook_stays_failed() {
    let page = OVERLAY_PAGE.replace(RESTART_HOOK, "data-anvil-label=\"restart\"");

    let gate = run_gate(&files_with_page(&page), &evidence(), true, "pass");

    assert_failed_on_restart(&gate);
    assert_never_full(&gate);
}

#[test]
fn restart_reachable_during_play_keeps_the_gate_unchanged() {
    let mut value = evidence();
    value["action_hooks"] = json!(["primary", "restart"]);
    value["restart_hook_reachable_after_start"] = json!(true);
    value["restart_hook_count_after_start"] = json!(1);

    let gate = run_gate(&files_with_page(OVERLAY_PAGE), &value, true, "pass");

    assert_eq!(gate.status, "pass", "{gate:?}");
    assert!(gate.reasons.is_empty(), "{gate:?}");
    assert_eq!(gate.interaction_status, "passed", "{gate:?}");
}

#[test]
fn restart_not_required_keeps_the_gate_unchanged() {
    let gate = run_gate(&files_with_page(OVERLAY_PAGE), &evidence(), false, "pass");

    assert_eq!(gate.status, "pass", "{gate:?}");
    assert!(gate.reasons.is_empty(), "{gate:?}");
}

#[test]
fn missing_primary_or_state_change_stays_failed() {
    let mut no_primary = evidence();
    no_primary["action_hooks"] = json!([]);
    let mut no_state_change = evidence();
    no_state_change["state_dimensions_changed"] = json!([]);

    for value in [no_primary, no_state_change] {
        let gate = run_gate(&files_with_page(OVERLAY_PAGE), &value, true, "pass");

        assert_eq!(gate.status, "failed", "{gate:?}");
        assert!(
            !gate
                .reasons
                .contains(&TERMINAL_STATE_NOT_REACHED_REASON.to_string()),
            "{gate:?}"
        );
        assert!(
            gate.reasons.contains(&RESTART_MISSING.to_string()),
            "{gate:?}"
        );
        assert_never_full(&gate);
    }
}

#[test]
fn restart_hook_only_in_unrouted_file_stays_failed() {
    let page = OVERLAY_PAGE.replace(RESTART_HOOK, "data-anvil-label=\"restart\"");

    let gate = run_gate(
        &[
            ("src/app/page.tsx", &page),
            ("src/components/Unused.tsx", OVERLAY_PAGE),
        ],
        &evidence(),
        true,
        "pass",
    );

    assert_failed_on_restart(&gate);
    assert_never_full(&gate);
}

#[test]
fn restart_hook_without_strong_reset_handler_stays_failed() {
    let page = OVERLAY_PAGE.replace(
        "    setGameState(initGame());\n    setParticles([]);\n    setShake(false);\n",
        "    console.log('restart');\n",
    );
    assert_ne!(page, OVERLAY_PAGE, "fixture no longer has the reset body");

    let gate = run_gate(&files_with_page(&page), &evidence(), true, "pass");

    assert_failed_on_restart(&gate);
    assert_never_full(&gate);
}

#[test]
fn partial_never_lifts_a_gate_that_already_failed() {
    let gate = run_gate(&files_with_page(OVERLAY_PAGE), &evidence(), true, "failed");

    assert_eq!(gate.status, "failed", "{gate:?}");
    assert_eq!(gate.reasons, [RESTART_MISSING], "{gate:?}");
    assert_never_full(&gate);
}

#[test]
fn unreadable_evidence_stays_failed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
    std::fs::write(dir.path().join("src/app/page.tsx"), OVERLAY_PAGE).unwrap();
    let missing = dir.path().join("absent.json");
    let mut status = "pass".to_string();
    let mut reasons = Vec::new();
    let mut interaction_status = "passed".to_string();

    enforce_release_gate(
        &mut status,
        &mut reasons,
        &mut interaction_status,
        missing.to_str().unwrap(),
        true,
        dir.path(),
        PROFILE_ID,
    );

    assert_eq!(status, "failed");
    assert_eq!(reasons, ["contract_instrumentation_evidence_unreadable"]);
}

#[test]
fn restart_hook_detection_reads_attribute_forms_and_ignores_comments() {
    for accepted in [
        "<button data-anvil-action=\"restart\">",
        "<button data-anvil-action='restart'>",
        "<button data-anvil-action={\"restart\"}>",
        "<button data-anvil-action = { `restart` }>",
        "const url = \"http://x\"; <b data-anvil-action=\"restart\" />",
    ] {
        assert!(source_has_restart_hook(accepted), "{accepted}");
    }
    for rejected in [
        "// <button data-anvil-action=\"restart\">",
        "/* data-anvil-action=\"restart\" */",
        "{/* <b data-anvil-action=\"restart\" /> */}",
        "<button data-anvil-action=\"primary\">",
        "<button data-anvil-action=\"restarting\">",
        "<button data-anvil-action={mode}>",
    ] {
        assert!(!source_has_restart_hook(rejected), "{rejected}");
    }
}

fn handler_page(import_line: &str, handler_call: &str) -> String {
    format!(
        r#""use client";
import React, {{ useState }} from 'react';
{import_line}

export default function Page() {{
  const [gameState, setGameState] = useState<any>({{ status: 'IDLE', score: 0 }});
  const handleRestart = () => {{
    {handler_call}
  }};
  return (
    <div data-anvil-state={{JSON.stringify(gameState)}}>
      {{gameState.status === 'IDLE' && <button onClick={{() => setGameState({{ status: 'PLAYING', score: 0 }})}} data-anvil-action="primary">Start</button>}}
      {{gameState.status === 'GAMEOVER' && <button onClick={{handleRestart}} data-anvil-action="restart">Again</button>}}
    </div>
  );
}}
"#
    )
}

fn factory(literal_body: &str) -> String {
    format!("export function initGame() {{\n  return {{ {literal_body} }};\n}}\n")
}

const IMPORT_INIT: &str = "import { initGame } from '@/lib/game-logic';";

fn gate_for_factory(handler_call: &str, lib: &str) -> Gate {
    let page = handler_page(IMPORT_INIT, handler_call);
    run_gate(
        &[("src/app/page.tsx", &page), ("src/lib/game-logic.ts", lib)],
        &evidence(),
        true,
        "pass",
    )
}

#[test]
fn restart_handler_replacing_state_with_reset_factory_is_partial() {
    let gate = gate_for_factory(
        "setGameState(initGame());",
        &factory("status: 'IDLE', bullets: [], score: 0, level: 1"),
    );

    assert_eq!(gate.status, "partial", "{gate:?}");
    assert_eq!(
        gate.reasons,
        [TERMINAL_STATE_NOT_REACHED_REASON],
        "{gate:?}"
    );
    assert_never_full(&gate);
}

#[test]
fn factory_with_only_one_reset_key_stays_failed() {
    for body in ["status: 'IDLE', score: 0", "status: 'IDLE', bullets: []"] {
        let gate = gate_for_factory("setGameState(initGame());", &factory(body));

        assert_failed_on_restart(&gate);
        assert_never_full(&gate);
    }
}

#[test]
fn factory_values_that_are_not_zero_or_empty_stay_failed() {
    for body in [
        "status: 'IDLE', bullets: [], score: 5",
        "status: 'IDLE', bullets: [seed], score: 0",
        "status: 'IDLE', bullets: previous.bullets, score: 0",
    ] {
        let gate = gate_for_factory("setGameState(initGame());", &factory(body));

        assert_failed_on_restart(&gate);
        assert_never_full(&gate);
    }
}

#[test]
fn discarded_factory_return_value_stays_failed() {
    let gate = gate_for_factory(
        "initGame();\n    setGameState({ status: 'PLAYING', score: 1 });",
        &factory("status: 'IDLE', bullets: [], score: 0"),
    );

    assert_failed_on_restart(&gate);
    assert_never_full(&gate);
}

#[test]
fn factory_two_calls_away_stays_failed() {
    let page = handler_page(
        "import { restartRound } from '@/lib/round';",
        "setGameState(restartRound());",
    );
    let round = "import { initGame } from './game-logic';\n\nexport function restartRound() {\n  return initGame();\n}\n";
    let lib = factory("status: 'IDLE', bullets: [], score: 0");

    let gate = run_gate(
        &[
            ("src/app/page.tsx", &page),
            ("src/lib/round.ts", round),
            ("src/lib/game-logic.ts", &lib),
        ],
        &evidence(),
        true,
        "pass",
    );

    assert_failed_on_restart(&gate);
    assert_never_full(&gate);
}

#[test]
fn factory_outside_route_bound_closure_stays_failed() {
    let page = handler_page("", "setGameState(initGame());");
    let lib = factory("status: 'IDLE', bullets: [], score: 0");

    let gate = run_gate(
        &[("src/app/page.tsx", &page), ("src/lib/game-logic.ts", &lib)],
        &evidence(),
        true,
        "pass",
    );

    assert_failed_on_restart(&gate);
    assert_never_full(&gate);
}

#[test]
fn ambiguous_factory_definition_stays_failed() {
    let once = factory("status: 'IDLE', bullets: [], score: 0");
    let doubled = format!("{once}\n{once}");

    let gate = gate_for_factory("setGameState(initGame());", &doubled);

    assert_failed_on_restart(&gate);
    assert_never_full(&gate);
}

#[test]
fn factory_form_is_judged_like_the_equivalent_inline_reset() {
    let lib = factory("status: 'IDLE', bullets: [], score: 0");
    for (factory_call, inline_call) in [
        (
            "setWorld(initGame());",
            "setWorld({ status: 'IDLE' }); score = 0; bullets = [];",
        ),
        (
            "setGameState(initGame());",
            "setGameState({ status: 'IDLE' }); score = 0; bullets = [];",
        ),
    ] {
        let from_factory = gate_for_factory(factory_call, &lib);
        let inline_page = handler_page("", inline_call);
        let inline = run_gate(
            &[("src/app/page.tsx", &inline_page)],
            &evidence(),
            true,
            "pass",
        );

        assert_eq!(from_factory.status, inline.status, "{factory_call}");
        assert_eq!(from_factory.reasons, inline.reasons, "{factory_call}");
        assert_never_full(&from_factory);
    }
}

const RESET_ARROW: &str = "export const emptyRound = () => ({ score: 0, bullets: [] });\n";

#[test]
fn reset_arrow_after_a_non_resetting_factory_stays_failed() {
    for lib in [
        format!(
            "export const initGame = function () {{\n  return {{ status: 'IDLE', score: 5, bullets: [seed] }};\n}};\n{RESET_ARROW}"
        ),
        format!("export const initGame = buildInitial;\n{RESET_ARROW}"),
        format!("export const initGame: Factory = buildInitial;\n{RESET_ARROW}"),
    ] {
        let gate = gate_for_factory("setGameState(initGame());", &lib);

        assert_failed_on_restart(&gate);
        assert_never_full(&gate);
    }
}

#[test]
fn reset_object_inside_nested_arrow_or_function_stays_failed() {
    for lib in [
        "export function initGame() {\n  return xs.map(x => ({ score: 0, bullets: [] }));\n}\n",
        "export function initGame() {\n  function inner() { return { score: 0, bullets: [] }; }\n  return inner();\n}\n",
        "export function initGame() {\n  if (flag) { return { score: 0, bullets: [] }; }\n  return other;\n}\n",
    ] {
        let gate = gate_for_factory("setGameState(initGame());", lib);

        assert_failed_on_restart(&gate);
        assert_never_full(&gate);
    }
}

#[test]
fn arrow_factories_with_block_or_concise_bodies_are_partial() {
    for lib in [
        "export const initGame = (): State => ({ status: 'IDLE', score: 0, bullets: [] });\n",
        "export const initGame = async () => {\n  return { status: 'IDLE', score: 0, bullets: [] };\n};\n",
        "export const initGame: () => State = function () {\n  return { status: 'IDLE', score: 0, bullets: [] };\n};\n",
    ] {
        let gate = gate_for_factory("setGameState(initGame());", lib);

        assert_eq!(gate.status, "partial", "{lib}: {gate:?}");
        assert_never_full(&gate);
    }
}

#[test]
fn local_declaration_of_the_factory_value_stays_failed() {
    let lib = factory("status: 'IDLE', bullets: [], score: 0");
    for call in [
        "const fresh = initGame();",
        "let fresh = initGame();",
        "const fresh: State = initGame();",
    ] {
        let gate = gate_for_factory(call, &lib);

        assert_failed_on_restart(&gate);
        assert_never_full(&gate);
    }
}
