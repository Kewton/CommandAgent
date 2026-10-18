//! Issue #507: pin the compile-rollback and zero-edit-eligibility boundaries of
//! the final acceptance repair path. Each test names the cargo-mutants
//! mutations it is meant to kill. Mounted from
//! `runner/acceptance/issue507_tests.rs` so `final_acceptance.rs` does not grow.
#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use crate::planner::runner::final_acceptance::*;
    use crate::planner::runner::{CompileError, Config, RepairTarget, VerificationReport};
    use clap::Parser;
    use std::path::Path;

    fn rollback_config(root: &Path) -> Config {
        let mut config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            root.to_str().unwrap(),
            "--profile",
            "nextjs",
        ]))
        .unwrap();
        config.eval_events_path = Some(root.join("run/events.jsonl"));
        config
    }

    fn build_report(path: &str) -> VerificationReport {
        let mut report = VerificationReport::pass();
        report.push_compile_errors(
            "npm run build",
            vec![CompileError {
                path: path.to_string(),
                line: 12,
                column: 1,
                message: "Expected ';'".to_string(),
                excerpt: "12 | BROKEN_SYNTAX".to_string(),
                symbol: None,
                route_bound: Some(true),
            }],
        );
        report
    }

    const GOOD_PAGE: &str = "export default function Page(){return <main>Recovered</main>;}\n";
    const BROKEN_PAGE: &str = "export default function Page(){\nBROKEN_SYNTAX\n}\n";

    #[cfg(unix)]
    fn write_executable(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

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
            "#!/bin/sh\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then\n\
  if grep -q 'BROKEN_SYNTAX' src/app/page.tsx; then\n\
    echo 'Failed to compile.' >&2\n\
    exit 1\n\
  fi\n\
  echo 'fake build ok'\n\
  exit 0\n\
fi\n\
exit 2\n",
        );
        write_executable(&root.join("node_modules/.bin/next"), "#!/bin/sh\nexit 0\n");
    }

    fn write_snapshot(root: &Path, content: &str) {
        let dir = root.join("run/snapshots/latest/src/app");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("page.tsx"), content).unwrap();
    }

    fn run_rollback(config: &Config) -> Option<CompileRollbackOutcome> {
        try_compile_rollback_after_repair_exhaustion(
            config,
            "nextjs",
            "Create a static Next.js page",
            "phase-two",
            "Update the page copy.",
            &build_report("src/app/page.tsx"),
            "bounded_repair_exhausted",
        )
        .unwrap()
    }

    fn events_text(root: &Path) -> String {
        std::fs::read_to_string(root.join("run/events.jsonl")).unwrap_or_default()
    }

    fn event_lines(root: &Path, name: &str) -> Vec<serde_json::Value> {
        events_text(root)
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|event| event["event"] == name)
            .collect()
    }

    fn page_text(root: &Path) -> String {
        std::fs::read_to_string(root.join("src/app/page.tsx")).unwrap()
    }

    // kills: final_acceptance.rs:953 replace -> Ok(None)
    // kills: final_acceptance.rs:972 delete ! (snapshot present must be restored)
    // kills: final_acceptance.rs:1002 delete ! (both) -- static pass and build pass roll back
    #[test]
    #[cfg(unix)]
    fn rollback_applies_when_snapshot_static_and_build_all_pass() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_nextjs_workspace(root);
        write_snapshot(root, GOOD_PAGE);
        let config = rollback_config(root);

        let outcome = run_rollback(&config).expect("rollback must apply");

        assert_eq!(outcome.paths, vec!["src/app/page.tsx".to_string()]);
        assert_eq!(outcome.snapshot_origins.len(), 1);
        assert!(
            outcome.snapshot_origins[0].ends_with("run/snapshots/latest/src/app/page.tsx"),
            "{:?}",
            outcome.snapshot_origins
        );
        assert_eq!(
        outcome.carry_forward_guidance,
        vec![
            "phase phase-two changes to src/app/page.tsx were rolled back; re-apply: Update the page copy."
                .to_string()
        ]
    );
        assert_eq!(page_text(root), GOOD_PAGE);
        let applied = event_lines(root, "compile_rollback_applied");
        assert_eq!(applied.len(), 1, "{}", events_text(root));
        assert_eq!(applied[0]["build_reverified"], true);
        assert_eq!(applied[0]["build_reverification_status"], "passed");
        assert!(event_lines(root, "compile_rollback_failed").is_empty());
        assert!(event_lines(root, "compile_rollback_skipped").is_empty());
    }

    // kills: final_acceptance.rs:953 replace -> Ok(Some(Default::default()))
    // kills: final_acceptance.rs:972 delete ! (missing snapshot must skip, not restore)
    #[test]
    #[cfg(unix)]
    fn rollback_is_skipped_when_snapshot_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_nextjs_workspace(root);
        std::fs::create_dir_all(root.join("run/snapshots/latest")).unwrap();
        let config = rollback_config(root);

        assert!(run_rollback(&config).is_none());

        assert_eq!(page_text(root), BROKEN_PAGE);
        let skipped = event_lines(root, "compile_rollback_skipped");
        assert_eq!(skipped.len(), 1, "{}", events_text(root));
        assert!(
            skipped[0]
                .to_string()
                .contains("snapshot_missing:src/app/page.tsx"),
            "{}",
            skipped[0]
        );
        assert!(event_lines(root, "compile_rollback_applied").is_empty());
    }

    // kills: final_acceptance.rs:953 replace -> Ok(Some(Default::default()))
    #[test]
    #[cfg(unix)]
    fn rollback_is_skipped_when_snapshot_store_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_nextjs_workspace(root);
        let mut config = rollback_config(root);
        config.eval_events_path = None;

        assert!(run_rollback(&config).is_none());

        assert_eq!(page_text(root), BROKEN_PAGE);
    }

    // kills: final_acceptance.rs:1002 `||` -> `&&` (static fails, build passes)
    // kills: final_acceptance.rs:1002 delete first ! (static fails must reject)
    #[test]
    #[cfg(unix)]
    fn rollback_is_rejected_when_static_verification_fails_but_build_passes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_nextjs_workspace(root);
        write_snapshot(root, GOOD_PAGE);
        std::fs::remove_file(root.join("src/app/layout.tsx")).unwrap();
        let config = rollback_config(root);

        assert!(run_rollback(&config).is_none());

        let failed = event_lines(root, "compile_rollback_failed");
        assert_eq!(failed.len(), 1, "{}", events_text(root));
        assert_eq!(failed[0]["build_reverification_status"], "passed");
        assert_eq!(failed[0]["build_reverified"], false);
        assert!(
            failed[0]["rebuild_reason"]
                .as_str()
                .unwrap()
                .contains("layout missing"),
            "{}",
            failed[0]
        );
        assert!(event_lines(root, "compile_rollback_applied").is_empty());
    }

    // kills: final_acceptance.rs:1002 `||` -> `&&` (static passes, build fails)
    // kills: final_acceptance.rs:1002 delete second ! (failing build must reject)
    #[test]
    #[cfg(unix)]
    fn rollback_is_rejected_when_build_fails_but_static_verification_passes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_nextjs_workspace(root);
        write_snapshot(root, BROKEN_PAGE);
        let config = rollback_config(root);

        assert!(run_rollback(&config).is_none());

        let failed = event_lines(root, "compile_rollback_failed");
        assert_eq!(failed.len(), 1, "{}", events_text(root));
        assert_eq!(failed[0]["build_reverification_status"], "failed");
        assert_eq!(failed[0]["build_reverified"], false);
        assert!(event_lines(root, "compile_rollback_applied").is_empty());
    }

    fn missing_evidence_report() -> VerificationReport {
        VerificationReport::profile_failed(
            "missing_required_evidence:restart_or_recoverable_state_evidence",
        )
    }

    fn one_compile_error() -> Vec<CompileError> {
        vec![CompileError {
            path: "src/app/page.tsx".to_string(),
            line: 1,
            column: 1,
            message: "Expected ';'".to_string(),
            excerpt: "1 | BROKEN".to_string(),
            symbol: None,
            route_bound: Some(true),
        }]
    }

    // kills: final_acceptance.rs:608 replace -> true / -> false
    // kills: final_acceptance.rs:610 delete !
    #[test]
    fn zero_edit_is_eligible_for_implementation_with_missing_evidence_and_no_compile_errors() {
        assert!(evidence_repair_zero_edit_eligible(
            &missing_evidence_report(),
            RepairTarget::Implementation
        ));
    }

    // kills: final_acceptance.rs:608 replace -> true
    // kills: final_acceptance.rs:609 `&&` -> `||` (compile errors present alone)
    // kills: final_acceptance.rs:610 `&&` -> `||`
    #[test]
    fn zero_edit_is_not_eligible_when_compile_errors_remain() {
        let mut report = missing_evidence_report();
        report.push_compile_errors("npm run build", one_compile_error());

        assert!(!evidence_repair_zero_edit_eligible(
            &report,
            RepairTarget::Implementation
        ));
    }

    // kills: final_acceptance.rs:608 replace -> true
    // kills: final_acceptance.rs:609 `==` -> `!=`, `&&` -> `||` (non-implementation target alone)
    #[test]
    fn zero_edit_is_not_eligible_for_a_non_implementation_target() {
        assert!(!evidence_repair_zero_edit_eligible(
            &missing_evidence_report(),
            RepairTarget::RequiredEvidenceMissing
        ));
        assert!(!evidence_repair_zero_edit_eligible(
            &missing_evidence_report(),
            RepairTarget::DependencySetup
        ));
    }

    // kills: final_acceptance.rs:608 replace -> true
    // kills: final_acceptance.rs:610 delete !, `&&` -> `||` (no missing signals alone)
    #[test]
    fn zero_edit_is_not_eligible_without_missing_evidence_signals() {
        assert!(!evidence_repair_zero_edit_eligible(
            &VerificationReport::pass(),
            RepairTarget::Implementation
        ));
    }
}
