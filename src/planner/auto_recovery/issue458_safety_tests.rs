#[cfg(test)]
mod safety {
    use super::*;

    #[test]
    fn issue458_readiness_compile_diagnosis_survives_into_canonical_continuation() {
        use crate::minimal_loop::browser_probe::BrowserReadinessObservation;
        let continuation = |file: &str, message: &str| {
            let dir = tempfile::tempdir().unwrap();
            let config = setup(dir.path(), 3, Some(CHECK));
            let first = captured(&config, "compile_error", "original diagnostic");
            let mut planner = UnusedClient;
            let mut execution = UnusedClient;
            let mut real = runner(&config, &mut planner, &mut execution);
            let prepared = real.prepare(&first).unwrap();
            real.start(1, &first, &prepared).unwrap();
            let treatment = real.transaction_config.as_ref().unwrap();
            let observation_root = treatment
                .workspace_root
                .join(".commandagent/recovery-observations/attempt-129/workspace");
            let observation = BrowserReadinessObservation {
                ok: false,
                status: "failed".into(),
                profile: "nextjs".into(),
                port: 34001,
                route: "/".into(),
                command: "npm run build".into(),
                http_status: None,
                failure_kind: "build_verifier_failed".into(),
                evidence_path: observation_root.join("evidence/readiness.json"),
                elapsed_ms: 123,
                output_excerpt: "full build log remains in readiness evidence".into(),
                build_output_path: observation_root.join("build.log").to_string_lossy().into(),
                compile_errors: vec![crate::minimal_loop::build_verifier::CompileError {
                    path: observation_root.join(file).to_string_lossy().into(),
                    line: 3,
                    column: 2,
                    message: message.into(),
                    excerpt: String::new(),
                    symbol: None,
                    route_bound: None,
                }],
                child_spawned: false,
                child_reaped: true,
                has_canvas: false,
                interactive_control_count: 0,
                title_text_excerpt: String::new(),
            };
            let RecoveryPreflight::Failed { reason, identity } = retry::readiness_failure(
                &observation,
                &observation_root,
                "nextjs_route_observation_failed:build_verifier_failed".into(),
            ) else {
                panic!("typed compiler failure must be diagnosed")
            };
            assert!(reason.contains(message));
            assert!(reason.contains(&format!("./{file}:3:2")));
            let before = current_source_sha256(dir.path()).unwrap();
            let outcome = retry::reject_verification(
                &config,
                treatment,
                1,
                real.transaction_snapshot.take().unwrap(),
                &first,
                success("execution claimed success"),
                (&reason, identity),
            );
            assert_eq!(current_source_sha256(dir.path()).unwrap(), before);
            let Some(AttemptFailure::Recoverable(candidate)) = outcome.failure else {
                panic!("{outcome:?}")
            };
            assert!(
                candidate
                    .plan
                    .phases
                    .iter()
                    .any(|phase| phase.prompt.contains(message))
            );
            assert!(
                candidate
                    .plan
                    .phases
                    .iter()
                    .all(|phase| !phase.prompt.contains("recovery-observations"))
            );
            (
                normalized_plan(&candidate.plan).unwrap(),
                candidate.retry_identity.unwrap(),
            )
        };
        let original = continuation("app.txt", "TS2322: Type mismatch");
        let moved_root = continuation("app.txt", "TS2322: Type mismatch");
        assert_eq!(
            original, moved_root,
            "only the treatment/observation root changed"
        );
        for (file, message) in [
            ("app.txt", "TS2345: Invalid argument"),
            ("other.txt", "TS2322: Type mismatch"),
            ("app.txt", "TS2322: Different expected type"),
        ] {
            let changed = continuation(file, message);
            assert_ne!(
                original.0, changed.0,
                "semantic compiler change must change the next plan"
            );
            assert_ne!(
                original.1, changed.1,
                "semantic compiler change must change the failure identity"
            );
        }
    }

    #[test]
    fn issue458_production_finish_validates_child_before_rebuilding() {
        for (mode, stop) in [
            ("missing", CandidateStop::RecoveryYamlMissing),
            ("corrupt", CandidateStop::RecoveryYamlInvalid),
            ("review", CandidateStop::RecoveryNeedsReview),
            ("drift", CandidateStop::WorkspaceDrift),
            ("escape", CandidateStop::PathEscape),
            ("target_escape", CandidateStop::PathEscape),
            ("target_protected", CandidateStop::ResumeSafetyRejected),
            ("target_private", CandidateStop::ResumeSafetyRejected),
            ("target_symlink", CandidateStop::ResumeSafetyRejected),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let external = tempfile::tempdir().unwrap();
            let config = setup(dir.path(), 2, Some(CHECK));
            let before = current_source_sha256(dir.path()).unwrap();
            let first = captured(&config, "compile_error", "initial failure");
            let mut planner = UnusedClient;
            let mut execution = UnusedClient;
            let mut real = runner(&config, &mut planner, &mut execution);
            let prepared = real.prepare(&first).unwrap();
            real.start(1, &first, &prepared).unwrap();
            let treatment = real.transaction_config.as_ref().unwrap();
            let mut child = captured(treatment, "compile_error", "latest diagnostic");
            match mode {
                "missing" => std::fs::remove_file(&child.path).unwrap(),
                "corrupt" => std::fs::write(&child.path, "invalid: [").unwrap(),
                "review" => {
                    let text = std::fs::read_to_string(&child.path).unwrap();
                    std::fs::write(&child.path, format!("recovery_needs_review: true\n{text}"))
                        .unwrap();
                }
                "drift" => {
                    let text = std::fs::read_to_string(&child.path).unwrap();
                    std::fs::write(
                        &child.path,
                        format!("recovery_expected_completed_artifacts:\n  - missing.txt\n{text}"),
                    )
                    .unwrap();
                }
                "escape" => {
                    let path = external.path().join("child.yaml");
                    std::fs::copy(&child.path, &path).unwrap();
                    child.path = path;
                }
                "target_escape" => child.handoff.repair_targets = vec!["../outside.txt".into()],
                "target_protected" => child.handoff.repair_targets = vec!["frozen.txt".into()],
                "target_private" => {
                    child.handoff.repair_targets = vec![".commandagent/contract.json".into()]
                }
                "target_symlink" => {
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(
                        treatment.workspace_root.join("frozen.txt"),
                        treatment.workspace_root.join("alias.txt"),
                    )
                    .unwrap();
                    child.handoff.repair_targets = vec!["alias.txt".into()];
                }
                _ => unreachable!(),
            }
            let outcome = real.finish(1, &first, failed(recoverable(child)));
            // A product symlink can fail the boundary snapshot's stricter
            // source check before target rebinding. Either boundary stops it.
            if mode == "target_symlink"
                && matches!(outcome.failure, Some(AttemptFailure::NonRecoverable))
            {
                assert!(outcome.result.is_err());
            } else {
                assert!(
                    matches!(outcome.failure, Some(AttemptFailure::Stopped(reason)) if reason == stop),
                    "{mode}: {outcome:?}"
                );
            }
            assert_eq!(current_source_sha256(dir.path()).unwrap(), before, "{mode}");
            assert!(events(&config).iter().any(
                |e| e["event"] == "recovery_promotion_decision" && e["decision"] == "rejected"
            ));
        }
    }

    #[test]
    fn issue458_production_readiness_classification_requires_typed_product_evidence() {
        use crate::minimal_loop::browser_probe::BrowserReadinessObservation;
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        for case in fixture["readiness"].as_array().unwrap() {
            let mut observation = BrowserReadinessObservation {
                ok: false,
                status: "failed".into(),
                profile: "nextjs".into(),
                port: 34001,
                route: "/".into(),
                command: "npm start".into(),
                http_status: case["http_status"].as_i64(),
                failure_kind: case["kind"].as_str().unwrap().into(),
                evidence_path: PathBuf::from("evidence/readiness.json"),
                elapsed_ms: 987,
                output_excerpt: "start exited: Operation not permitted; looks like compile_error"
                    .into(),
                build_output_path: String::new(),
                compile_errors: vec![],
                child_spawned: false,
                child_reaped: true,
                has_canvas: false,
                interactive_control_count: 0,
                title_text_excerpt: String::new(),
            };
            if case["compile"] == true {
                observation.compile_errors.push(
                    crate::minimal_loop::build_verifier::CompileError {
                        path: "src/app/page.tsx".into(),
                        line: 1,
                        column: 1,
                        message: "type mismatch".into(),
                        excerpt: String::new(),
                        symbol: None,
                        route_bound: None,
                    },
                );
            }
            let result = retry::readiness_failure(
                &observation,
                Path::new("/fixture"),
                format!(
                    "nextjs_route_observation_failed:{}",
                    observation.failure_kind
                ),
            );
            assert_eq!(
                matches!(result, RecoveryPreflight::Failed { .. }),
                case["retry"].as_bool().unwrap(),
                "{case}: {result:?}"
            );
        }
    }

    #[test]
    fn issue458_real_nextjs_readiness_early_exit_is_unavailable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let mut config = setup(dir.path(), 2, Some(CHECK));
        config.offline = false;
        std::fs::write(dir.path().join("package.json"), r#"{"scripts":{"build":"next build","start":"next start"},"dependencies":{"next":"0.0.0"}}"#).unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/.bin")).unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/next")).unwrap();
        let npm = dir.path().join("node_modules/.bin/npm");
        std::fs::write(&npm, "#!/bin/sh\nif [ \"$2\" = build ]; then exit 0; fi\nprintf 'Operation not permitted\\n' >&2\nexit 1\n").unwrap();
        std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::os::unix::fs::symlink("npm", dir.path().join("node_modules/.bin/next")).unwrap();
        let contract =
            crate::minimal_loop::completion::CompletionContract::load_for_config(&config)
                .unwrap()
                .unwrap();
        let result = observe_nextjs_recovery_capabilities(&config, &contract, dir.path());
        assert!(
            matches!(result, Err(RecoveryPreflight::Unavailable { ref reason }) if reason == "nextjs_route_observation_failed:start_exited"),
            "{result:?}"
        );
    }
}
