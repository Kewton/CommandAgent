#[cfg(unix)]
fn write_compile_error_nextjs_workspace(root: &Path, port: u16) -> PathBuf {
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::create_dir_all(root.join("src/components")).unwrap();
    std::fs::create_dir_all(root.join(".anvil")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/next")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/tailwindcss")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/postcss")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/autoprefixer")).unwrap();
    std::fs::write(root.join(".anvil/enable-browser-probe-tests"), "1").unwrap();
    std::fs::write(
        root.join("package.json"),
        format!(
            r#"{{"scripts":{{"build":"next build","dev":"next dev -p {port}","start":"next start -p {port}"}},"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}},"devDependencies":{{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"}}}}"#
        ),
    )
    .unwrap();
    std::fs::write(root.join("node_modules/next/package.json"), "{}").unwrap();
    std::fs::write(root.join("node_modules/tailwindcss/package.json"), "{}").unwrap();
    std::fs::write(root.join("node_modules/postcss/package.json"), "{}").unwrap();
    std::fs::write(root.join("node_modules/autoprefixer/package.json"), "{}").unwrap();
    let component = r#""use client";
import { useState } from "react";
export function SpaceInvaders(){
  const [score, setScore] = useState(0);
  const fire = () => setScore((value) => value + 1);
  return <main><button onClick={fire}>Fire</button><button onClick={reset}>Restart</button><canvas /><p>score {score}</p></main>;
}
"#;
    std::fs::write(root.join("src/components/SpaceInvaders.tsx"), component).unwrap();
    std::fs::write(
        root.join("src/app/page.tsx"),
        "export default function Page(){return <main><button>Plain</button></main>;}\n",
    )
    .unwrap();
    std::fs::write(root.join("src/app/layout.tsx"), nextjs_layout_source()).unwrap();
    std::fs::write(root.join("src/app/globals.css"), nextjs_globals_css()).unwrap();
    std::fs::write(
        root.join("src/app/global.d.ts"),
        "declare module \"*.css\";\n",
    )
    .unwrap();
    std::fs::write(root.join("tsconfig.json"), nextjs_tsconfig_json()).unwrap();
    std::fs::write(root.join("postcss.config.js"), nextjs_postcss_config()).unwrap();
    std::fs::write(root.join("tailwind.config.ts"), nextjs_tailwind_config_ts()).unwrap();
    let exe = shell_quote(&std::env::current_exe().unwrap().display().to_string());
    let script = format!(
        "#!/bin/sh\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then\n\
  if grep -q 'onClick={{reset}}' src/components/SpaceInvaders.tsx && ! grep -q 'const reset' src/components/SpaceInvaders.tsx; then\n\
echo './src/components/SpaceInvaders.tsx:137:28' >&2\n\
echo \"Type error: Cannot find name 'reset'.\" >&2\n\
exit 1\n\
  fi\n\
  echo 'fake build ok'\n\
  exit 0\n\
fi\n\
if [ \"$1\" = \"run\" ] && {{ [ \"$2\" = \"dev\" ] || [ \"$2\" = \"start\" ]; }}; then\n\
  COMMANDAGENT_FAKE_DEV_SERVER_CHILD=1 COMMANDAGENT_FAKE_DEV_SERVER_GRANDCHILD=0 exec {exe} --ignored --exact planner::runner::tests::fake_dev_server_package_manager_child --nocapture\n\
fi\n\
echo \"unexpected fake npm args: $*\" >&2\n\
exit 2\n"
    );
    let npm = root.join("node_modules/.bin/npm");
    std::fs::write(&npm, script).unwrap();
    let mut permissions = std::fs::metadata(&npm).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&npm, permissions).unwrap();
    let next_path = root.join("node_modules/.bin/next");
    std::fs::write(&next_path, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&next_path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(next_path, permissions).unwrap();
    let contract_path = root.join("completion-contract.json");
    std::fs::write(
        &contract_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "required_paths": ["src/app/page.tsx", "src/components/SpaceInvaders.tsx"],
            "verify_commands": ["npm run build"],
            "profile": "nextjs",
            "goal": explicit_port_goal("Create an interactive browser app", port),
            "required_capabilities": ["playable_ui", "stateful_interaction"],
            "verify_repair_cap": 2
        }))
        .unwrap(),
    )
    .unwrap();
    contract_path
}

#[cfg(unix)]
fn write_api_mismatch_build_shim(root: &Path) {
    std::fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/next")).unwrap();
    let script = "#!/bin/sh\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then\n\
  if grep -q 'onStateChange' src/app/SpaceInvadersGame.tsx 2>/dev/null; then\n\
echo './src/app/SpaceInvadersGame.tsx:30:12' >&2\n\
echo \"Type error: Property 'onStateChange' does not exist on type 'SpaceInvadersEngine'.\" >&2\n\
exit 1\n\
  fi\n\
  if ! grep -q 'getState' src/app/SpaceInvadersGame.tsx 2>/dev/null; then\n\
echo './src/app/SpaceInvadersGame.tsx:30:12' >&2\n\
echo \"Type error: expected poll-based getState repair.\" >&2\n\
exit 1\n\
  fi\n\
  echo 'fake build ok'\n\
  exit 0\n\
fi\n\
echo \"unexpected fake npm args: $*\" >&2\n\
exit 2\n";
    let npm = root.join("node_modules/.bin/npm");
    std::fs::write(&npm, script).unwrap();
    let mut permissions = std::fs::metadata(&npm).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&npm, permissions).unwrap();
    let next_path = root.join("node_modules/.bin/next");
    std::fs::write(&next_path, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&next_path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(next_path, permissions).unwrap();
}

fn api_mismatch_step_plan() -> StepPlan {
    StepPlan {
        goal: "Fix a Next.js TypeScript API mismatch on port 3011".to_string(),
        steps: vec![PlanStep {
            id: "verify-build".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create the route-bound game component and engine, then build."
                .to_string(),
            expected_paths: vec![
                "src/app/page.tsx".to_string(),
                "src/app/SpaceInvadersGame.tsx".to_string(),
                "src/lib/game-engine.ts".to_string(),
                "package.json".to_string(),
            ],
            verify: vec!["npm run build".to_string()],
        }],
    }
}

fn api_mismatch_initial_reply(port: u16) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![
            crate::state::ToolCall::new(
                "Write",
                serde_json::json!({
                    "path": "package.json",
                    "content": format!(
                        r#"{{"scripts":{{"build":"next build","dev":"next dev -p {port}","start":"next start -p {port}"}},"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}}}"#
                    )
                }),
            ),
            crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/page.tsx","content":"import SpaceInvadersGame from \"./SpaceInvadersGame\";\n\nexport default function Page() {\n  return <SpaceInvadersGame />;\n}\n"}),
            ),
            crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/SpaceInvadersGame.tsx","content":api_mismatch_broken_game_source()}),
            ),
            crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/lib/game-engine.ts","content":api_mismatch_engine_source()}),
            ),
        ],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn api_mismatch_poll_fix_reply() -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/SpaceInvadersGame.tsx","content":api_mismatch_poll_fixed_game_source()}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn api_mismatch_read_only_reply() -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Read",
            serde_json::json!({"path":"src/app/SpaceInvadersGame.tsx"}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn api_mismatch_insufficient_game_source() -> &'static str {
    r#""use client";
import { useRef, useState } from "react";
import { SpaceInvadersEngine, type GameState } from "../lib/game-engine";

export default function SpaceInvadersGame() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [gameState] = useState<GameState>({ score: 0, status: "ready" });
  void SpaceInvadersEngine;
  return <main data-anvil-state={JSON.stringify(gameState)}><canvas ref={canvasRef} /></main>;
}
"#
}

fn api_mismatch_broken_game_source() -> &'static str {
    r#""use client";
import { useEffect, useRef, useState } from "react";
import { SpaceInvadersEngine, type GameState } from "../lib/game-engine";

export default function SpaceInvadersGame() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [gameState, setGameState] = useState<GameState>({ score: 0, status: "ready" });
  useEffect(() => {
const canvas = canvasRef.current;
if (!canvas) return;
const engine = new SpaceInvadersEngine(canvas);
engine.onStateChange((state) => {
  setGameState({ ...state });
});
engine.start();
return () => engine.destroy();
  }, []);
  return <main data-anvil-state={JSON.stringify(gameState)}><canvas ref={canvasRef} /></main>;
}
"#
}

fn api_mismatch_poll_fixed_game_source() -> &'static str {
    r#""use client";
import { useEffect, useRef, useState } from "react";
import { SpaceInvadersEngine, type GameState } from "../lib/game-engine";

export default function SpaceInvadersGame() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [gameState, setGameState] = useState<GameState>({ score: 0, status: "ready" });
  useEffect(() => {
const canvas = canvasRef.current;
if (!canvas) return;
const engine = new SpaceInvadersEngine(canvas);
let raf = 0;
const tick = () => {
  setGameState({ ...engine.getState() });
  raf = requestAnimationFrame(tick);
};
engine.start();
raf = requestAnimationFrame(tick);
return () => {
  cancelAnimationFrame(raf);
  engine.destroy();
};
  }, []);
  return <main data-anvil-state={JSON.stringify(gameState)}><canvas ref={canvasRef} /></main>;
}
"#
}

fn api_mismatch_engine_source() -> &'static str {
    r#"export interface GameState {
  score: number;
  status: string;
}

export class SpaceInvadersEngine {
  private state: GameState = { score: 0, status: "ready" };
  public start() { this.state = { ...this.state, status: "playing" }; }
  public pause() { this.state = { ...this.state, status: "paused" }; }
  public reset() { this.state = { score: 0, status: "ready" }; }
  public setKey(key: string, pressed: boolean) { void key; void pressed; }
  public getState(): GameState { return this.state; }
  public destroy() { this.state = { ...this.state, status: "destroyed" }; }
}
"#
}

fn generated_nextjs_artifact_plan_json_with_build_verify(goal: &str) -> String {
    let expected_paths = nextjs_scaffold_expected_paths();
    serde_json::to_string(&StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "create-nextjs-artifacts".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!(
                "Create a coherent Next.js scaffold with {}",
                expected_paths.join(", ")
            ),
            expected_paths,
            verify: vec!["npm run build".to_string()],
        }],
    })
    .unwrap()
}

#[cfg(unix)]
fn static_good_page_source() -> &'static str {
    "export default function Page(){return <main>Recovered static app</main>;}\n"
}

#[cfg(unix)]
fn static_broken_page_source() -> &'static str {
    "export default function Page(){\n  return (\n    <main>\n      <p>Broken</p>\nBROKEN_SYNTAX\n    </main>\n  );\n}\n"
}

#[cfg(unix)]
fn write_static_compile_repair_workspace(root: &Path, page: &str) {
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
    for package in ["next", "tailwindcss", "postcss", "autoprefixer"] {
        let dir = root.join("node_modules").join(package);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), "{}").unwrap();
    }
    std::fs::write(root.join("package.json"), nextjs_complete_package_json()).unwrap();
    std::fs::write(root.join("tsconfig.json"), nextjs_tsconfig_json()).unwrap();
    std::fs::write(root.join("postcss.config.js"), nextjs_postcss_config()).unwrap();
    std::fs::write(root.join("tailwind.config.ts"), nextjs_tailwind_config_ts()).unwrap();
    std::fs::write(root.join("src/app/page.tsx"), page).unwrap();
    std::fs::write(root.join("src/app/layout.tsx"), nextjs_layout_source()).unwrap();
    std::fs::write(root.join("src/app/globals.css"), nextjs_globals_css()).unwrap();
    std::fs::write(
        root.join("src/app/global.d.ts"),
        "declare module \"*.css\";\n",
    )
    .unwrap();
    let page_path = root.join("src/app/page.tsx");
    let script = format!(
        "#!/bin/sh\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then\n\
  if grep -q 'BROKEN_SYNTAX' src/app/page.tsx; then\n\
echo 'Failed to compile.' >&2\n\
echo './src/app/page.tsx' >&2\n\
echo 'Error:' >&2\n\
echo \"  x Expected ';', '}}' or <eof>\" >&2\n\
echo '   ,-[{}:12:1]' >&2\n\
echo ' 9 |   return (' >&2\n\
echo '10 |     <main>' >&2\n\
echo '11 |       <p>Broken</p>' >&2\n\
echo '12 | BROKEN_SYNTAX' >&2\n\
echo '   | ^' >&2\n\
exit 1\n\
  fi\n\
  echo 'fake build ok'\n\
  exit 0\n\
fi\n\
echo \"unexpected fake npm args: $*\" >&2\n\
exit 2\n",
        page_path.display()
    );
    let npm = root.join("node_modules/.bin/npm");
    std::fs::write(&npm, script).unwrap();
    let mut permissions = std::fs::metadata(&npm).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&npm, permissions).unwrap();
    let next = root.join("node_modules/.bin/next");
    std::fs::write(&next, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&next).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(next, permissions).unwrap();
}

fn write_static_build_contract(root: &Path) -> PathBuf {
    let path = root.join("completion-contract.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "required_paths": ["src/app/page.tsx"],
            "verify_commands": ["npm run build"],
            "profile": "nextjs",
            "goal": "Create a static Next.js page",
            "required_capabilities": [],
            "required_evidence": ["implementation_artifact"],
            "verify_repair_cap": 2
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

fn static_compile_repair_plan() -> UltraPlan {
    UltraPlan {
        goal: "Create a static Next.js page".to_string(),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "phase-one".to_string(),
                prompt: "Keep the current compiling static page.".to_string(),
            },
            UltraPhase {
                id: "phase-two".to_string(),
                prompt: "Update the page copy for the final app.".to_string(),
            },
        ],
    }
}

fn static_phase_step_plan_json(verify_build: bool) -> String {
    serde_json::to_string(&StepPlan {
        goal: "Create a static Next.js page".to_string(),
        steps: vec![PlanStep {
            id: if verify_build {
                "verify-static-build".to_string()
            } else {
                "update-static-page".to_string()
            },
            kind: if verify_build {
                "verify".to_string()
            } else {
                "setup".to_string()
            },
            expected_result: "static page is present".to_string(),
            instruction: if verify_build {
                "Verify the current static Next.js page build".to_string()
            } else {
                "Update src/app/page.tsx for the static app".to_string()
            },
            expected_paths: if verify_build {
                Vec::new()
            } else {
                vec!["src/app/page.tsx".to_string()]
            },
            verify: if verify_build {
                vec!["npm run build".to_string()]
            } else {
                Vec::new()
            },
        }],
    })
    .unwrap()
}

#[cfg(unix)]
fn static_breaking_build_step_plan() -> StepPlan {
    StepPlan {
        goal: "Create a static Next.js page".to_string(),
        steps: vec![PlanStep {
            id: "break-then-verify-build".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Update src/app/page.tsx and verify the build.".to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: vec!["npm run build".to_string()],
        }],
    }
}

#[cfg(unix)]
fn static_breaking_build_step_plan_json() -> String {
    serde_json::to_string(&static_breaking_build_step_plan()).unwrap()
}

fn write_static_page_reply(content: &str) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/page.tsx","content":content}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn read_static_page_reply() -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Read",
            serde_json::json!({"path":"src/app/page.tsx"}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

#[cfg(unix)]
fn bash_true_reply() -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Bash",
            serde_json::json!({"command":"true"}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

