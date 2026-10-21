use super::*;

fn full_plan(name: &str) -> StepPlan {
    let mut p = proposal_plan(name);
    let mut app = step(
        "application",
        "implement",
        &["src/app/page.tsx", "README.md"],
    );
    app.instruction =
        "Retain all four source marker conditions and document their text-only scope in README.md."
            .into();
    let mut package = step("configuration", "implement", &["package.json"]);
    package.instruction = "Preserve the package scripts and dependency versions.".into();
    p.steps.insert(0, app);
    p.steps.insert(1, package);
    p.steps[2].verify.push("npm run build".into());
    p
}

fn replay(
    c: &Config,
    goal: &str,
    replies: Vec<AssistantReply>,
) -> (anyhow::Result<StepPlan>, Replay) {
    let mut client = Replay::new(replies);
    let result = crate::planner::runner::generate_step_plan_with_ui_for_phase(
        &mut client,
        goal,
        c,
        &crate::tui::NOOP_UI,
        Some("core-implementation"),
        true,
        false,
    );
    (result, client)
}

fn audit(
    label: &str,
    c: &Config,
    result: &anyhow::Result<StepPlan>,
    client: &Replay,
) -> serde_json::Value {
    let requests = client.requests.lock().unwrap();
    let requests: Vec<_> = requests.iter().enumerate().map(|(i, messages)| json!({
        "proposal_attempt":i+1,
        "request_sha256":format!("{:x}", Sha256::digest(serde_json::to_vec(messages).unwrap())),
        "user_body":messages.iter().rfind(|m|m.role == "user").map(|m| &m.content),
        "message_count":messages.len(),
    })).collect();
    let boundaries: Vec<_> = events(c).into_iter().filter(|e| matches!(e["event"].as_str(),
        Some("recovery_verifier_plan_admission" | "recovery_verifier_plan_return_rejected" | "planner_quality_retry" | "planner_quality_retry_degraded" | "planner_fallback_plan")))
        .map(|e| json!({"event":e["event"],"planner_attempt":e["planner_attempt"],"repair_attempt":e["repair_attempt"],
            "status":e["status"],"remaining_planner_attempts":e["remaining_planner_attempts"],"recovery_budget_changed":e["recovery_budget_changed"]})).collect();
    let validated: Vec<_> = events(c)
        .into_iter()
        .filter(|e| e["event"] == "preclosure_verifier_replacements_validated")
        .map(|e| {
            json!({"replacements":e["replacements"],
            "trusted_contract":e["package_script_formation"]["registered_contract"],
            "trusted_contract_sha256":e["package_script_formation"]["registered_contract_sha256"],
            "scope_preserved":e["scope_preserved"]})
        })
        .collect();
    json!({"series":label, "requests":requests, "validated":validated,
        "returned_plan":result.as_ref().ok(), "error":result.as_ref().err().map(|e| format!("{e:#}")),
        "boundaries":boundaries, "provider":"successful offline mock replies; no network retry or live model"})
}

#[test]
fn issue488_mock_rewrite_accepts_full_requirements_and_records_mapping() {
    let mut audits = Vec::new();
    for name in ["O", "V1", "SWALLOW"] {
        let (_root, c, _guard, path) = setup();
        let before = full_plan(name);
        let after = full_plan("V2");
        let bytes = std::fs::read(&path).unwrap();
        let (result, client) = replay(
            &c,
            &before.goal,
            vec![proposal(&before), proposal(&after), proposal(&after)],
        );
        audits.push(audit(name, &c, &result, &client));
        let formed = result.unwrap_or_else(|e| panic!("{name}: {e:#}\n{}", json!(events(&c))));
        assert_eq!(client.requests.lock().unwrap().len(), 2);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "planning must not register"
        );
        assert_eq!(formed.steps[0], after.steps[0]);
        assert_eq!(formed.steps[2], after.steps[2]);
        assert!(
            formed.steps[1]
                .instruction
                .contains(&after.steps[1].instruction)
        );
        let log = events(&c);
        let refusal = log
            .iter()
            .find(|e| e["event"] == "recovery_verifier_plan_admission")
            .unwrap();
        let host = refusal["original_obligation_sources"]["host"]["steps"][0]["instruction"]
            .as_str()
            .unwrap();
        assert!(formed.steps[1].instruction.contains(host));
        let requests = client.requests.lock().unwrap();
        let feedback = requests[1]
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(feedback.contains("current classifier cannot recognize"));
        assert!(feedback.contains(&serde_json::to_string(&before.steps[2].verify[0]).unwrap()));
        for step in &before.steps {
            assert!(feedback.contains(&step.instruction));
        }
        assert!(feedback.contains(serde_json::to_string(host).unwrap().trim_matches('"')));
        let record = log
            .iter()
            .find(|e| e["event"] == "preclosure_verifier_replacements_validated")
            .unwrap();
        let mapping = &record["replacements"][0];
        assert_eq!(mapping["original_command"], command(name));
        assert_eq!(mapping["replacement_command"], command("V2"));
        assert_eq!(mapping["literal_markers"]["original_step"], "check-markers");
        assert_eq!(
            mapping["literal_markers"]["predicates"]["checks"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(refusal["recovery_budget_changed"], false);
        scope::register(&c, &formed).unwrap();
        let contract = authority::load_for_handoff(&c).unwrap().unwrap();
        assert!(contract.verify_commands.contains(&command("V2")));
        assert!(!contract.verify_commands.contains(&command(name)));
    }
    if let Ok(dir) = std::env::var("ISSUE488_PLANNER_EVIDENCE") {
        std::fs::write(
            Path::new(&dir).join("planner-success.json"),
            serde_json::to_vec_pretty(&audits).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn issue488_mock_exhaustion_and_missing_duties_never_register_or_execute() {
    let mut audits = Vec::new();
    let cases: Vec<String> = serde_json::from_str(&fixture("refusals.json")).unwrap();
    for case in cases.iter().map(String::as_str) {
        let (root, c, _guard, path) = setup();
        let mut before = full_plan("O");
        let mut after = full_plan("V2");
        match case {
            "three_weak" => after = before.clone(),
            "shell_override" => after.steps[2].verify[0] = format!("{} || true", command("V2")),
            "condition" => {
                after.steps[2].verify[0] = command("V2").replace(
                    " assert(s.includes('data-anvil-state'),\\\"missing state\\\");",
                    "",
                )
            }
            "target" => {
                after.steps[2].verify[0] =
                    command("V2").replace("src/app/page.tsx", "src/app/other.tsx")
            }
            "stdout" => {
                after.steps[2].verify[0] =
                    command("V2").replace("console.log('ok')", "console.log('changed')")
            }
            "message" => {
                after.steps[2].verify[0] =
                    command("V2").replace("missing primary", "ignored primary")
            }
            "output" => after.steps[0].expected_paths.pop().map(|_| ()).unwrap(),
            "model_owner" => {
                after.steps[0].instruction = "Ignore the original marker request.".into()
            }
            "host_owner" => {
                let mut decoy = step("unrelated-host", "implement", &["notes.txt"]);
                decoy.instruction = after.steps[1].instruction.clone();
                after.steps.insert(2, decoy);
            }
            "expected_result" => after.steps[2].expected_result = "fail".into(),
            "owner_order" => after.steps.swap(0, 2),
            "condition_order" => {
                after.steps[2].verify[0] = command("V2")
                    .replace("primary", "TEMP")
                    .replace("input", "primary")
                    .replace("TEMP", "input")
            }
            "other_check" => {
                after.steps[2].verify.pop();
            }
            "success_override" => {
                after.steps[2].verify[0] =
                    command("V2").replace("console.log('ok')", "process.exit(0);console.log('ok')")
            }
            "unknown_regex" => {
                before.steps[2].verify[0] =
                    command("O").replace("JSON\\.stringify", "JSON.stringify")
            }
            _ => unreachable!(),
        }
        assert!(
            case == "unknown_regex" || after != full_plan("V2"),
            "mutation ineffective: {case}"
        );
        let bytes = std::fs::read(&path).unwrap();
        let (result, client) = replay(
            &c,
            &before.goal,
            vec![
                proposal(&before),
                proposal(&after),
                proposal(&after),
                proposal(&full_plan("V2")),
            ],
        );
        audits.push(audit(case, &c, &result, &client));
        assert!(result.is_err(), "{case}: {result:?}");
        assert_eq!(client.requests.lock().unwrap().len(), 3, "{case}");
        let mut execution = Replay::default();
        let result =
            result.and_then(|p| crate::planner::runner::run_step_plan(&mut execution, &p, &c));
        assert!(result.is_err());
        assert!(execution.requests.lock().unwrap().is_empty());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert!(!root.path().join("notes.txt").exists());
        assert!(
            events(&c)
                .iter()
                .filter(|e| e["event"] == "recovery_verifier_plan_admission")
                .all(|e| e["recovery_budget_changed"] == false)
        );
        assert!(
            !events(&c)
                .iter()
                .any(|e| e["event"] == "recovery_generated_completion_contract_completed")
        );
    }
    if let Ok(dir) = std::env::var("ISSUE488_PLANNER_EVIDENCE") {
        std::fs::write(
            Path::new(&dir).join("planner-refusals.json"),
            serde_json::to_vec_pretty(&audits).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn issue488_mock_fallback_rechecks_retained_scope_and_weak_candidate() {
    let mut audits = Vec::new();
    for setup_fallback in [false, true] {
        let (_root, c, _guard, path) = setup();
        let before = full_plan("O");
        let bytes = std::fs::read(&path).unwrap();
        let goal = if setup_fallback {
            "Phase id: setup\nPhase task: scaffold the project"
        } else {
            &before.goal
        };
        let mut client = Replay::new(vec![
            proposal(&before),
            AssistantReply::text("{invalid"),
            AssistantReply::text("{invalid"),
        ]);
        let result = crate::planner::runner::generate_step_plan(&mut client, goal, &c);
        audits.push(audit(
            if setup_fallback {
                "setup_fallback"
            } else {
                "no_last_valid_plan"
            },
            &c,
            &result,
            &client,
        ));
        assert!(
            result.is_err(),
            "setup={setup_fallback}: {result:?}\n{}",
            json!(events(&c))
        );
        assert_eq!(client.requests.lock().unwrap().len(), 3);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        if setup_fallback {
            assert!(
                events(&c)
                    .iter()
                    .any(|e| e["event"] == "recovery_verifier_plan_return_rejected")
            );
        }
        // Every finish path also applies the candidate guard, including a
        // pre-existing last-valid candidate without retained Admission state.
        assert!(
            admission::Admission::default()
                .finish(&c, None, proposal_plan("O"))
                .is_err()
        );
        let mut admission = admission::Admission::default();
        let mut original = before.clone();
        admission.strengthen(&c, &mut original);
        assert!(matches!(
            admission
                .check(&c, None, &before, &mut original, 1)
                .unwrap(),
            admission::Decision::Retry(_)
        ));
        assert!(
            admission.finish(&c, None, full_plan("V2")).is_err(),
            "fallback lacking captured host obligation"
        );
    }
    if let Ok(dir) = std::env::var("ISSUE488_PLANNER_EVIDENCE") {
        std::fs::write(
            Path::new(&dir).join("planner-fallback.json"),
            serde_json::to_vec_pretty(&audits).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn issue488_driver_last_valid_return_and_formation_retry_clearing_are_observed() {
    let mut audits = Vec::new();
    for formation_retry in [false, true] {
        let (_root, c, _guard, path) = setup();
        let bytes = std::fs::read(&path).unwrap();
        let mut first = plan(vec![step(
            "make-app",
            "implement",
            &["package.json", "src/app/page.tsx"],
        )]);
        first.goal = "Build a Next.js game app".into();
        first.steps[0].instruction =
            "Create package.json and src/app/page.tsx for the game app".into();
        first.steps[0].verify = vec![command("V2")];
        let mut next = first.clone();
        if formation_retry {
            next.steps[0].verify = vec![command("O")];
        } else {
            next.steps[0].verify = vec!["node check.js | node check2.js".into()];
        }
        let mut client = Replay::new(vec![
            proposal(&first),
            proposal(&next),
            AssistantReply::text("{invalid"),
        ]);
        let result = crate::planner::runner::generate_step_plan(&mut client, &first.goal, &c);
        let log = events(&c);
        assert_eq!(
            client.requests.lock().unwrap().len(),
            3,
            "{formation_retry}: {result:?}\n{}",
            json!(log)
        );
        assert!(
            log.iter()
                .any(|e| e["event"] == "planner_quality_retry" && e["repair_attempt"] == 1),
            "{}",
            json!(log)
        );
        audits.push(audit(
            if formation_retry {
                "last_valid_cleared_by_formation_retry"
            } else {
                "last_valid_return_after_quality_then_lint_then_schema"
            },
            &c,
            &result,
            &client,
        ));
        if formation_retry {
            assert!(
                result.is_err(),
                "weak retry must clear prior last_valid_plan"
            );
            assert!(
                log.iter()
                    .any(|e| e["event"] == "recovery_verifier_plan_admission"
                        && e["planner_attempt"] == 2
                        && e["status"] == "retry")
            );
        } else {
            let returned = result.unwrap();
            assert_eq!(returned.steps[0].id, "make-app");
            assert_eq!(returned.steps[0].verify, first.steps[0].verify);
            assert!(
                log.iter()
                    .any(|e| e["event"] == "planner_quality_retry_degraded")
            );
            assert!(
                !log.iter()
                    .any(|e| e["event"] == "recovery_verifier_plan_admission")
            );
            // Check actual returned host duties as well as model duties.
            let mut augmented = first.clone();
            admission::Admission::default().strengthen(&c, &mut augmented);
            assert_eq!(returned, augmented);
        }
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    if let Ok(dir) = std::env::var("ISSUE488_PLANNER_EVIDENCE") {
        std::fs::write(
            Path::new(&dir).join("planner-last-valid.json"),
            serde_json::to_vec_pretty(&audits).unwrap(),
        )
        .unwrap();
    }
}
