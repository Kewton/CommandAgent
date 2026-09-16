use super::*;
use crate::planner::recovery_contract_authority::inline_admission;

fn register(entry: &str, c: &Config, commands: &[String]) -> anyhow::Result<()> {
    match entry {
        "commands" => authority::register_step_plan_commands(c, commands),
        "handoff" => authority::bind_for_recovery(c, commands).map(|_| ()),
        "plan" => {
            let mut p = proposal_plan("V2");
            p.steps[0].verify = commands.to_vec();
            let producer = step("make-verifier", "implement", &[SCRIPT]);
            p.steps.insert(0, producer);
            scope::register(c, &p)
        }
        _ => unreachable!(),
    }
}

#[test]
fn issue488_each_registration_entry_is_atomic_in_both_orders_and_accepts_good_batch() {
    for entry in ["commands", "plan", "handoff"] {
        for bad in [Some("O"), Some("V1"), Some("SWALLOW"), None] {
            for reverse in [false, true] {
                let (_root, c, _guard, path) = setup();
                let bytes = std::fs::read(&path).unwrap();
                let owners = scope::generated(&c);
                let mut commands = vec![command("V2"), format!("node {SCRIPT}")];
                if let Some(bad) = bad {
                    commands.push(command(bad));
                }
                if reverse {
                    commands.reverse();
                }
                let result = register(entry, &c, &commands);
                assert_eq!(
                    result.is_ok(),
                    bad.is_none(),
                    "{entry}/{bad:?}/{reverse}: {result:?}"
                );
                if bad.is_some() {
                    assert_eq!(std::fs::read(&path).unwrap(), bytes);
                    assert_eq!(scope::generated(&c), owners);
                    assert!(
                        !events(&c)
                            .iter()
                            .any(|e| e["event"]
                                == "recovery_generated_completion_contract_completed")
                    );
                    let stored: CompletionContract = serde_json::from_slice(&bytes).unwrap();
                    assert!(stored.verify_commands.is_empty());
                } else {
                    let stored: CompletionContract =
                        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                    for command in &commands {
                        assert!(stored.verify_commands.contains(command));
                    }
                    assert_eq!(scope::generated(&c).len(), usize::from(entry == "plan"));
                    assert!(
                        events(&c)
                            .iter()
                            .any(|e| e["event"]
                                == "recovery_generated_completion_contract_completed")
                    );
                }
            }
        }
    }
}

#[test]
fn issue488_authority_and_requirement_controls_preserve_existing_broad_formation() {
    for control in [
        "eligible",
        "configured",
        "unowned",
        "already_registered",
        "config_profile",
        "contract_profile",
        "fix",
        "investigate",
        "empty_requirements",
        "unknown_obligation",
        "normalized_obligation",
        "capability_only",
    ] {
        let (_root, mut c, guard, path) = setup();
        let mut guard = Some(guard);
        let mut contract: CompletionContract =
            serde_json::from_str(&fixture("contract.json")).unwrap();
        match control {
            "configured" => c.completion_contract_path = Some(path.clone()),
            "unowned" => drop(guard.take()),
            "already_registered" => contract.verify_commands = vec![command("O")],
            "config_profile" => c.profile = "generic".into(),
            "contract_profile" => contract.profile = Some("generic".into()),
            "fix" => c.intent_override = Some(crate::config::IntentId::Fix),
            "investigate" => c.intent_override = Some(crate::config::IntentId::Investigate),
            "empty_requirements" => contract.required_evidence.clear(),
            "unknown_obligation" => {
                contract.required_evidence.clear();
                contract.required_obligations = vec!["unknown".into()];
            }
            "normalized_obligation" => {
                contract.required_evidence.clear();
                contract.required_obligations = vec![" ACCEPTANCE-EVIDENCE ".into()];
            }
            "capability_only" => {
                contract.required_evidence.clear();
                contract.required_capabilities = vec!["input_output_contract".into()];
            }
            _ => {}
        }
        std::fs::write(&path, serde_json::to_vec(&contract).unwrap()).unwrap();
        let refused = inline_admission::refusal(&c, &contract, &command("O"))
            .unwrap()
            .is_some();
        assert_eq!(
            refused,
            matches!(
                control,
                "eligible" | "normalized_obligation" | "capability_only"
            ),
            "{control}"
        );
        if control != "unknown_obligation" {
            let bytes = std::fs::read(&path).unwrap();
            let result = authority::register_step_plan_commands(&c, &[command("O")]);
            assert_eq!(result.is_err(), refused, "{control}: {result:?}");
            if matches!(control, "configured" | "unowned" | "already_registered") {
                assert_eq!(std::fs::read(&path).unwrap(), bytes);
            }
        }
        if control == "unknown_obligation" {
            assert!(
                admission::Admission::default()
                    .check(&c, None, &proposal_plan("O"), &mut proposal_plan("O"), 1)
                    .is_err()
            );
            continue;
        }
        // The older, broader #479 formation rule is deliberately retained.
        let original = proposal_plan("O");
        let decision = admission::Admission::default()
            .check(&c, None, &original, &mut original.clone(), 1)
            .unwrap();
        assert_eq!(
            matches!(decision, admission::Decision::Retry(_)),
            control != "configured",
            "{control}"
        );
    }
}

#[test]
fn issue488_policy_normalization_generated_hooks_and_closed_contracts_stay_separate() {
    let (_root, c, _guard, path) = setup();
    let before = std::fs::read(&path).unwrap();
    for command in [
        format!("{} && {}", command("V2"), command("O")),
        format!("{} || true", command("O")),
        "cargo test --manifest-path ../Cargo.toml".into(),
    ] {
        assert!(
            authority::register_step_plan_commands(&c, std::slice::from_ref(&command)).is_err(),
            "{command}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    // Baseline allows this command spelling at registration. The new rule
    // requires literal argv and does not reinterpret shell expansion as proof.
    let nonliteral = r#"node -e "$UNTRUSTED_SOURCE""#;
    let contract = authority::load_for_handoff(&c).unwrap().unwrap();
    assert!(command_diagnosis::source_independent_inline_failure(&contract, nonliteral).is_none());
    assert_eq!(
        crate::planner::verify::normalize_verify_command(nonliteral)
            .unwrap()
            .as_str(),
        nonliteral
    );
    let compound = "test -f README.md && npm run build".to_string();
    authority::register_step_plan_commands(&c, &[compound]).unwrap();
    let contract = authority::load_for_handoff(&c).unwrap().unwrap();
    assert_eq!(contract.verify_commands, ["npm run build"]);
    let bytes = std::fs::read(&path).unwrap();
    let closed = authority::bind_for_recovery(&c, &[command("O")]).unwrap();
    authority::register_step_plan_commands(&closed, &[command("O")]).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    for marker in [
        "data-anvil-action=\"primary\"",
        "data-anvil-action=\"input\"",
        "data-anvil-state",
    ] {
        let command = crate::planner::verify::normalize_verify_command(&format!(
            "grep -q '{marker}' src/app/page.tsx"
        ))
        .unwrap();
        assert!(
            command_diagnosis::source_independent_inline_failure(&contract, command.as_str())
                .is_none()
        );
    }
}

#[test]
fn issue488_contract_identity_is_checked_before_fixed_evidence_diagnosis() {
    for name in ["O", "V2"] {
        let (_root, mut c, _guard, path) = setup();
        // Attach a pre-existing frozen contract to exercise independent identity
        // protection. The new preclosure guard must not reopen this contract.
        let mut contract: CompletionContract =
            serde_json::from_str(&fixture("contract.json")).unwrap();
        contract.verify_commands = vec![command(name)];
        std::fs::write(&path, serde_json::to_vec(&contract).unwrap()).unwrap();
        c.completion_contract_path = Some(path.clone());
        bind_context(&c);
        assert!(crate::planner::recovery_inspection::has_origin(&c).unwrap());
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.push(b' '); // Same semantics, changed frozen bytes.
        std::fs::write(&path, &bytes).unwrap();
        let original = proposal_plan(name);
        let error = admission::Admission::default()
            .check(
                &c,
                Some("repair-markers"),
                &original,
                &mut original.clone(),
                1,
            )
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("contract changed"),
            "{name}: {error:#}"
        );
        assert!(!error.to_string().contains("weak inline"));
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
