fn hollow_canvas_game_page_source() -> &'static str {
    r#""use client";
import { useState } from "react";
export default function Page(){
  const [mode, setMode] = useState("menu");
  return <main><button onClick={() => setMode("playing")}>Start</button><canvas /><p>score 0 health 3 {mode}</p></main>;
}
"#
}

fn unattached_canvas_ref_game_page_source() -> &'static str {
    r#""use client";
import { useEffect, useRef, useState } from "react";
import { useGame } from "./useGame";

export default function Page() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [score, setScore] = useState(0);
  const [gameOver, setGameOver] = useState(false);
  const [screen, setScreen] = useState("menu");
  const [bullets, setBullets] = useState<{ x: number; y: number }[]>([]);
  const [enemies, setEnemies] = useState([{ x: 10, y: 20 }]);
  useGame(canvasRef);
  const fireBullet = () => {
setScreen("playing");
setBullets((items) => [...items, { x: 10, y: 90 }]);
setScore((value) => value + 1);
  };
  const restart = () => {
setScreen("menu");
setGameOver(false);
setScore(0);
setBullets([]);
setEnemies([{ x: 10, y: 20 }]);
  };
  useEffect(() => {
const onKeyDown = (event: KeyboardEvent) => {
  if (event.key === "ArrowLeft" || event.key === " ") {
    fireBullet();
  }
};
const frame = requestAnimationFrame(() => {
  bullets.forEach((bullet) => {
    enemies.forEach((enemy) => {
      if (Math.abs(bullet.x - enemy.x) < 12 && Math.abs(bullet.y - enemy.y) < 12) {
        setGameOver(true);
        setScore((value) => value + 10);
      }
    });
  });
  setEnemies((items) => items.map((enemy) => ({ ...enemy, x: enemy.x + 1 })));
});
window.addEventListener("keydown", onKeyDown);
return () => {
  cancelAnimationFrame(frame);
  window.removeEventListener("keydown", onKeyDown);
};
  }, [bullets, enemies]);
  return (
<main data-anvil-state={JSON.stringify({ screen, score, gameOver, bullets, enemies })}>
  <button data-anvil-action="primary" onClick={fireBullet}>Start</button>
  <button data-anvil-action="restart" onClick={restart}>Restart</button>
  <canvas width={800} height={600} />
  <p>score {score} enemy collision {gameOver ? "game over" : screen}</p>
</main>
  );
}
"#
}

fn attached_canvas_ref_game_page_source() -> String {
    unattached_canvas_ref_game_page_source().replace(
        "<canvas width={800} height={600} />",
        "<canvas ref={canvasRef} width={800} height={600} />",
    )
}

fn canvas_ref_game_hook_source() -> &'static str {
    r##"import { useEffect, type RefObject } from "react";

export function useGame(canvasRef: RefObject<HTMLCanvasElement | null>) {
  useEffect(() => {
const canvas = canvasRef.current;
if (!canvas) return;
const ctx = canvas.getContext("2d");
if (!ctx) return;
let frame = 0;
const draw = () => {
  frame += 1;
  ctx.fillStyle = "#111827";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#22c55e";
  ctx.fillRect(40 + frame, 500, 60, 20);
  requestAnimationFrame(draw);
};
const id = requestAnimationFrame(draw);
return () => cancelAnimationFrame(id);
  }, [canvasRef]);
}
"##
}

fn interaction_state_missing_probe_result() -> Value {
    serde_json::json!({
        "ok": false,
        "status": "failed",
        "interaction_success": false,
        "interaction_performed": false,
        "input_event_observed": true,
        "start_transition": true,
        "input_state_evaluated_after_start": true,
        "input_state_change": false,
        "state_changed": false,
        "visible_state_changed": false,
        "probe_mode": "heuristic",
        "contract_hook_status": "primary_missing",
        "candidate_table": [
            {"rank": 1, "index": 0, "text_excerpt": "", "changed": false},
            {"rank": 2, "index": 1, "text_excerpt": "Start", "changed": true}
        ],
        "input_dispatches": [
            "ArrowLeft keydown",
            "ArrowRight keydown",
            "Space keydown",
            "canvas/center click"
        ],
        "informational_failure_kinds": ["primary_start_transition_missing"],
        "steps": ["surface_visible", "start_transition", "control_input_dispatched", "input_state_evaluated_after_start"],
        "before_marker": "screen=menu score=0 health=3",
        "after_marker": "screen=playing score=0 health=3",
        "input_before_marker": "player=20 score=0 health=3",
        "input_after_marker": "player=20 score=0 health=3",
        "recovery_transition": true,
        "recovery_transition_status": "observed",
        "failure_kind": "input_state_change_missing_after_start",
        "duration_ms": 17
    })
}

fn interaction_state_changed_probe_result() -> Value {
    serde_json::json!({
        "ok": true,
        "status": "passed",
        "probe_mode": "contract",
        "contract_hook_status": "usable",
        "contract_hooks": {
            "usable": true,
            "primary_present": true,
            "restart_present": true,
            "valid_state_count": 1
        },
        "action_hooks": ["primary", "restart"],
        "state_dimensions_changed": ["playerX", "score"],
        "restart_hook_reachable_after_start": true,
        "restart_hook_count_after_start": 1,
        "interaction_success": true,
        "interaction_performed": true,
        "input_event_observed": true,
        "start_transition": true,
        "input_state_evaluated_after_start": true,
        "input_state_change": true,
        "state_changed": true,
        "visible_state_changed": true,
        "steps": [
            "surface_visible",
            "start_transition",
            "control_input_dispatched",
            "input_state_evaluated_after_start",
            "input_state_change",
            "recovery_transition"
        ],
        "before_marker": "screen=menu score=0 health=3",
        "after_marker": "screen=playing score=0 health=3",
        "input_before_marker": "player=20 score=0 health=3",
        "input_after_marker": "player=15 score=1 health=3",
        "recovery_transition": true,
        "recovery_transition_status": "observed",
        "duration_ms": 19
    })
}

fn recovery_not_observed_probe_result() -> Value {
    serde_json::json!({
        "ok": true,
        "status": "passed",
        "probe_mode": "contract",
        "contract_hook_status": "usable",
        "contract_hooks": {
            "usable": true,
            "primary_present": true,
            "restart_present": false,
            "valid_state_count": 1
        },
        "action_hooks": ["primary"],
        "state_dimensions_changed": ["playerX", "score"],
        "restart_hook_reachable_after_start": false,
        "restart_hook_count_after_start": 0,
        "interaction_success": true,
        "interaction_performed": true,
        "input_event_observed": true,
        "start_transition": true,
        "input_state_evaluated_after_start": true,
        "input_state_change": true,
        "state_changed": true,
        "visible_state_changed": true,
        "steps": [
            "surface_visible",
            "start_transition",
            "control_input_dispatched",
            "input_state_evaluated_after_start",
            "input_state_change",
            "recovery_transition:not_observed"
        ],
        "before_marker": "screen=menu",
        "after_marker": "screen=playing",
        "input_before_marker": "player=20 score=0",
        "input_after_marker": "player=15 score=1",
        "recovery_before_marker": "screen=playing",
        "recovery_after_marker": "screen=playing",
        "recovery_transition": false,
        "recovery_transition_status": "not_observed",
        "duration_ms": 23
    })
}

fn contract_interaction_pass_json() -> String {
    serde_json::to_string(&interaction_state_changed_probe_result()).unwrap()
}

#[cfg(unix)]
fn probe_nextjs_scaffold_tool_calls(
    port: u16,
    page: &str,
    check_path: &str,
) -> Vec<crate::state::ToolCall> {
    vec![
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({
                "path":"package.json",
                "content": format!(
                    r#"{{"scripts":{{"build":"next build","dev":"next dev -p {port}","start":"next start -p {port}"}},"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}},"devDependencies":{{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"}}}}"#
                )
            }),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"tsconfig.json","content":nextjs_tsconfig_json()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"postcss.config.js","content":nextjs_postcss_config()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"tailwind.config.ts","content":nextjs_tailwind_config_ts()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/page.tsx","content":page}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/layout.tsx","content":nextjs_layout_source()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/globals.css","content":nextjs_globals_css()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/global.d.ts","content":"declare module \"*.css\";"}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":check_path,"content":"x = 1\n"}),
        ),
    ]
}

#[cfg(unix)]
fn probe_nextjs_scaffold_reply(port: u16, page: String) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: probe_nextjs_scaffold_tool_calls(port, &page, "check_scaffold.py"),
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn interactive_game_page_variant(label: usize) -> String {
    interactive_game_page_source()
        .replace("score {score}", &format!("score {{score}} health {label}"))
}

fn contract_interactive_game_page_variant(label: usize) -> String {
    contract_interactive_game_page_source()
        .replace("score {score}", &format!("score {{score}} health {label}"))
}

fn contract_interactive_game_page_without_restart_variant(label: usize) -> String {
    contract_interactive_game_page_without_restart_source()
        .replace("score {score}", &format!("score {{score}} health {label}"))
}

