use super::*;

#[test]
fn issue465_new_build_config_cannot_hide_the_defect_even_when_registered_check_passes() {
    for repair in [false, true] {
        let (_root, config) = setup_with(|config| {
            std::fs::copy(
                Path::new(FIXTURE).join("config-sensitive-check.js"),
                config.workspace_root.join("checks/config-sensitive.js"),
            )
            .unwrap();
            super::review::edit_contract(config, |contract| {
                contract["verify_commands"] = json!(["node checks/config-sensitive.js"]);
            });
        });
        assert!(!config.workspace_root.join("next.config.js").exists());
        let original_api = std::fs::read(config.workspace_root.join(API)).unwrap();
        let contract = CompletionContract::load_for_config(&config)
            .unwrap()
            .unwrap();
        assert!(!contract.verify(&config.workspace_root).is_pass());
        let action = if repair {
            fix_store()
        } else {
            tool(
                "Write",
                json!({"path":"next.config.js", "content":"export default { typescript: { ignoreBuildErrors: true } };"}),
            )
        };
        let result = crate::planner::run_step_plan_with_ui(
            &mut Replay::new(vec![
                failed_edit(),
                cat("cat src/api.js"),
                action,
                AssistantReply::text("Completed"),
            ]),
            &plan(),
            &config,
            &crate::tui::NOOP_UI,
        );
        // Both the bypass and the genuine related-store fix make this registered
        // check pass. Only the genuine repair may complete the Runner step.
        assert!(contract.verify(&config.workspace_root).is_pass());
        assert_eq!(
            std::fs::read(config.workspace_root.join(API)).unwrap(),
            original_api
        );
        assert_eq!(result.is_ok(), repair, "{result:?}");
        let log = events(&config);
        assert_eq!(log.iter().any(|e| e["status"] == "resolved"), repair);
        if !repair {
            assert!(
                log.iter()
                    .any(|e| e["failure_reason"].as_str().is_some_and(|r| r
                        .contains("configuration added")
                        && r.contains("next.config.js")))
            );
        }
    }
}

#[test]
fn issue465_new_ignored_nested_and_symlinked_configuration_is_not_an_unchecked_input() {
    for path in [
        "next.config.js",
        "tsconfig.build.json",
        "jsconfig.json",
        "nested/next.config.mjs",
        ".babelrc",
        ".npmrc",
    ] {
        let (_root, config) = setup();
        let opts = options(&config, &plan(), 0);
        let destination = config.workspace_root.join(path);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(config.workspace_root.join(".ignore"), "*\n").unwrap();
        std::fs::write(destination, "{}").unwrap();
        assert!(
            opts.recovery_obligation
                .as_ref()
                .unwrap()
                .feedback(&config, &opts)
                .unwrap()
                .contains("configuration added"),
            "{path}"
        );
    }
    #[cfg(unix)]
    {
        let (_root, config) = setup();
        let opts = options(&config, &plan(), 0);
        std::fs::write(
            config.workspace_root.join("bypass.js"),
            "export default {};",
        )
        .unwrap();
        std::os::unix::fs::symlink("bypass.js", config.workspace_root.join("next.config.js"))
            .unwrap();
        assert!(
            opts.recovery_obligation
                .as_ref()
                .unwrap()
                .feedback(&config, &opts)
                .unwrap()
                .contains("configuration added")
        );
    }
}
