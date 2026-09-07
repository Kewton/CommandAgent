#[cfg(test)]
mod tests {
    use super::super::tests::{config, contract_json};
    use super::super::*;

    fn write_contract(config: &Config, profile: &str, commands: &[&str]) -> std::path::PathBuf {
        let path = generated_path(config, ULTRA_RUN_CONTRACT);
        let mut contract: CompletionContract = serde_json::from_str(&contract_json()).unwrap();
        contract.profile = Some(profile.to_string());
        contract.verify_commands = commands.iter().map(|command| command.to_string()).collect();
        std::fs::write(&path, serde_json::to_vec(&contract).unwrap()).unwrap();
        record_generated_contract(config, "ultra-plan-run", &path);
        path
    }

    #[test]
    fn empty_generated_nextjs_contract_breaks_the_empty_handoff_cycle() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let _authority = begin_run(&config);
        write_contract(&config, "nextjs", &[]);
        let bound = bind_for_recovery(&config, &[]).unwrap();
        let contract = CompletionContract::load_for_config(&bound)
            .unwrap()
            .unwrap();
        assert_eq!(contract.verify_commands, ["npm run build"]);
        assert_eq!(contract.required_capabilities, ["persistence"]);
        assert!(
            contract
                .required_evidence
                .contains(&"persistence_evidence".into())
        );
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("\"source\":\"profile_runtime\""));
        assert!(events.contains("\"registered_verify_commands_from_failed_plan\":0"));
    }

    #[test]
    fn generated_registration_is_validated_and_refresh_preserves_all_checks() {
        for profile in ["nextjs", "generic"] {
            let root = tempfile::tempdir().unwrap();
            let config = config(root.path());
            let _authority = begin_run(&config);
            let path = write_contract(&config, profile, &[]);
            register_step_plan_commands(&config, &["cargo test".into(), "cargo test".into()])
                .unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert!(
                register_step_plan_commands(
                    &config,
                    &["cargo test --manifest-path ../Cargo.toml".into()]
                )
                .is_err()
            );
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            let commands =
                generated_verify_commands(&config, "ultra-plan-run", profile, "create a todo app")
                    .unwrap();
            assert!(commands.contains(&"cargo test".into()));
            assert_eq!(
                commands.contains(&"npm run build".into()),
                profile == "nextjs"
            );
            assert!(
                generated_verify_commands(&config, "plan-run", profile, "step")
                    .unwrap()
                    .is_empty()
            );
            assert!(
                !generated_verify_commands(&config, "ultra-plan-run", profile, "different goal")
                    .unwrap()
                    .contains(&"cargo test".into())
            );
        }
    }

    #[test]
    fn configured_data_and_nonempty_registries_never_admit_recovery_proposals() {
        for profile in ["nextjs", "generic", "data"] {
            for configured in [false, true] {
                for commands in [vec![], vec!["test -f registered.py"]] {
                    let root = tempfile::tempdir().unwrap();
                    let mut config = config(root.path());
                    let _authority = begin_run(&config);
                    let path = write_contract(&config, profile, &commands);
                    if configured {
                        config.completion_contract_path = Some(path.clone());
                    }
                    let original = std::fs::read(&path).unwrap();
                    if configured || profile == "data" {
                        register_step_plan_commands(&config, &["test -f proposal.py".into()])
                            .unwrap();
                    }
                    if configured || profile == "data" || !commands.is_empty() {
                        let bound =
                            bind_for_recovery(&config, &["test -f proposal.py".into()]).unwrap();
                        assert_eq!(std::fs::read(&path).unwrap(), original);
                        assert_eq!(
                            CompletionContract::load_for_config(&bound)
                                .unwrap()
                                .unwrap()
                                .verify_commands,
                            commands
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn empty_generic_contract_without_registered_authority_stays_empty() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let _authority = begin_run(&config);
        let path = write_contract(&config, "generic", &[]);
        let before = std::fs::read(&path).unwrap();
        let bound = bind_for_recovery(&config, &[]).unwrap();
        assert!(handoff_commands(&bound, &[]).is_empty());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn stagnation_uses_run_authority_without_widening_step_acceptance() {
        for step_commands in [vec![], vec!["test -f step.py"]] {
            let root = tempfile::tempdir().unwrap();
            let mut config = config(root.path());
            let _authority = begin_run(&config);
            write_contract(&config, "nextjs", &["npm run build"]);
            let step_path = generated_path(&config, "completion-contract-plan-run.json");
            std::fs::write(
                &step_path,
                serde_json::json!({
                    "goal": "Narrow step goal", "verify_commands": step_commands,
                })
                .to_string(),
            )
            .unwrap();
            record_generated_contract(&config, "plan-run", &step_path);
            config.completion_contract_path = Some(step_path.clone());
            let before = std::fs::read(&step_path).unwrap();
            let fidelity =
                crate::minimal_loop::recovery_handoff_fidelity::RecoveryHandoffFidelity::resolve(
                    &config,
                    "Execute one step",
                    &["src/app/page.tsx".into()],
                    &[],
                )
                .unwrap();
            assert_eq!(fidelity.original_goal, "create a todo app");
            assert_eq!(fidelity.verify_commands, ["npm run build"]);
            assert!(fidelity.is_complete());
            assert_eq!(std::fs::read(step_path).unwrap(), before);
            assert_eq!(
                CompletionContract::load_for_config(&config)
                    .unwrap()
                    .unwrap()
                    .verify_commands,
                step_commands
            );
        }
    }

    #[test]
    fn bounded_repair_preserves_four_one_one_commands_and_fills_only_empty_handoffs() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let _authority = begin_run(&config);
        write_contract(&config, "nextjs", &["npm run build"]);
        for existing in [
            vec![
                "npm run build",
                "test -f package.json",
                "test -f src/app/page.tsx",
                "test -f src/app/layout.tsx",
            ],
            vec!["npm run build"],
            vec!["test -f src/app/page.tsx"],
        ] {
            let existing = existing.into_iter().map(str::to_string).collect::<Vec<_>>();
            assert_eq!(handoff_commands(&config, &existing), existing);
        }
        assert_eq!(handoff_commands(&config, &[]), ["npm run build"]);
    }

    #[test]
    fn invalid_authority_fails_handoff_honestly_without_registering_commands() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let _authority = begin_run(&config);
        let path = write_contract(
            &config,
            "nextjs",
            &["cargo test --manifest-path ../Cargo.toml"],
        );
        let before = std::fs::read(&path).unwrap();
        assert!(load_for_handoff(&config).is_err());
        assert!(handoff_commands(&config, &[]).is_empty());
        assert!(bind_for_recovery(&config, &[]).is_err());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn explicitly_configured_generated_filename_and_canonical_alias_remain_closed() {
        let root = tempfile::tempdir().unwrap();
        let mut config = config(root.path());
        let _authority = begin_run(&config);
        write_contract(&config, "nextjs", &["npm run build"]);
        let step_path = generated_path(&config, "completion-contract-plan-run.json");
        let text = r#"{"goal":"Explicit user goal","verify_commands":["test -f user.py"]}"#;
        std::fs::write(&step_path, text).unwrap();
        let mut paths = vec![step_path.clone()];
        #[cfg(unix)]
        {
            let alias = root.path().join("explicit-alias.json");
            std::os::unix::fs::symlink(&step_path, &alias).unwrap();
            paths.push(alias);
        }
        for path in paths {
            config.completion_contract_path = Some(path);
            let contract = load_for_handoff(&config).unwrap().unwrap();
            assert_eq!(contract.goal.as_deref(), Some("Explicit user goal"));
            assert_eq!(contract.verify_commands, ["test -f user.py"]);
            let bound = bind_for_recovery(&config, &["npm run build".into()]).unwrap();
            assert_eq!(
                CompletionContract::load_for_config(&bound)
                    .unwrap()
                    .unwrap()
                    .verify_commands,
                ["test -f user.py"]
            );
            assert_eq!(std::fs::read_to_string(&step_path).unwrap(), text);
        }
    }

    #[test]
    fn fresh_run_does_not_import_prior_commands_even_with_the_same_profile_and_goal() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        {
            let _first = begin_run(&config);
            write_contract(&config, "nextjs", &["test -f prior-run-only"]);
            assert!(
                generated_verify_commands(&config, "ultra-plan-run", "nextjs", "create a todo app")
                    .unwrap()
                    .contains(&"test -f prior-run-only".into())
            );
        }
        // A later standalone minimal-loop call also has no ownership of the
        // previous UltraPlan's generated contract.
        assert!(load_for_handoff(&config).unwrap().is_none());
        let _fresh = begin_run(&config);
        assert_eq!(
            generated_verify_commands(&config, "ultra-plan-run", "nextjs", "create a todo app")
                .unwrap(),
            ["npm run build"]
        );
        assert!(load_for_handoff(&config).unwrap().is_none());
        assert!(
            bind_for_recovery(&config, &[])
                .unwrap()
                .completion_contract_path
                .is_none()
        );
        register_step_plan_commands(&config, &["test -f never-admitted".into()]).unwrap();
        let stale: CompletionContract = serde_json::from_slice(
            &std::fs::read(generated_path(&config, ULTRA_RUN_CONTRACT)).unwrap(),
        )
        .unwrap();
        assert_eq!(stale.verify_commands, ["test -f prior-run-only"]);
    }

    #[test]
    fn profile_configuration_assertions_remain_enforced_by_the_final_profile_gate() {
        use crate::planner::profiles::nextjs;
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let _authority = begin_run(&config);
        write_contract(&config, "nextjs", &["npm run build"]);
        let checks = nextjs::setup_step_checks(
            root.path(),
            "create a todo app",
            "ensure-port-scripts",
            "package.json scripts",
        )
        .unwrap();
        assert_eq!(checks.verify_commands.len(), 3);
        register_step_plan_commands(&config, &checks.verify_commands).unwrap();
        assert_eq!(handoff_commands(&config, &[]), ["npm run build"]);
        // No blanket exemption for node commands or assertions about a different port.
        assert!(!nextjs::recovery_authority::final_verifier_covers_command(
            "create a todo app",
            "node -p '1+1'"
        ));
        let other_port = nextjs::setup_step_checks(
            root.path(),
            "port 4011",
            "ensure-port-scripts",
            "package.json scripts",
        )
        .unwrap();
        assert!(!nextjs::recovery_authority::final_verifier_covers_command(
            "create a todo app",
            &other_port.verify_commands[0]
        ));

        std::fs::create_dir_all(root.path().join("src/app")).unwrap();
        std::fs::write(
            root.path().join("src/app/page.jsx"),
            "export default function Page(){return <main>Todo</main>}",
        )
        .unwrap();
        std::fs::write(root.path().join("src/app/layout.jsx"), "export default function Layout({children}){return <html><body>{children}</body></html>}").unwrap();
        let package = json!({"scripts":{"build":"next build","dev":"next dev -p 3011","start":"next start -p 3011"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}});
        std::fs::write(root.path().join("package.json"), package.to_string()).unwrap();
        let valid = nextjs::verify(root.path(), "create a todo app");
        assert!(valid.is_pass(), "{valid:?}");
        for (script, invalid, reason) in [
            ("build", "echo skipped", "scripts.build"),
            ("dev", "next dev -p 4011", "dev script"),
            ("start", "next start -p 4011", "start script"),
        ] {
            let mut broken = package.clone();
            broken["scripts"][script] = invalid.into();
            std::fs::write(root.path().join("package.json"), broken.to_string()).unwrap();
            let failed = nextjs::verify(root.path(), "create a todo app");
            assert!(!failed.is_pass());
            assert!(failed.primary_reason().contains(reason), "{failed:?}");
        }
    }
}
