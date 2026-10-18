//! Issue #511 (audit FIN-028): pin the recovery/diagnostic classification of the
//! final acceptance path. Each test names the cargo-mutants mutations
//! (`final_acceptance.rs` lines at develop `c6a31e17`) it is meant to kill.
//! Mounted from `runner/acceptance/issue507_tests.rs` so `final_acceptance.rs`
//! does not grow.
#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use crate::config::PromptLayout;
    use crate::planner::runner::final_acceptance::*;
    use crate::planner::runner::phase::{UltraRunContext, UltraRunSetupAuthorityState};
    use crate::planner::runner::{CompileError, Config, VerificationReport};
    use crate::planner::ultra_plan::UltraPlan;
    use crate::providers::{AssistantReply, ChatClient};
    use crate::state::{ConversationMessage, ToolCall};
    use crate::tools::registry::ToolSpec;
    use clap::Parser;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    // ---------------------------------------------------------------------
    // Pure classification helpers
    // ---------------------------------------------------------------------

    fn failures(items: &[&str]) -> VerificationReport {
        let mut report = VerificationReport::pass();
        for item in items {
            report.push_profile_failure(*item);
        }
        report
    }

    #[test]
    fn model_fixable_profile_failures_drops_each_infrastructure_marker() {
        // kills: final_acceptance.rs:2174 -> vec![] / vec![""] / vec!["xyzzy"]
        // kills: final_acceptance.rs:2180..2187 && -> || (every marker excludes alone)
        // kills: final_acceptance.rs:2179..2187 delete ! (an unrelated failure stays)
        for marker in [
            "browser_interaction_evidence_required",
            "interaction_evidence_missing",
            "interaction evidence status: missing",
            "interaction evidence path: /tmp/x.json",
            "interaction_unverified:probe_unavailable",
            "unverified: ui",
            "probe_dependency_missing",
            "probe_infrastructure_failed",
            "app interaction untested",
            "PROBE_INFRASTRUCTURE_FAILED",
        ] {
            let report = failures(&[marker, "weak_verification_evidence:x"]);
            assert_eq!(
                final_acceptance_model_fixable_profile_failures(&report),
                vec!["weak_verification_evidence:x".to_string()],
                "{marker}"
            );
        }
        let report = failures(&["first", "second"]);
        assert_eq!(
            final_acceptance_model_fixable_profile_failures(&report),
            vec!["first".to_string(), "second".to_string()]
        );
        assert!(
            final_acceptance_model_fixable_profile_failures(&VerificationReport::pass()).is_empty()
        );
    }

    #[test]
    fn repairs_contract_evidence_table() {
        // kills: final_acceptance.rs:2160 -> true / false
        // kills: final_acceptance.rs:2160 == -> != (target comparison)
        // kills: final_acceptance.rs:2161 || -> && (target alone is enough)
        // kills: final_acceptance.rs:2166,2167 || -> && (each marker alone)
        let rows: &[(&[&str], &str, bool)] = &[
            (&[], "required_evidence_missing", true),
            (&[], "implementation", false),
            (
                &["missing_required_evidence:test_artifact"],
                "implementation",
                true,
            ),
            (&["weak_verification_evidence:x"], "implementation", true),
            (
                &["Required Evidence Missing for ui"],
                "implementation",
                true,
            ),
            (&["something else"], "implementation", false),
            (
                &["unverified: missing_required_evidence:x"],
                "implementation",
                false,
            ),
            (
                &["weak_verification_evidence"],
                "required_evidence_missing",
                true,
            ),
        ];
        for (profile_failures, target, expected) in rows {
            assert_eq!(
                final_acceptance_repairs_contract_evidence(&failures(profile_failures), target),
                *expected,
                "{profile_failures:?} / {target}"
            );
        }
    }

    fn write_probe_json(dir: &Path, name: &str, value: serde_json::Value) -> String {
        let path = dir.join(name);
        std::fs::write(&path, value.to_string()).unwrap();
        path.to_string_lossy().to_string()
    }

    fn probe_report(path: &str) -> VerificationReport {
        failures(&[&format!("interaction evidence path: {path}")])
    }

    #[test]
    fn recovery_failure_evidence_reports_infrastructure_probe_details() {
        // kills: final_acceptance.rs:1791 -> vec![] / vec![""] / vec!["xyzzy"]
        // kills: final_acceptance.rs:1797 && -> || (probe present, other category)
        // kills: final_acceptance.rs:1800 == -> != (category comparison)
        // kills: final_acceptance.rs:1806 delete ! (non-empty stage is reported)
        // kills: final_acceptance.rs:1811 delete ! (non-empty error is reported)
        let dir = tempfile::tempdir().unwrap();
        let path = write_probe_json(
            dir.path(),
            "infra.json",
            json!({
                "failure_category": "infrastructure",
                "failure_kind": "probe_dependency_missing",
                "stage": "launch",
                "error": "chromium missing",
            }),
        );
        let evidence = final_acceptance_recovery_failure_evidence_base(
            None,
            "generic",
            "goal",
            &probe_report(&path),
            "",
        );
        let lines: Vec<&String> = evidence
            .iter()
            .filter(|line| line.starts_with("behavioral probe"))
            .collect();
        assert_eq!(
            lines,
            vec![
                "behavioral probe infrastructure reason: probe_dependency_missing",
                "behavioral probe failed stage: launch",
                "behavioral probe error: chromium missing",
            ]
        );
    }

    #[test]
    fn recovery_failure_evidence_skips_empty_stage_error_and_defaults_kind() {
        // kills: final_acceptance.rs:1806 delete ! (empty stage is not reported)
        // kills: final_acceptance.rs:1811 delete ! (empty error is not reported)
        let dir = tempfile::tempdir().unwrap();
        let path = write_probe_json(
            dir.path(),
            "infra-empty.json",
            json!({"failure_category": "infrastructure", "stage": "", "error": ""}),
        );
        let evidence = final_acceptance_recovery_failure_evidence_base(
            None,
            "generic",
            "goal",
            &probe_report(&path),
            "",
        );
        let lines: Vec<&String> = evidence
            .iter()
            .filter(|line| line.starts_with("behavioral probe"))
            .collect();
        assert_eq!(
            lines,
            vec!["behavioral probe infrastructure reason: probe_infrastructure_failed"]
        );
    }

    #[test]
    fn recovery_failure_evidence_ignores_non_infrastructure_probe() {
        // kills: final_acceptance.rs:1797 && -> ||
        // kills: final_acceptance.rs:1800 == -> !=
        let dir = tempfile::tempdir().unwrap();
        let path = write_probe_json(
            dir.path(),
            "product.json",
            json!({"failure_category": "product", "stage": "click", "error": "boom"}),
        );
        let evidence = final_acceptance_recovery_failure_evidence_base(
            None,
            "generic",
            "goal",
            &probe_report(&path),
            "",
        );
        assert!(
            !evidence.iter().any(|l| l.starts_with("behavioral probe")),
            "{evidence:?}"
        );
        // A report without any readable probe never reports probe details.
        let missing = final_acceptance_recovery_failure_evidence_base(
            None,
            "generic",
            "goal",
            &probe_report("/nonexistent/probe.json"),
            "",
        );
        assert!(!missing.iter().any(|l| l.starts_with("behavioral probe")));
    }

    #[test]
    fn recovery_failure_evidence_appends_reason_and_compile_guidance() {
        // kills: final_acceptance.rs:1791 -> vec![] / vec![""] / vec!["xyzzy"]
        // kills: final_acceptance.rs:1827 delete ! (blank reason adds nothing)
        let blank = final_acceptance_recovery_failure_evidence_base(
            None,
            "generic",
            "goal",
            &VerificationReport::pass(),
            "  ",
        );
        assert!(blank.is_empty(), "{blank:?}");

        let with_reason = final_acceptance_recovery_failure_evidence_base(
            None,
            "generic",
            "goal",
            &VerificationReport::pass(),
            "bounded_repair_exhausted",
        );
        assert_eq!(with_reason, vec!["bounded_repair_exhausted".to_string()]);

        let mut report = VerificationReport::pass();
        report.push_compile_errors(
            "npm run build",
            vec![CompileError {
                path: "src/app/page.tsx".to_string(),
                line: 3,
                column: 5,
                message: "Cannot find name 'foo'.".to_string(),
                excerpt: String::new(),
                symbol: None,
                route_bound: None,
            }],
        );
        let evidence = final_acceptance_recovery_failure_evidence_base(
            None, "generic", "goal", &report, "stopped",
        );
        assert_eq!(
            evidence.first().map(String::as_str),
            Some("fix_compile_error: Compile error: src/app/page.tsx:3:5 Cannot find name 'foo'.")
        );
        assert_eq!(evidence.last().map(String::as_str), Some("stopped"));
        assert!(
            evidence
                .iter()
                .filter(|l| l.starts_with("fix_compile_error: "))
                .count()
                >= 3,
            "{evidence:?}"
        );
    }

    #[test]
    fn recovery_failure_evidence_uses_root_only_with_pending_signals() {
        // kills: final_acceptance.rs:1816 delete ! (root is consulted only when keys are pending)
        let root = tempfile::tempdir().unwrap();
        // No pending signals: the root must not add an exhaustion line.
        let none = final_acceptance_recovery_failure_evidence_base(
            Some(root.path()),
            "generic",
            "goal",
            &VerificationReport::pass(),
            "boom",
        );
        assert_eq!(none, vec!["boom".to_string()]);

        // Pending signals: remedy lines, then the exhaustion classification,
        // then the raw reason.
        let report = failures(&["missing_required_evidence:user_input_handler_evidence"]);
        let with_root = final_acceptance_recovery_failure_evidence_base(
            Some(root.path()),
            "generic",
            "goal",
            &report,
            "bounded_repair_exhausted",
        );
        assert_eq!(
            with_root,
            vec![
                "user_input_handler_evidence: wire keyboard, pointer, click, touch, or form handlers to route-bound state changes".to_string(),
                "exhaustion classification: bounded_repair_exhausted".to_string(),
                "bounded_repair_exhausted".to_string(),
            ]
        );
        // Without a root the remedy lines stand alone.
        let without_root = final_acceptance_recovery_failure_evidence_base(
            None,
            "generic",
            "goal",
            &report,
            "bounded_repair_exhausted",
        );
        assert_eq!(
            without_root,
            vec![with_root[0].clone(), "bounded_repair_exhausted".to_string()]
        );
    }

    // ---------------------------------------------------------------------
    // try_final_acceptance_compile_regeneration
    // ---------------------------------------------------------------------

    const BROKEN_PAGE: &str = "export default function Page(){\nBROKEN_SYNTAX\n}\n";
    const GOOD_PAGE: &str = "export default function Page(){return <main>Recovered</main>;}\n";

    #[derive(Clone)]
    struct Scripted {
        replies: Arc<Mutex<VecDeque<Result<AssistantReply, String>>>>,
        requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    }

    impl Scripted {
        fn new(replies: Vec<Result<AssistantReply, String>>) -> Self {
            Self {
                replies: Arc::new(Mutex::new(replies.into())),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn prompt_text(&self) -> String {
            self.requests
                .lock()
                .unwrap()
                .iter()
                .flatten()
                .map(|m| m.content.clone())
                .collect::<Vec<_>>()
                .join("\n=====\n")
        }
    }

    impl ChatClient for Scripted {
        fn label(&self) -> &str {
            "scripted-511"
        }
        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }
        fn supports_native_tools(&self, _model: &str) -> bool {
            true
        }
        fn chat(
            &mut self,
            _model: &str,
            messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            self.requests.lock().unwrap().push(messages.to_vec());
            match self.replies.lock().unwrap().pop_front() {
                Some(Ok(reply)) => Ok(reply),
                Some(Err(message)) => Err(anyhow::anyhow!(message)),
                None => Ok(AssistantReply::text("done")),
            }
        }
    }

    fn write_reply(path: &str, content: &str) -> Result<AssistantReply, String> {
        Ok(AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Write",
                json!({"path": path, "content": content}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        })
    }

    fn config_for(root: &Path, layout: PromptLayout) -> Config {
        let mut config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            root.to_str().unwrap(),
            "--profile",
            "nextjs",
        ]))
        .unwrap();
        config.eval_events_path = Some(root.join("run/events.jsonl"));
        config.prompt_layout = layout;
        config.yes = true;
        config
    }

    fn broken_report(line: usize) -> VerificationReport {
        let mut report = VerificationReport::pass();
        report.push_compile_errors(
            "npm run build",
            vec![CompileError {
                path: "src/app/page.tsx".to_string(),
                line,
                column: 1,
                message: "Expected ';'".to_string(),
                excerpt: format!("{line} | BROKEN_SYNTAX"),
                symbol: None,
                route_bound: Some(true),
            }],
        );
        report
    }

    struct Outcome {
        accepted: bool,
        report: VerificationReport,
        context: UltraRunContext,
    }

    fn run_regeneration(
        client: &mut Scripted,
        config: &Config,
        goal: &str,
        mut report: VerificationReport,
        expected_paths: &[String],
    ) -> Outcome {
        let plan = UltraPlan::deterministic(goal, "nextjs", "default", "create");
        let mut context = UltraRunContext::default();
        let mut remedies = Vec::new();
        let mut setup = UltraRunSetupAuthorityState::default();
        let accepted = try_final_acceptance_compile_regeneration(
            client,
            config,
            &plan,
            &mut context,
            &mut report,
            &mut remedies,
            &mut setup,
            0,
            expected_paths,
            config,
            &crate::tui::NOOP_UI,
        )
        .unwrap();
        Outcome {
            accepted,
            report,
            context,
        }
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    /// The prompt handed to the model is the only place the `RepairContext`
    /// assembled by the function is observable, so each observable field is
    /// pinned through it.
    #[test]
    fn compile_regeneration_context_reaches_the_model_prompt() {
        // kills: final_acceptance.rs:760 delete field profile (Next.js contract advisory)
        // kills: final_acceptance.rs:761 delete field overall_goal
        // kills: final_acceptance.rs:763 delete field expected_paths
        // kills: final_acceptance.rs:765 delete field workspace_root (current file content)
        // kills: final_acceptance.rs:766 delete field prompt_layout (stable rules prefix)
        // The remaining fields (required_final_artifacts, missing_paths,
        // compile_reanchored_retry: 762, 764, 767) are not read by
        // `build_compile_regeneration_prompt_with_context`, so no behaviour
        // observable from here depends on them.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        copy_tree(
            Path::new("tests/corpus/apps/issue456-create-recovery/original"),
            root,
        );
        // The advisory fires for a collection fed by a local default.
        let original_page = std::fs::read_to_string(root.join("src/app/page.tsx")).unwrap();
        let broken_page = format!(
            "{}\nBROKEN_SYNTAX\n",
            original_page.replace(
                "useState<Member[]>([])",
                "useState<Member[]>(DEFAULT_MEMBERS)",
            )
        );
        std::fs::write(root.join("src/app/page.tsx"), &broken_page).unwrap();
        let advisory = crate::planner::profiles::nextjs::contract_repair_advisory(root)
            .expect("fixture must trigger the Next.js contract advisory");
        let expected_paths = vec![
            "src/app/page.tsx".to_string(),
            "src/app/api/tasks/[id]/route.ts".to_string(),
        ];

        let mut client = Scripted::new(vec![Err("scripted provider failure".to_string())]);
        let config = config_for(root, PromptLayout::Stable);
        let outcome = run_regeneration(
            &mut client,
            &config,
            "Goal marker 511: build a task board",
            broken_report(2),
            &expected_paths,
        );
        assert!(!outcome.accepted);
        let prompt = client.prompt_text();

        assert!(
            prompt.contains("Compile-error regeneration for step `final-acceptance`."),
            "{prompt}"
        );
        // profile
        assert!(prompt.contains(&advisory), "{prompt}");
        // overall_goal
        assert!(
            prompt.contains("Overall goal:\nGoal marker 511: build a task board"),
            "{prompt}"
        );
        // expected_paths
        assert!(
            prompt.contains(
                "Relevant repair paths:\n- src/app/page.tsx\n- src/app/api/tasks/[id]/route.ts"
            ),
            "{prompt}"
        );
        // workspace_root
        assert!(
            prompt.contains(&format!(
                "Current content of src/app/page.tsx:\n```tsx\n{broken_page}\n```"
            )),
            "{prompt}"
        );
        // prompt_layout
        assert!(prompt.contains("Repair rules:"), "{prompt}");
        // failed turn restores the original file
        assert_eq!(
            std::fs::read_to_string(root.join("src/app/page.tsx")).unwrap(),
            broken_page
        );
    }

    #[test]
    fn compile_regeneration_prompt_layout_follows_config() {
        // kills: final_acceptance.rs:766 delete field prompt_layout (legacy keeps no prefix)
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src/app")).unwrap();
        std::fs::write(root.join("src/app/page.tsx"), BROKEN_PAGE).unwrap();
        let mut client = Scripted::new(vec![Err("scripted provider failure".to_string())]);
        let config = config_for(root, PromptLayout::Legacy);
        let outcome = run_regeneration(
            &mut client,
            &config,
            "goal",
            broken_report(2),
            &["src/app/page.tsx".to_string()],
        );
        assert!(!outcome.accepted);
        let prompt = client.prompt_text();
        assert!(
            prompt.contains("Compile-error regeneration for step `final-acceptance`."),
            "{prompt}"
        );
        assert!(!prompt.contains("Repair rules:"), "{prompt}");
    }

    #[test]
    fn compile_regeneration_without_compile_errors_is_a_no_op() {
        // kills: final_acceptance.rs:700 -> Ok(true)
        let dir = tempfile::tempdir().unwrap();
        let config = config_for(dir.path(), PromptLayout::Stable);
        let mut client = Scripted::new(Vec::new());
        let outcome = run_regeneration(
            &mut client,
            &config,
            "goal",
            VerificationReport::pass(),
            &[],
        );
        assert!(!outcome.accepted);
        assert!(client.requests.lock().unwrap().is_empty());
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    /// A Next.js workspace whose fake `npm run build` fails with a parsable
    /// compile frame while `BROKEN_SYNTAX` is present in the page.
    #[cfg(unix)]
    fn write_nextjs_workspace(root: &Path) {
        std::fs::create_dir_all(root.join("src/app")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
        for package in ["next", "tailwindcss", "postcss", "autoprefixer"] {
            let dir = root.join("node_modules").join(package);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("package.json"), "{}").unwrap();
        }
        std::fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"},"devDependencies":{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"},"scripts":{"build":"next build","dev":"next dev -p 3011","start":"next start -p 3011"}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("tsconfig.json"),
            r#"{"compilerOptions":{"target":"ES2017","lib":["dom","dom.iterable","esnext"],"allowJs":true,"skipLibCheck":true,"strict":true,"noEmit":true,"esModuleInterop":true,"module":"esnext","moduleResolution":"bundler","resolveJsonModule":true,"isolatedModules":true,"jsx":"preserve","incremental":true,"plugins":[{"name":"next"}],"baseUrl":".","paths":{"@/*":["./src/*"]}},"include":["next-env.d.ts","**/*.ts","**/*.tsx",".next/types/**/*.ts"],"exclude":["node_modules"]}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("postcss.config.js"),
            "module.exports = { plugins: { tailwindcss: {}, autoprefixer: {} } };\n",
        )
        .unwrap();
        std::fs::write(
            root.join("tailwind.config.ts"),
            "import type { Config } from 'tailwindcss';\nconst config: Config = { content: ['./src/app/**/*.{js,ts,jsx,tsx,mdx}'], theme: { extend: {} }, plugins: [] };\nexport default config;\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/app/layout.tsx"),
            "import './globals.css';\nexport default function Layout({children}:{children:React.ReactNode}){return <html><body>{children}</body></html>;}",
        )
        .unwrap();
        std::fs::write(
            root.join("src/app/globals.css"),
            "@tailwind base;\n@tailwind components;\n@tailwind utilities;\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/app/global.d.ts"),
            "declare module \"*.css\";\n",
        )
        .unwrap();
        std::fs::write(root.join("src/app/page.tsx"), BROKEN_PAGE).unwrap();
        write_executable(
            &root.join("node_modules/.bin/npm"),
            r#"#!/bin/sh
if [ "$1" = "run" ] && [ "$2" = "build" ]; then
  if grep -q 'BROKEN_SYNTAX' src/app/page.tsx; then
cat <<'EOF'
./src/app/page.tsx
Error:
  x Expected ';'

   ,-[./src/app/page.tsx:2:1]
1 | export default function Page(){
2 | BROKEN_SYNTAX
  | ^
3 | }
   `----
> Build failed because of webpack errors
EOF
    exit 1
  fi
  echo 'fake build ok'
  exit 0
fi
exit 2
"#,
        );
        write_executable(&root.join("node_modules/.bin/next"), "#!/bin/sh\nexit 0\n");
    }

    #[cfg(unix)]
    fn regeneration_scenario(
        replies: Vec<Result<AssistantReply, String>>,
    ) -> (tempfile::TempDir, Outcome) {
        let dir = tempfile::tempdir().unwrap();
        write_nextjs_workspace(dir.path());
        let config = config_for(dir.path(), PromptLayout::Stable);
        let mut client = Scripted::new(replies);
        let outcome = run_regeneration(
            &mut client,
            &config,
            "Create a static Next.js page",
            broken_report(2),
            &["src/app/page.tsx".to_string()],
        );
        (dir, outcome)
    }

    #[cfg(unix)]
    fn page_text(root: &Path) -> String {
        std::fs::read_to_string(root.join("src/app/page.tsx")).unwrap()
    }

    #[test]
    #[cfg(unix)]
    fn compile_regeneration_accepts_a_single_file_fix_that_removes_errors() {
        // kills: final_acceptance.rs:700 -> Ok(false)
        // kills: final_acceptance.rs:821 - -> + / - -> / (error delta is before minus after)
        // kills: final_acceptance.rs:822 > -> == / > -> < (a positive delta is accepted)
        let (dir, outcome) =
            regeneration_scenario(vec![write_reply("src/app/page.tsx", GOOD_PAGE)]);
        assert!(outcome.accepted, "{:?}", outcome.report);
        assert!(outcome.report.compile_errors.is_empty());
        assert_eq!(page_text(dir.path()), GOOD_PAGE);
        assert_eq!(
            outcome.context.created_or_changed_paths,
            vec!["src/app/page.tsx".to_string()]
        );
        assert_eq!(
            outcome.context.last_repair_changed_paths,
            vec!["src/app/page.tsx".to_string()]
        );
    }

    #[test]
    #[cfg(unix)]
    fn compile_regeneration_rejects_a_rewrite_that_keeps_the_error_count() {
        // kills: final_acceptance.rs:700 -> Ok(true)
        // kills: final_acceptance.rs:821 - -> + / - -> / (unchanged count is not an improvement)
        // kills: final_acceptance.rs:822 > -> >= (a zero delta is rejected)
        let still_broken = "export default function Page(){\nBROKEN_SYNTAX // rewritten\n}\n";
        let (dir, outcome) =
            regeneration_scenario(vec![write_reply("src/app/page.tsx", still_broken)]);
        assert!(!outcome.accepted);
        assert_eq!(outcome.report.compile_errors.len(), 1);
        assert_eq!(
            page_text(dir.path()),
            BROKEN_PAGE,
            "rejected rewrite is rolled back"
        );
        assert!(outcome.context.created_or_changed_paths.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn compile_regeneration_rejects_a_fix_that_also_touches_other_files() {
        // kills: final_acceptance.rs:822 && -> || (improvement alone is not enough)
        let mut both = write_reply("src/app/page.tsx", GOOD_PAGE).unwrap();
        both.tool_calls.push(ToolCall::new(
            "Write",
            json!({"path": "src/app/extra.ts", "content": "export const extra = 1;\n"}),
        ));
        let (dir, outcome) = regeneration_scenario(vec![Ok(both)]);
        assert!(!outcome.accepted, "{:?}", outcome.report);
        assert_eq!(page_text(dir.path()), BROKEN_PAGE);
        assert!(outcome.context.created_or_changed_paths.is_empty());
    }
}
