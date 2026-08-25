use commandagent::verification_spec::create_shadow::{
    CreateGoldBinding, CreateGoldClaim, OracleExecutionEvidence, evaluate_create_shadow,
};
use commandagent::verification_spec::{
    EvidenceStrength, ExpectedPolarity, OracleInput, OracleObservation, OracleResult,
    OracleStrategy, ShadowFailureKind, VerificationIntent, parse_provider_spec, provider_failure,
};
use serde_json::{Value, json};

const NEXTJS: &str = include_str!("fixtures/verification_spec_v0/create-nextjs-shadow.json");
const NEXTJS_COVERAGE: &str =
    include_str!("fixtures/verification_spec_v0/create-nextjs-shadow-coverage.json");
const PYTHON_CLI: &str =
    include_str!("fixtures/verification_spec_v0/create-python-cli-shadow.json");
const PYTHON_CLI_COVERAGE: &str =
    include_str!("fixtures/verification_spec_v0/create-python-cli-shadow-coverage.json");

#[test]
fn nextjs_gold_matrix_covers_copy_style_interaction_and_port_path() {
    let raw: Value = serde_json::from_str(NEXTJS).unwrap();
    let generation = generated(&raw);
    let evidence = [
        evidence("click-twice", EvidenceStrength::Runtime),
        evidence("heading-copy", EvidenceStrength::Runtime),
        evidence("computed-background", EvidenceStrength::Deterministic),
        evidence("play-http", EvidenceStrength::Runtime),
    ];
    let report = evaluate_create_shadow(&nextjs_gold(), &generation, &evidence);
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        serde_json::from_str::<Value>(NEXTJS_COVERAGE).unwrap()
    );
}

#[test]
fn python_cli_gold_matrix_requires_both_known_aggregation_inputs() {
    let raw: Value = serde_json::from_str(PYTHON_CLI).unwrap();
    let generation = generated(&raw);
    let evidence = [
        evidence("sum-positive", EvidenceStrength::Runtime),
        evidence("sum-zero", EvidenceStrength::Runtime),
    ];
    let report = evaluate_create_shadow(&python_cli_gold(), &generation, &evidence);
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        serde_json::from_str::<Value>(PYTHON_CLI_COVERAGE).unwrap()
    );

    let mut missing_row = raw;
    missing_row["claims"][0]["oracle_ids"] = json!(["sum-positive"]);
    missing_row["oracles"].as_array_mut().unwrap().remove(1);
    let report =
        evaluate_create_shadow(&python_cli_gold(), &generated(&missing_row), &evidence[..1]);
    assert!(!report.all_required_passed);
    assert_eq!(
        report.claims[0].unverified_reason.as_deref(),
        Some("binding_missing:1")
    );
}

#[test]
fn build_only_oracle_is_weaker_than_the_known_interaction_expectation() {
    let mut raw: Value = serde_json::from_str(NEXTJS).unwrap();
    raw["claims"] = json!([raw["claims"][0].clone()]);
    raw["claims"][0]["oracle_ids"] = json!(["build-only"]);
    let mut oracle = raw["oracles"][0].clone();
    oracle["id"] = json!("build-only");
    oracle["strategy"] = json!("command");
    oracle["setup"]["argv"] = json!(["npm", "run", "build"]);
    oracle["input"] = json!({"kind":"none"});
    oracle["observation"] = json!({"kind":"exit_code","expected":0});
    raw["oracles"] = json!([oracle]);

    let report = evaluate_create_shadow(
        &[nextjs_gold().remove(0)],
        &generated(&raw),
        &[evidence("build-only", EvidenceStrength::Runtime)],
    );
    assert!(!report.all_required_passed);
    assert_eq!(
        report.claims[0].unverified_reason.as_deref(),
        Some("binding_missing:0")
    );
}

#[test]
fn model_declared_pass_cannot_replace_external_execution_evidence() {
    let mut raw: Value = serde_json::from_str(PYTHON_CLI).unwrap();
    for oracle in raw["oracles"].as_array_mut().unwrap() {
        oracle["lifecycle"] = json!("executed");
        oracle["result"] = json!("pass");
        oracle["observed_strength"] = json!("runtime");
    }
    let report = evaluate_create_shadow(&python_cli_gold(), &generated(&raw), &[]);
    assert!(!report.all_required_passed);
    assert!(!report.claims[0].executed);
    assert_eq!(
        report.claims[0].unverified_reason.as_deref(),
        Some("execution_evidence_missing:sum-positive")
    );
}

#[test]
fn install_dev_server_and_free_form_shell_remain_outside_verify_policy() {
    for argv in [
        json!(["npm", "install"]),
        json!(["npm", "run", "dev"]),
        json!(["sh", "-c", "printf 5"]),
    ] {
        let mut raw: Value = serde_json::from_str(PYTHON_CLI).unwrap();
        raw["claims"][0]["oracle_ids"] = json!(["sum-positive"]);
        raw["oracles"].as_array_mut().unwrap().truncate(1);
        raw["oracles"][0]["setup"]["argv"] = argv;
        let report = evaluate_create_shadow(
            &[python_cli_gold().remove(0)],
            &generated(&raw),
            &[evidence("sum-positive", EvidenceStrength::Runtime)],
        );
        assert!(!report.all_required_passed);
        assert!(
            report.claims[0]
                .unverified_reason
                .as_deref()
                .is_some_and(|reason| reason.starts_with("policy_rejected:")),
            "report={report:?}"
        );
    }
}

#[test]
fn provider_failure_unsupported_negative_and_unsafe_evidence_are_unverified() {
    let rejected = provider_failure(ShadowFailureKind::ProviderUnavailable, "offline");
    let report = evaluate_create_shadow(&python_cli_gold(), &rejected, &[]);
    assert!(!report.all_required_passed);
    assert!(
        report.claims[0]
            .unverified_reason
            .as_deref()
            .is_some_and(|reason| reason.starts_with("generation_rejected:"))
    );

    let negative = CreateGoldClaim {
        id: "no-network".to_string(),
        required: true,
        minimum_strength: EvidenceStrength::Deterministic,
        bindings: Vec::new(),
        unsupported_reason: Some("network_log_oracle_unavailable".to_string()),
    };
    let report = evaluate_create_shadow(&[negative], &generated_json(PYTHON_CLI), &[]);
    assert_eq!(
        report.claims[0].unverified_reason.as_deref(),
        Some("unsupported:network_log_oracle_unavailable")
    );

    let evidence = [
        OracleExecutionEvidence {
            evidence_path: "../escape.json".to_string(),
            ..evidence("sum-positive", EvidenceStrength::Runtime)
        },
        evidence("sum-zero", EvidenceStrength::Runtime),
    ];
    let report = evaluate_create_shadow(&python_cli_gold(), &generated_json(PYTHON_CLI), &evidence);
    assert_eq!(
        report.claims[0].unverified_reason.as_deref(),
        Some("evidence_path_unsafe:sum-positive")
    );
}

fn generated(raw: &Value) -> commandagent::verification_spec::ShadowGeneration {
    let goal = raw["goal"].as_str().unwrap();
    commandagent::verification_spec::ShadowGeneration::Generated(Box::new(
        parse_provider_spec(goal, VerificationIntent::Create, &raw.to_string()).unwrap(),
    ))
}

fn generated_json(raw: &str) -> commandagent::verification_spec::ShadowGeneration {
    generated(&serde_json::from_str(raw).unwrap())
}

fn evidence(id: &str, strength: EvidenceStrength) -> OracleExecutionEvidence {
    OracleExecutionEvidence {
        oracle_id: id.to_string(),
        observed_strength: strength,
        outcome: OracleResult::Pass,
        evidence_path: format!("evidence/{id}.json"),
    }
}

fn nextjs_gold() -> Vec<CreateGoldClaim> {
    vec![
        CreateGoldClaim {
            id: "counter-two-clicks".to_string(),
            required: true,
            minimum_strength: EvidenceStrength::Runtime,
            bindings: vec![CreateGoldBinding {
                accepted_strategies: vec![OracleStrategy::Interaction],
                expected_polarity: ExpectedPolarity::Success,
                input: OracleInput::Dom {
                    route: "/play".to_string(),
                    selector: "button".to_string(),
                },
                observation: OracleObservation::Interaction {
                    expected: "two clicks change 0 to 2".to_string(),
                },
            }],
            unsupported_reason: None,
        },
        CreateGoldClaim {
            id: "ui-copy-style".to_string(),
            required: true,
            minimum_strength: EvidenceStrength::Deterministic,
            bindings: vec![
                CreateGoldBinding {
                    accepted_strategies: vec![OracleStrategy::Dom],
                    expected_polarity: ExpectedPolarity::Success,
                    input: OracleInput::Dom {
                        route: "/play".to_string(),
                        selector: "h1".to_string(),
                    },
                    observation: OracleObservation::Dom {
                        expected: "Start".to_string(),
                    },
                },
                CreateGoldBinding {
                    accepted_strategies: vec![OracleStrategy::Dom],
                    expected_polarity: ExpectedPolarity::Success,
                    input: OracleInput::Dom {
                        route: "/play".to_string(),
                        selector: "main".to_string(),
                    },
                    observation: OracleObservation::Dom {
                        expected: "rgb(0, 0, 255)".to_string(),
                    },
                },
            ],
            unsupported_reason: None,
        },
        CreateGoldClaim {
            id: "port-path".to_string(),
            required: true,
            minimum_strength: EvidenceStrength::Runtime,
            bindings: vec![CreateGoldBinding {
                accepted_strategies: vec![OracleStrategy::Http],
                expected_polarity: ExpectedPolarity::Success,
                input: OracleInput::Http {
                    method: "GET".to_string(),
                    port: 4173,
                    path: "/play".to_string(),
                },
                observation: OracleObservation::HttpStatus { expected: 200 },
            }],
            unsupported_reason: None,
        },
    ]
}

fn python_cli_gold() -> Vec<CreateGoldClaim> {
    vec![CreateGoldClaim {
        id: "cli-known-values".to_string(),
        required: true,
        minimum_strength: EvidenceStrength::Runtime,
        bindings: vec![
            CreateGoldBinding {
                accepted_strategies: vec![OracleStrategy::Stdout],
                expected_polarity: ExpectedPolarity::Success,
                input: OracleInput::Text {
                    value: "2 3".to_string(),
                },
                observation: OracleObservation::Stdout {
                    expected: "5".to_string(),
                },
            },
            CreateGoldBinding {
                accepted_strategies: vec![OracleStrategy::Stdout],
                expected_polarity: ExpectedPolarity::Success,
                input: OracleInput::Text {
                    value: "-1 1".to_string(),
                },
                observation: OracleObservation::Stdout {
                    expected: "0".to_string(),
                },
            },
        ],
        unsupported_reason: None,
    }]
}
