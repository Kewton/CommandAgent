fn generated_step_plan_json(goal: &str) -> String {
    serde_json::to_string(&StepPlan::single(goal)).unwrap()
}

fn generated_ultra_plan_yaml(goal: &str) -> String {
    render_ultra_plan(&UltraPlan {
        goal: goal.to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            crate::planner::ultra_plan::UltraPhase {
                id: "scaffold".to_string(),
                prompt: format!("Create the required project artifacts for {goal}."),
            },
            crate::planner::ultra_plan::UltraPhase {
                id: "verify".to_string(),
                prompt: format!(
                    "Run deterministic verification for {goal} and repair failures."
                ),
            },
        ],
    })
}

fn two_phase_ultra_plan(goal: &str, profile: &str) -> UltraPlan {
    UltraPlan {
        goal: goal.to_string(),
        profile: profile.to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "scaffold".to_string(),
                prompt: "Create the initial scaffold.".to_string(),
            },
            UltraPhase {
                id: "finish".to_string(),
                prompt: "Complete the final behavior and verification evidence.".to_string(),
            },
        ],
    }
}

fn single_write_step_plan_json(goal: &str, path: &str) -> String {
    serde_json::to_string(&StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "write-artifact".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!("Create {path}."),
            expected_paths: vec![path.to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn browser_release_evidence_tool_calls() -> Vec<crate::state::ToolCall> {
    vec![
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({
                "path": "browser-readiness.json",
                "content": r#"{"ok":true,"http_status":200,"route_rendered":true}"#
            }),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({
                "path": "browser-interaction.json",
                "content": contract_interaction_pass_json()
            }),
        ),
    ]
}

fn generic_interactive_source() -> &'static str {
    r#"import { useState } from "react";
export default function Memo(){
  const [items, setItems] = useState([]);
  return <form onSubmit={(event) => { event.preventDefault(); setItems([...items, "note"]); }}>
<input onChange={() => setItems([...items, "draft"])} />
<button type="submit">Add</button>
<ul>{items.map((item, index) => <li key={index}>{item}</li>)}</ul>
  </form>;
}
"#
}

fn challenge_ultra_plan() -> UltraPlan {
    UltraPlan {
        goal: "Create a browser challenge screen".to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            crate::planner::ultra_plan::UltraPhase {
                id: "phase-one".to_string(),
                prompt: "Create the first page artifact.".to_string(),
            },
            crate::planner::ultra_plan::UltraPhase {
                id: "phase-two".to_string(),
                prompt: "Close remaining final requirements.".to_string(),
            },
        ],
    }
}

fn challenge_implement_step_plan_json() -> String {
    serde_json::to_string(&StepPlan {
        goal: "Create src/app/page.tsx".to_string(),
        steps: vec![PlanStep {
            id: "page".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create or update src/app/page.tsx".to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn challenge_setup_step_plan_json() -> String {
    serde_json::to_string(&StepPlan {
        goal: "Record phase two setup completion".to_string(),
        steps: vec![PlanStep {
            id: "phase-two-marker".to_string(),
            kind: "setup".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create phase-two.txt".to_string(),
            expected_paths: vec!["phase-two.txt".to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn final_marker_implement_step_plan_json() -> String {
    serde_json::to_string(&StepPlan {
        goal: "Run the final implementation pass".to_string(),
        steps: vec![PlanStep {
            id: "final-page-pass".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Update src/app/page.tsx as the final implementation artifact."
                .to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn write_challenge_contract(root: &Path) -> PathBuf {
    write_challenge_contract_with_cap(root, 1)
}

fn write_challenge_contract_with_cap(root: &Path, verify_repair_cap: usize) -> PathBuf {
    let path = root.join("challenge-contract.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "required_paths": ["src/app/page.tsx"],
            "required_evidence": ["challenge_or_adversary_evidence"],
            "verify_repair_cap": verify_repair_cap
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

fn latest_event(path: &Path, event: &str) -> Value {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .rfind(|value| value.get("event").and_then(Value::as_str) == Some(event))
        .unwrap_or_else(|| panic!("missing event {event} in {}", path.display()))
}

fn events_with_name(path: &Path, event: &str) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("event").and_then(Value::as_str) == Some(event))
        .collect()
}

fn event_array_contains(value: &Value, key: &str, needle: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|item| item.as_str() == Some(needle))
}

fn planner_request_text(client: &FakeClient, index: usize) -> String {
    client.messages()[index]
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn prompt_section_lines(prompt: &str, header: &str) -> Vec<String> {
    let mut lines = prompt.lines().skip_while(|line| *line != header);
    let Some(first) = lines.next() else {
        panic!("missing prompt section {header:?} in {prompt}");
    };
    std::iter::once(first.to_string())
        .chain(
            lines
                .take_while(|line| !line.trim().is_empty())
                .map(str::to_string),
        )
        .collect()
}

fn nextjs_scaffold_expected_paths() -> Vec<String> {
    vec![
        "package.json".to_string(),
        "tsconfig.json".to_string(),
        "postcss.config.js".to_string(),
        "tailwind.config.ts".to_string(),
        "src/app/layout.tsx".to_string(),
        "src/app/page.tsx".to_string(),
        "src/app/globals.css".to_string(),
        "src/app/global.d.ts".to_string(),
    ]
}

fn nextjs_complete_package_json() -> &'static str {
    r#"{"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"},"devDependencies":{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"},"scripts":{"build":"next build","dev":"next dev -p 3011","start":"next start -p 3011"}}"#
}

fn canvas_game_repair_guidance() -> &'static str {
    &crate::planner::profiles::nextjs::knowledge::get()
        .repair_guidance
        .canvas_game_interaction
}

fn nextjs_lean_package_json() -> &'static str {
    r#"{"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"},"scripts":{"build":"next build","dev":"next dev -p 3011","start":"next start -p 3011"}}"#
}

fn explicit_port_goal(goal: &str, port: u16) -> String {
    format!("{goal} on port {port}")
}

fn nextjs_tsconfig_json() -> &'static str {
    r#"{"compilerOptions":{"target":"ES2017","lib":["dom","dom.iterable","esnext"],"allowJs":true,"skipLibCheck":true,"strict":true,"noEmit":true,"esModuleInterop":true,"module":"esnext","moduleResolution":"bundler","resolveJsonModule":true,"isolatedModules":true,"jsx":"preserve","incremental":true,"plugins":[{"name":"next"}],"baseUrl":".","paths":{"@/*":["./src/*"]}},"include":["next-env.d.ts","**/*.ts","**/*.tsx",".next/types/**/*.ts"],"exclude":["node_modules"]}"#
}

fn nextjs_layout_source() -> &'static str {
    "import './globals.css';\nexport default function Layout({children}:{children:React.ReactNode}){return <html><body>{children}</body></html>;}"
}

fn nextjs_globals_css() -> &'static str {
    "@tailwind base;\n@tailwind components;\n@tailwind utilities;\n"
}

fn nextjs_tailwind_config_ts() -> &'static str {
    "import type { Config } from 'tailwindcss';\nconst config: Config = { content: ['./src/pages/**/*.{js,ts,jsx,tsx,mdx}', './src/components/**/*.{js,ts,jsx,tsx,mdx}', './src/app/**/*.{js,ts,jsx,tsx,mdx}'], theme: { extend: {} }, plugins: [] };\nexport default config;\n"
}

fn nextjs_postcss_config() -> &'static str {
    "module.exports = { plugins: { tailwindcss: {}, autoprefixer: {} } };\n"
}

