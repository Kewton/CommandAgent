use commandagent::verification_spec::critic::{
    CRITIC_PROMPT_VERSION, CRITIC_SCHEMA_VERSION, CounterfactualEvidence, CriticDecision,
    CriticGeneration, CriticJudgment, CriticLineage, CriticResourceBudget, CriticResourceUsage,
    CriticValidationStatus, LineageStage, OracleContract, RESOURCE_BUDGET_VERSION, checkpoint,
    observe_critic, parse_critic_judgment, validate_critic,
};
use commandagent::verification_spec::{
    EvidenceStrength, ExpectedPolarity, OracleInput, OracleObservation, OracleSetup,
};

fn contract() -> OracleContract {
    OracleContract {
        claim_id: "parser-reproducer".to_string(),
        expected_polarity: ExpectedPolarity::Failure,
        minimum_strength: EvidenceStrength::Runtime,
        input: OracleInput::Fixture {
            path: "tests/fixtures/parser.txt".to_string(),
            sha256: "a".repeat(64),
        },
        observation: OracleObservation::Stderr {
            expected: "invalid token".to_string(),
        },
        setup: OracleSetup {
            argv: vec![
                "cargo".to_string(),
                "test".to_string(),
                "parser".to_string(),
            ],
            cwd: ".".to_string(),
            fixture_paths: vec!["tests/fixtures/parser.txt".to_string()],
        },
    }
}

fn lineage() -> CriticLineage {
    let frozen = contract();
    let mut bound = frozen.clone();
    bound.setup.argv.push("--exact".to_string());
    bound.setup.cwd = "./".to_string();
    CriticLineage {
        freeze: checkpoint(
            LineageStage::Freeze,
            &frozen,
            1,
            "run-398",
            "critic-model",
            "req-1",
        ),
        bind: checkpoint(
            LineageStage::Bind,
            &bound,
            2,
            "run-398",
            "critic-model",
            "req-1",
        ),
        execute: checkpoint(
            LineageStage::Execute,
            &bound,
            3,
            "run-398",
            "critic-model",
            "req-1",
        ),
        frozen,
        bound,
        semantic_equivalence: true,
        concretization_reason: Some("argv spelling concretized for the selected test".to_string()),
    }
}

fn judgment(decision: CriticDecision) -> CriticJudgment {
    CriticJudgment {
        schema_version: CRITIC_SCHEMA_VERSION.to_string(),
        prompt_version: CRITIC_PROMPT_VERSION.to_string(),
        run_id: "run-398".to_string(),
        model: "critic-model".to_string(),
        request_id: "req-1".to_string(),
        decision,
        issue_codes: if decision == CriticDecision::Reject {
            vec!["binding_weakened".to_string()]
        } else {
            Vec::new()
        },
        rationale: "typed contract and counterfactual checked".to_string(),
    }
}

fn budget() -> CriticResourceBudget {
    CriticResourceBudget {
        budget_version: RESOURCE_BUDGET_VERSION.to_string(),
        max_total_tokens: 1_024,
        max_latency_ms: 5_000,
        max_retries: 1,
    }
}

fn usage() -> CriticResourceUsage {
    CriticResourceUsage {
        total_tokens: 700,
        latency_ms: 2_500,
        retries: 0,
    }
}

fn counterfactual(lineage: &CriticLineage) -> CounterfactualEvidence {
    CounterfactualEvidence::Generated {
        frozen_contract_sha256: lineage.freeze.artifact_sha256.clone(),
        executed: true,
        discriminated: true,
        evidence_path: "evidence/critic-counterfactual.json".to_string(),
    }
}

fn evaluate(lineage: &CriticLineage) -> commandagent::verification_spec::critic::CriticValidation {
    validate_critic(
        &CriticGeneration::Generated(judgment(CriticDecision::Accept)),
        lineage,
        &counterfactual(lineage),
        &budget(),
        &usage(),
    )
}

#[test]
fn schema_parsing_and_runtime_validation_have_separate_responsibilities() {
    let raw = serde_json::to_string(&judgment(CriticDecision::Accept)).unwrap();
    let parsed = parse_critic_judgment(&raw).unwrap();
    assert_eq!(parsed.decision, CriticDecision::Accept);

    let mut weakened = lineage();
    weakened.bound.minimum_strength = EvidenceStrength::Weak;
    weakened.bind = checkpoint(
        LineageStage::Bind,
        &weakened.bound,
        2,
        "run-398",
        "critic-model",
        "req-1",
    );
    weakened.execute = checkpoint(
        LineageStage::Execute,
        &weakened.bound,
        3,
        "run-398",
        "critic-model",
        "req-1",
    );
    let report = evaluate(&weakened);
    assert_eq!(report.status, CriticValidationStatus::Rejected);
    assert!(
        report
            .reasons
            .contains(&"minimum_strength_weakened".to_string())
    );
}

#[test]
fn semantic_equivalent_argv_concretization_preserves_full_lineage() {
    let lineage = lineage();
    let report = evaluate(&lineage);
    assert_eq!(report.status, CriticValidationStatus::Verified);
    assert!(report.reasons.is_empty());
    assert_eq!(lineage.freeze.stage, LineageStage::Freeze);
    assert_eq!(lineage.bind.stage, LineageStage::Bind);
    assert_eq!(lineage.execute.stage, LineageStage::Execute);
    assert!(lineage.freeze.epoch < lineage.bind.epoch);
    assert!(lineage.bind.epoch < lineage.execute.epoch);
    assert_eq!(lineage.freeze.run_id, "run-398");
    assert_eq!(lineage.freeze.model, "critic-model");
    assert_eq!(lineage.freeze.prompt_version, CRITIC_PROMPT_VERSION);
    assert_eq!(lineage.freeze.schema_version, CRITIC_SCHEMA_VERSION);
}

#[test]
fn stronger_evidence_requirement_is_not_treated_as_weakening() {
    let mut lineage = lineage();
    lineage.frozen.minimum_strength = EvidenceStrength::Deterministic;
    lineage.freeze = checkpoint(
        LineageStage::Freeze,
        &lineage.frozen,
        1,
        "run-398",
        "critic-model",
        "req-1",
    );
    let counterfactual = counterfactual(&lineage);
    let report = validate_critic(
        &CriticGeneration::Generated(judgment(CriticDecision::Accept)),
        &lineage,
        &counterfactual,
        &budget(),
        &usage(),
    );
    assert_eq!(report.status, CriticValidationStatus::Verified);
}

#[test]
fn semantic_mutations_are_rejected_for_every_strength_and_polarity_variant() {
    let mut cases = Vec::new();
    for polarity in [
        ExpectedPolarity::Success,
        ExpectedPolarity::Present,
        ExpectedPolarity::Absent,
    ] {
        let mut changed = lineage();
        changed.bound.expected_polarity = polarity;
        cases.push(changed);
    }
    for strength in [EvidenceStrength::Weak, EvidenceStrength::Deterministic] {
        let mut changed = lineage();
        changed.bound.minimum_strength = strength;
        cases.push(changed);
    }
    let mut expected = lineage();
    expected.bound.observation = OracleObservation::Stderr {
        expected: "some easier error".to_string(),
    };
    cases.push(expected);

    for mut changed in cases {
        changed.bind = checkpoint(
            LineageStage::Bind,
            &changed.bound,
            2,
            "run-398",
            "critic-model",
            "req-1",
        );
        changed.execute = checkpoint(
            LineageStage::Execute,
            &changed.bound,
            3,
            "run-398",
            "critic-model",
            "req-1",
        );
        assert_eq!(evaluate(&changed).status, CriticValidationStatus::Rejected);
    }
}

#[test]
fn absent_or_unavailable_counterfactual_is_reasoned_unverified() {
    let lineage = lineage();
    for counterfactual in [
        CounterfactualEvidence::Absent {
            reason: "no safe inverse fixture".to_string(),
        },
        CounterfactualEvidence::Unavailable {
            reason: "dependency unavailable".to_string(),
        },
    ] {
        let report = validate_critic(
            &CriticGeneration::Generated(judgment(CriticDecision::Accept)),
            &lineage,
            &counterfactual,
            &budget(),
            &usage(),
        );
        assert_eq!(report.status, CriticValidationStatus::Unverified);
        assert!(
            report
                .reasons
                .iter()
                .any(|reason| reason.contains("counterfactual"))
        );
    }
}

#[test]
fn provider_failure_and_budget_breach_preserve_authority() {
    let lineage = lineage();
    let authoritative = "existing-full";
    let observation = observe_critic(
        &authoritative,
        &CriticGeneration::Unavailable {
            reason: "provider offline".to_string(),
        },
        &lineage,
        &counterfactual(&lineage),
        &budget(),
        &CriticResourceUsage {
            total_tokens: 1_025,
            latency_ms: 5_001,
            retries: 2,
        },
    );
    assert_eq!(observation.authoritative, authoritative);
    assert_eq!(
        observation.validation.status,
        CriticValidationStatus::Unverified
    );
    assert!(observation.validation.shadow_only);
    assert!(!observation.validation.authoritative_verdict_changed);
    assert!(!observation.validation.candidate_execution_authorized);
    assert!(
        observation
            .validation
            .reasons
            .iter()
            .any(|reason| reason == "critic_token_budget_exceeded")
    );
    assert!(
        observation
            .validation
            .reasons
            .iter()
            .any(|reason| reason == "critic_latency_budget_exceeded")
    );
    assert!(
        observation
            .validation
            .reasons
            .iter()
            .any(|reason| reason == "critic_retry_budget_exceeded")
    );
}

#[test]
fn adversarial_corpus_has_zero_false_full() {
    let base = lineage();
    let mut attacks = Vec::new();

    let mut polarity = base.clone();
    polarity.bound.expected_polarity = ExpectedPolarity::Success;
    attacks.push((polarity, counterfactual(&base), budget(), usage()));

    let mut observation = base.clone();
    observation.bound.observation = OracleObservation::Stderr {
        expected: "any error".to_string(),
    };
    attacks.push((observation, counterfactual(&base), budget(), usage()));

    let mut stale = base.clone();
    stale.execute.epoch = stale.bind.epoch;
    attacks.push((stale, counterfactual(&base), budget(), usage()));

    attacks.push((
        base.clone(),
        CounterfactualEvidence::Absent {
            reason: "not constructible".to_string(),
        },
        budget(),
        usage(),
    ));

    let mut over_budget = usage();
    over_budget.total_tokens = budget().max_total_tokens + 1;
    attacks.push((base.clone(), counterfactual(&base), budget(), over_budget));

    let false_full = attacks
        .into_iter()
        .filter(|(lineage, counterfactual, budget, usage)| {
            validate_critic(
                &CriticGeneration::Generated(judgment(CriticDecision::Accept)),
                lineage,
                counterfactual,
                budget,
                usage,
            )
            .status
                == CriticValidationStatus::Verified
        })
        .count();
    assert_eq!(false_full, 0);
}

#[test]
fn critic_schema_rejects_unknown_fields_and_unreasoned_rejection() {
    let mut value = serde_json::to_value(judgment(CriticDecision::Reject)).unwrap();
    value["unexpected"] = serde_json::json!(true);
    assert!(
        parse_critic_judgment(&value.to_string())
            .unwrap_err()
            .starts_with("critic_schema_invalid:")
    );

    let mut judgment = judgment(CriticDecision::Reject);
    judgment.issue_codes.clear();
    assert_eq!(
        parse_critic_judgment(&serde_json::to_string(&judgment).unwrap()).unwrap_err(),
        "critic_reject_reason_missing"
    );

    let lineage = lineage();
    let report = validate_critic(
        &CriticGeneration::Generated(judgment),
        &lineage,
        &counterfactual(&lineage),
        &budget(),
        &usage(),
    );
    assert_eq!(report.status, CriticValidationStatus::Rejected);
}
