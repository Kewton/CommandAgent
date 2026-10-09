//! Promoted-profile port test over the Issue #608 cfg(test) dev-server
//! transport (Issue #633).
//!
//! The port-using profile-promotion body moved here from
//! `tests/tui_integration.rs` so it runs in the required `cargo test` library
//! suite (the integration test links the normally-built library and cannot call
//! the `#[cfg(test)]` transport). The logical port stays in the goal, the
//! contract, and the evidence; the fake dev child binds an OS-assigned port and
//! announces it through the transport ready file, so no number is freed and
//! re-bound and two checkouts can run the same logical port concurrently.
//!
//! The moved body runs in its own process: the required `#[test]` re-execs this
//! binary for the `#[ignore]`d inner body, so the TUI flow never shares the lib
//! test process (and its `cli_pack` / `presentation` global state) with the other
//! `tui::` tests. The inner is gated by an entry environment variable, exactly
//! like the fake dev child.
//!
//! `#[cfg(unix)]`-only: the child fixture is a POSIX shell package manager.

#[cfg(test)]
mod tests {
    use crate::config::{
        Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
        PromptLayout, Provider,
    };
    use crate::planner::step_plan::StepPlan;
    use crate::planner::ultra_plan::{UltraPhase, UltraPlan};
    use crate::providers::{AssistantReply, ChatClient};
    use crate::state::{ConversationMessage, ToolCall};
    use crate::tools::registry::ToolSpec;
    use crate::tui::status::UiStatus;
    use crate::tui::{InteractionUi, UiGuard};
    use serde_json::json;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard};

    include!("../../tests/support/tui_profile_fixture.rs");

    #[cfg(unix)]
    fn nextjs_package_json(port: u16) -> String {
        format!(
            r#"{{"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}},"devDependencies":{{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"}},"scripts":{{"build":"next build","dev":"next dev -p {port}","start":"next start -p {port}"}}}}"#
        )
    }

    #[cfg(unix)]
    fn nextjs_page_source() -> String {
        r#""use client";
import { useState } from "react";

export default function Page() {
  const [draft, setDraft] = useState("");
  const [items, setItems] = useState<string[]>([]);
  return <main data-anvil-state={items.length}>
    <input aria-label="Memo" value={draft} onChange={(event) => setDraft(event.target.value)} />
    <button data-anvil-action="primary" onClick={() => setItems([...items, draft])}>Add</button>
    <ul>{items.map((item, index) => <li key={index}>{item}</li>)}</ul>
  </main>;
}
"#
        .to_string()
    }

    #[cfg(unix)]
    fn sh_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }

    /// The fake package manager for this library test. `npm run dev`/`start` exec
    /// this same test binary as `profile_transport_dev_server_child`, whose only job
    /// is to bind an OS-assigned port and announce it through the transport's ready
    /// files; `npm run build` is a no-op success so the promoted Next.js acceptance
    /// path stays exercised without a real toolchain. This keeps the frozen
    /// observer/child bytes out of the product path, mirroring the Issue #622/#448
    /// test transports.
    #[cfg(unix)]
    fn write_fake_dev_server_package_manager(root: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let bin = root.join("node_modules/.bin");
        std::fs::create_dir_all(&bin).unwrap();
        for package in ["next", "tailwindcss", "postcss", "autoprefixer"] {
            std::fs::create_dir_all(root.join("node_modules").join(package)).unwrap();
        }
        let exe = sh_quote(&std::env::current_exe().unwrap().display().to_string());
        let child = "--ignored --exact tui::profile_transport_tests::tests::profile_transport_dev_server_child --nocapture";
        let npm = format!(
            "#!/bin/sh\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then\n\
  echo \"fake build ok\"\n\
  exit 0\n\
fi\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"dev\" ]; then\n\
  COMMANDAGENT_TUI_PROFILE_TRANSPORT_CHILD=1 exec {exe} {child}\n\
fi\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"start\" ]; then\n\
  COMMANDAGENT_TUI_PROFILE_TRANSPORT_CHILD=1 exec {exe} {child}\n\
fi\n\
echo \"unexpected fake npm args: $*\" >&2\n\
exit 2\n"
        );
        let npm_path = bin.join("npm");
        std::fs::write(&npm_path, npm).unwrap();
        let mut permissions = std::fs::metadata(&npm_path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&npm_path, permissions).unwrap();

        let next_path = bin.join("next");
        std::fs::write(&next_path, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = std::fs::metadata(&next_path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&next_path, permissions).unwrap();
    }

    /// The test-owned fake dev child. It works only through the same file contract
    /// the product's `cfg(test)` transport reads — the workspace marker, the
    /// generation file, the `{"generation","port"}` ready file, and the browser
    /// probe's mock-port file — so no product code or visibility changes are
    /// needed. It binds an OS-assigned port and announces the real one, so the
    /// logical goal port is never freed and re-bound.
    #[test]
    #[ignore]
    #[cfg(unix)]
    #[allow(clippy::zombie_processes)]
    fn profile_transport_dev_server_child() {
        if std::env::var("COMMANDAGENT_TUI_PROFILE_TRANSPORT_CHILD")
            .ok()
            .as_deref()
            != Some("1")
        {
            return;
        }
        let root = Path::new(".");
        let logical: u16 = std::env::var("PORT").unwrap().parse().unwrap();
        let dynamic = root
            .join(".anvil/evidence/dev-server-test-transport")
            .is_file();
        let listener =
            std::net::TcpListener::bind(("127.0.0.1", if dynamic { 0 } else { logical })).unwrap();
        if dynamic {
            let actual = listener.local_addr().unwrap().port();
            let generation: u64 =
                std::fs::read_to_string(root.join(".anvil/evidence/dev-server-probe-generation"))
                    .ok()
                    .and_then(|value| value.trim().parse().ok())
                    .unwrap_or(0);
            let evidence = root.join(".anvil/evidence");
            std::fs::create_dir_all(&evidence).unwrap();
            std::fs::write(
                evidence.join("dev-server-ready-port.json"),
                json!({"generation": generation, "port": actual}).to_string(),
            )
            .unwrap();
            std::fs::write(
                evidence.join("browser-probe-mock-port.txt"),
                actual.to_string(),
            )
            .unwrap();
        }
        let body = "<html><body><main data-anvil-state='{}'><input id=\"memo\" aria-label=\"Memo\" /><button id=\"add\" data-anvil-action=\"primary\">Add</button><ul id=\"items\"></ul></main></body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut request = [0_u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut request);
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        }
    }

    /// Turn on the browser and dev-server probes and the Issue #608 transport for
    /// this workspace. The two `enable-*-probe-tests` files are the `cfg(test)`
    /// contracts read by `ultra_browser_probe_runtime_enabled` and
    /// `dev_server_probe_runtime_enabled`. The `browser-probe-command.json` override
    /// (mirroring `test_transport::enable_browser_probe_transport`, which is
    /// private to the runner test module) makes the browser readiness probe opt into
    /// the transport: the fake child binds an OS-assigned port and publishes it, and
    /// the probe polls that port instead of the logical `port` in the goal. The
    /// `dev-server-test-transport` marker is the one
    /// `src/planner/runner/acceptance/test_transport.rs` reads for the dev route
    /// probe; the child writes both ready files.
    #[cfg(unix)]
    fn enable_profile_transport(root: &Path, port: u16) {
        let anvil = root.join(".anvil");
        std::fs::create_dir_all(anvil.join("evidence")).unwrap();
        std::fs::write(anvil.join("enable-dev-server-probe-tests"), "1").unwrap();
        std::fs::write(anvil.join("enable-browser-probe-tests"), "1").unwrap();
        std::fs::write(anvil.join("evidence/dev-server-test-transport"), "1").unwrap();
        std::fs::write(
            anvil.join("evidence/browser-probe-command.json"),
            json!({
                "program": "npm",
                "args": ["run", "start"],
                "port": port,
                "require_build": true,
                "dynamic_port": true,
                "display": "fake dev server (browser transport)",
            })
            .to_string(),
        )
        .unwrap();
    }

    /// The browser interaction observation the probe returns under `cfg(test)`,
    /// matching what the integration test's fake node script produced.
    #[cfg(unix)]
    fn interaction_probe_passed_value() -> serde_json::Value {
        serde_json::from_str(
        r#"{"ok":true,"status":"passed","interaction_success":true,"interaction_performed":true,"surface_visible":true,"start_control_found":true,"start_transition":true,"input_state_change":true,"input_state_evaluated_after_start":true,"input_event_observed":true,"state_changed":true,"probe_mode":"contract","contract_hook_status":"usable","action_hooks":["primary"],"state_dimensions_changed":["items","draft"],"primary_start_transition":true,"text_entry":"entered","text_entry_target":"input#memo","typed_token":"anvil-probe","token_echoed":true,"echo_latency_ms":1,"text_input_state_change":true,"stage":"observing","steps":["surface_visible","start_transition","control_input_dispatched","input_state_evaluated_after_start","input_state_change","text_input_state_change"],"before_marker":"items=0,draft=","after_marker":"items=1,draft=anvil-probe","server_http_status":200,"duration_ms":1}"#,
    )
    .unwrap()
    }

    /// Required `#[test]`: re-execs this binary so the TUI flow body runs in its
    /// own process and cannot race the other `tui::` tests' global state.
    #[test]
    #[cfg(unix)]
    fn tui_slash_promoted_profile_reflected_in_terminal_summary() {
        use std::io::Read;
        use std::time::{Duration, Instant};

        const INNER_TEST_NAME: &str =
            "tui::profile_transport_tests::tests::profile_transport_promoted_profile_run";

        let exe = std::env::current_exe().unwrap();
        let marker = exe.parent().unwrap().join(format!(
            "issue633-profile-inner-{}-{}.marker",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&marker);

        let mut child = std::process::Command::new(&exe)
            .args([
                "--ignored",
                "--exact",
                INNER_TEST_NAME,
                "--nocapture",
                "--test-threads=1",
            ])
            .env("COMMANDAGENT_TUI_PROFILE_TRANSPORT_INNER", "1")
            .env("COMMANDAGENT_TUI_PROFILE_TRANSPORT_MARKER", &marker)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn the inner profile transport test");

        // Drain both pipes on their own threads so a full pipe cannot deadlock.
        let mut stdout_pipe = child.stdout.take().unwrap();
        let stdout_reader = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stdout_pipe.read_to_string(&mut text);
            text
        });
        let mut stderr_pipe = child.stderr.take().unwrap();
        let stderr_reader = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr_pipe.read_to_string(&mut text);
            text
        });

        let deadline = Instant::now() + Duration::from_secs(120);
        let status = loop {
            match child.try_wait().expect("wait for the inner test") {
                Some(status) => break Some(status),
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                None => std::thread::sleep(Duration::from_millis(50)),
            }
        };
        let stdout = stdout_reader.join().unwrap();
        let stderr = stderr_reader.join().unwrap();

        let report = format!("--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}");
        let Some(status) = status else {
            panic!("inner profile transport test exceeded 120s\n{report}");
        };
        assert!(
            status.success(),
            "inner profile transport test failed ({status:?})\n{report}"
        );
        assert!(
            stdout.contains("1 passed"),
            "inner test did not run exactly one passing test\n{report}"
        );
        assert!(
            marker.is_file(),
            "inner test did not reach its final assertion\n{report}"
        );
        let _ = std::fs::remove_file(&marker);
    }

    /// The moved body. `#[ignore]`d so `cargo test` never runs it in-process; the
    /// outer test re-execs the binary for it, and it does nothing unless the outer
    /// set the entry variable (the same shape as the fake dev child).
    #[test]
    #[ignore]
    #[cfg(unix)]
    fn profile_transport_promoted_profile_run() {
        if std::env::var("COMMANDAGENT_TUI_PROFILE_TRANSPORT_INNER")
            .ok()
            .as_deref()
            != Some("1")
        {
            return;
        }
        // Target-anchored workspace: never under a read-restricted system prefix.
        let exe = std::env::current_exe().unwrap();
        let dir = tempfile::Builder::new()
            .prefix("issue633-profile-")
            .tempdir_in(exe.parent().unwrap())
            .unwrap();
        // Hold the logical port for the whole test. The transport binds an
        // OS-assigned port, so the goal/contract/evidence keep this number and
        // two concurrent checkouts never collide on a fixed port; the listener is
        // never released, proving occupancy while the run still succeeds.
        // 3011 is the product's NEXTJS_DEV_SERVER_DEFAULT_PORT, which the probe
        // treats as "no explicit port", so avoid returning it.
        let logical_listener = loop {
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            if listener.local_addr().unwrap().port() != 3011 {
                break listener;
            }
        };
        let port = logical_listener.local_addr().unwrap().port();
        let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
        let plan_path = dir.path().join("ultra.yaml");
        let goal = format!(
            "ちょっとしたメモアプリを作って。ブラウザで使えるようにしてください。{port}ポートで起動可能にしてください。"
        );
        let plan = UltraPlan {
            goal,
            profile: "generic".to_string(),
            style: "default".to_string(),
            intent: "create".to_string(),
            phases: vec![
                UltraPhase {
                    id: "setup-framework".to_string(),
                    prompt: "Create the package manifest".to_string(),
                },
                UltraPhase {
                    id: "implement-ui".to_string(),
                    prompt: "Create the promoted Next.js route".to_string(),
                },
            ],
        };
        std::fs::write(
            &plan_path,
            crate::planner::ultra_plan::render_ultra_plan(&plan),
        )
        .unwrap();
        let mut setup_step = StepPlan::single("create package");
        setup_step.steps[0].kind = "setup".to_string();
        setup_step.steps[0].expected_paths = vec!["package.json".to_string()];
        let mut route_step = StepPlan::single("create route");
        route_step.steps[0].kind = "implement".to_string();
        route_step.steps[0].expected_paths = vec![
            "tsconfig.json".to_string(),
            "postcss.config.js".to_string(),
            "tailwind.config.ts".to_string(),
            "src/app/layout.tsx".to_string(),
            "src/app/page.tsx".to_string(),
            "src/app/globals.css".to_string(),
            "src/app/global.d.ts".to_string(),
        ];
        write_fake_dev_server_package_manager(dir.path());
        enable_profile_transport(dir.path(), port);
        crate::minimal_loop::interaction_probe::write_test_availability_override(dir.path(), true);
        crate::minimal_loop::interaction_probe::write_test_result_override(
            dir.path(),
            &interaction_probe_passed_value(),
        );
        let mut cfg = config(dir.path().to_path_buf());
        cfg.eval_events_path = Some(events_path.clone());
        let mut planner = FakeClient::new(
            "planner",
            vec![
                AssistantReply::text(serde_json::to_string(&setup_step).unwrap()),
                AssistantReply::text(serde_json::to_string(&route_step).unwrap()),
            ],
        );
        let mut execution = FakeClient::new(
            "exec",
            vec![
                AssistantReply {
                    content: String::new(),
                    tool_calls: vec![ToolCall::new(
                        "Write",
                        json!({"path":"package.json","content":nextjs_package_json(port)}),
                    )],
                    prompt_tokens: None,
                    completion_tokens: None,
                },
                AssistantReply {
                    content: String::new(),
                    tool_calls: vec![
                        ToolCall::new(
                            "Write",
                            json!({"path":"src/app/layout.tsx","content":"export default function RootLayout({ children }: { children: React.ReactNode }) { return <html><body>{children}</body></html>; }"}),
                        ),
                        ToolCall::new(
                            "Write",
                            json!({"path":"tsconfig.json","content":r#"{"compilerOptions":{"target":"ES2017","lib":["dom","dom.iterable","esnext"],"allowJs":true,"skipLibCheck":true,"strict":true,"noEmit":true,"esModuleInterop":true,"module":"esnext","moduleResolution":"bundler","resolveJsonModule":true,"isolatedModules":true,"jsx":"preserve","incremental":true,"plugins":[{"name":"next"}]},"include":["next-env.d.ts","**/*.ts","**/*.tsx",".next/types/**/*.ts"],"exclude":["node_modules"]}"#}),
                        ),
                        ToolCall::new(
                            "Write",
                            json!({"path":"postcss.config.js","content":"module.exports = { plugins: { tailwindcss: {}, autoprefixer: {} } };"}),
                        ),
                        ToolCall::new(
                            "Write",
                            json!({"path":"tailwind.config.ts","content":"import type { Config } from 'tailwindcss';\nconst config: Config = { content: ['./src/app/**/*.{js,ts,jsx,tsx,mdx}'], theme: { extend: {} }, plugins: [] };\nexport default config;\n"}),
                        ),
                        ToolCall::new(
                            "Write",
                            json!({"path":"src/app/page.tsx","content":nextjs_page_source()}),
                        ),
                        ToolCall::new(
                            "Write",
                            json!({"path":"src/app/globals.css","content":"body { font-family: sans-serif; }"}),
                        ),
                        ToolCall::new(
                            "Write",
                            json!({"path":"src/app/global.d.ts","content":"declare module '*.css';"}),
                        ),
                        ToolCall::new(
                            "Write",
                            json!({"path":"browser-readiness.json","content":r#"{"ok":true,"http_status":200,"route_rendered":true}"#}),
                        ),
                        ToolCall::new(
                            "Write",
                            json!({"path":"browser-interaction.json","content":r#"{"ok":true,"status":"passed","interaction_success":true,"interaction_performed":true,"surface_visible":true,"start_transition":true,"input_state_change":true,"input_state_evaluated_after_start":true,"input_event_observed":true,"state_changed":true,"canvas_found":true}"#}),
                        ),
                    ],
                    prompt_tokens: None,
                    completion_tokens: None,
                },
            ],
        );
        let ui = FakeUi::default();

        let output = crate::tui::slash::handle_command(
            "/run-ultra-plan ultra.yaml",
            &cfg,
            &mut planner,
            &mut execution,
            &ui,
        )
        .unwrap();

        assert!(output.contains("ultra-plan-run complete"));
        let events = std::fs::read_to_string(&events_path).unwrap();
        assert!(
            events.contains(r#""event":"profile_reinferred""#),
            "{events}"
        );
        assert!(events.contains(r#""profile":"nextjs""#), "{events}");
        let requested_port = format!("{port} (goal)");
        assert!(
            events.contains(&format!(r#""requested_port":"{requested_port}""#)),
            "{events}"
        );
        let stops = tui_command_stop_events(&events);
        assert_eq!(
            stops[0].get("profile").and_then(|value| value.as_str()),
            Some("nextjs"),
            "{events}"
        );
        assert_eq!(
            stops[0]
                .get("effective_profile")
                .and_then(|value| value.as_str()),
            Some("nextjs"),
            "{events}"
        );
        assert_eq!(
            stops[0]
                .get("contract_origin")
                .and_then(|value| value.as_str()),
            Some("promoted_union"),
            "{events}"
        );
        assert_eq!(
            stops[0]
                .get("assurance_level")
                .and_then(|value| value.as_str()),
            Some("full"),
            "{events}"
        );
        assert_eq!(
            stops[0]
                .get("requested_port")
                .and_then(|value| value.as_str()),
            Some(requested_port.as_str()),
            "{events}"
        );
        let summary =
            std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).unwrap();
        assert!(
            summary.contains("Profile promoted: generic -> nextjs"),
            "{summary}"
        );
        assert!(summary.contains("Profile: nextjs"), "{summary}");
        assert!(
            summary.contains(&format!("Requested port: {requested_port}")),
            "{summary}"
        );

        // Evidence that the readiness probe actually ran the fake child and
        // observed HTTP on its OS-assigned port, rather than passing because the
        // cfg(test) probe was disabled or an outcome was scripted. `status`,
        // `http_status`, and the captured child output are the product's own
        // observation of the spawned child.
        let probe = events
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| {
                event.get("event").and_then(|value| value.as_str()) == Some("browser_probe")
            })
            .unwrap_or_else(|| panic!("missing browser_probe event:\n{events}"));
        assert_eq!(
            probe.get("status").and_then(|value| value.as_str()),
            Some("ready"),
            "{probe}"
        );
        assert_eq!(
            probe.get("ok").and_then(|value| value.as_bool()),
            Some(true),
            "{probe}"
        );
        assert_eq!(
            probe.get("http_status").and_then(|value| value.as_i64()),
            Some(200),
            "{probe}"
        );
        assert_eq!(
            probe.get("port").and_then(|value| value.as_u64()),
            Some(u64::from(port)),
            "{probe}"
        );
        assert_eq!(
            probe.get("requested_port").and_then(|value| value.as_str()),
            Some(requested_port.as_str()),
            "{probe}"
        );
        assert!(
            probe
                .get("output_excerpt")
                .and_then(|value| value.as_str())
                .is_some_and(|excerpt| !excerpt.is_empty()),
            "the probe must capture the spawned child's output: {probe}"
        );

        // The child must have bound its own OS-assigned port, not the logical
        // port this test still occupies; the announced port comes from the
        // transport's ready file, which the child wrote. `logical_listener` stays
        // alive to the end of the test, so success here proves the run did not
        // need the logical port to be free.
        let announced: u16 = std::fs::read_to_string(
            dir.path()
                .join(".anvil/evidence/browser-probe-mock-port.txt"),
        )
        .expect("child must announce its OS-assigned port")
        .trim()
        .parse()
        .expect("announced port is a u16");
        assert_ne!(announced, 0, "child must bind a real port");
        assert_ne!(
            announced, port,
            "child must not bind the logical port held by the test"
        );

        // Final marker for the outer test, written only after every assertion
        // above passed. Path comes from the outer test, outside the tempdir.
        let marker = std::env::var("COMMANDAGENT_TUI_PROFILE_TRANSPORT_MARKER")
            .expect("the outer test must pass the marker path");
        std::fs::write(marker, "inner-complete").unwrap();
    }
}
