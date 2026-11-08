#[test]
#[cfg(unix)]
fn compile_rollback_rejects_a_noncompiling_snapshot_and_accounts_build_time() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join(".anvil/runs/broken-rollback/events.jsonl");
    write_static_compile_repair_workspace(dir.path(), static_broken_page_source());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    snapshot_last_known_good_sources(
        &cfg,
        "test",
        Some("inspection-only"),
        "nextjs",
        "Create a static Next.js page",
        &["src/app/page.tsx".to_string()],
    );
    let mut report = VerificationReport::pass();
    report.push_compile_errors(
        "npm run build",
        vec![CompileError {
            path: "src/app/page.tsx".to_string(),
            line: 12,
            column: 1,
            message: "Expected ';', '}' or <eof>".to_string(),
            excerpt: "12 | BROKEN_SYNTAX\n   | ^".to_string(),
            symbol: None,
            route_bound: Some(true),
        }],
    );

    let rollback = try_compile_rollback_after_repair_exhaustion(
        &cfg,
        "nextjs",
        "Create a static Next.js page",
        "core-implementation",
        "Run the production build.",
        &report,
        "bounded_repair_exhausted",
    )
    .unwrap();

    assert!(rollback.is_none());
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"event\":\"compile_rollback_failed\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"build_reverified\":false"),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"build_reverification_status\":\"failed\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"build_duration_ms\":"),
        "{event_text}"
    );
    assert!(
        !event_text.contains("\"event\":\"compile_rollback_applied\""),
        "{event_text}"
    );
    assert!(
        crate::time_profile::aggregate_event_path(Some(&events)).builds_ms > 0,
        "{event_text}"
    );
}

/// A client whose every call is first one send of the run's wallet, as a provider's is, and goes
/// no further than a script. A refused call gives the refusal back, and the first notes how many
/// events were written and how many sends had gone by then.
#[cfg(unix)]
#[derive(Clone)]
struct WalletClient976 {
    wallet: Arc<crate::structured_budget::test_support::TestWallet>,
    inner: FakeClient,
    events: PathBuf,
    refused_at: Arc<Mutex<Option<(usize, u64)>>>,
}

#[cfg(unix)]
impl ChatClient for WalletClient976 {
    fn label(&self) -> &str {
        "wallet-fake-976"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let marker = crate::provider_call::current_final_repair();
        if let Err(refused) = self.wallet.send(50, marker.as_ref()) {
            let mut at = self.refused_at.lock().unwrap();
            if at.is_none() {
                let written = std::fs::read_to_string(&self.events)
                    .unwrap_or_default()
                    .lines()
                    .count();
                *at = Some((written, self.wallet.sends()));
            }
            return Err(crate::structured_budget::transport::refused(refused));
        }
        self.inner.chat(model, messages, tools, native_tools_enabled)
    }
}

/// Issue #976 (G5, N13): the compile regeneration's send is refused by the wallet, and the
/// regeneration takes the refusal as a turn that changed nothing. The rollback then puts back a
/// snapshot that builds. The command is still no success: no report is made after the refusal, no
/// send goes and no repair starts after it, and no completion is said.
#[test]
#[cfg(unix)]
fn a_compile_regeneration_the_wallet_refused_leaves_the_run_no_success_after_rollback() {
    use crate::structured_budget::ledger::LedgerFailure;
    use crate::structured_budget::outcome::{CommandRefusal, refusal};
    use crate::structured_budget::reservation::BudgetRefusal;
    use crate::structured_budget::transport::SendRefusal;

    // Two planner sends and six execution sends go before the compile regeneration's.
    let wallet = Arc::new(crate::structured_budget::test_support::TestWallet::open(
        |value| value["limits"]["physical_sends"] = serde_json::json!(8),
    ));
    let root = wallet.workspace.clone();
    let events = root.join(".anvil/runs/refused-regeneration/events.jsonl");
    write_static_compile_repair_workspace(&root, static_good_page_source());
    let mut cfg = config(root.clone());
    cfg.profile = "generic".to_string();
    cfg.eval_events_path = Some(events.clone());
    cfg.completion_contract_path = Some(write_static_build_contract(&root));
    cfg.field_sources.run_budget = Some(wallet.handle.clone());
    let plan = static_compile_repair_plan();
    let refused_at = Arc::new(Mutex::new(None));
    let client = |replies| WalletClient976 {
        wallet: wallet.clone(),
        inner: FakeClient::new(replies),
        events: events.clone(),
        refused_at: refused_at.clone(),
    };
    let mut planner = client(vec![
        AssistantReply::text(static_phase_step_plan_json(true)),
        AssistantReply::text(static_phase_step_plan_json(false)),
    ]);
    let mut execution = client(vec![
        read_static_page_reply(),
        write_static_page_reply(static_broken_page_source()),
        AssistantReply::text("The compile error is in src/app/page.tsx."),
        AssistantReply::text("It needs a syntax fix."),
        AssistantReply::text("The compile error remains in src/app/page.tsx."),
        AssistantReply::text("It still needs a syntax fix."),
        write_static_page_reply(static_good_page_source()),
    ]);

    let result = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg);

    let event_text = std::fs::read_to_string(&events).unwrap();
    let completions = event_text
        .matches("\"event\":\"ultra_plan_complete\"")
        .count();
    let err = match result {
        Ok(done) => panic!(
            "the refused command was called a success: {done}; ultra_plan_complete x{completions}"
        ),
        Err(err) => err,
    };
    assert_eq!(completions, 0, "{event_text}");
    let want = CommandRefusal::SendRefused(SendRefusal::Wallet(LedgerFailure::Budget(
        BudgetRefusal::PhysicalExceeded,
    )));
    assert_eq!(refusal(&cfg), Some(want));
    assert!(format!("{err:#}").contains(&want.to_string()), "{err:#}");

    let (written, sent) = refused_at
        .lock()
        .unwrap()
        .expect("the compile regeneration's send was refused");
    assert_eq!(sent, 8, "every send before the regeneration's went");
    assert_eq!(wallet.sends(), sent, "H4: no send after the refusal");
    let lines: Vec<&str> = event_text.lines().collect();
    let (before, after) = lines.split_at(written);
    let has = |lines: &[&str], event: &str| {
        lines
            .iter()
            .any(|line| line.contains(&format!("\"event\":\"{event}\"")))
    };
    assert!(
        has(before, "final_acceptance_deterministic_remedies"),
        "the report was made before: {event_text}"
    );
    assert!(
        has(after, "compile_rollback_applied"),
        "the rollback put a building snapshot back: {event_text}"
    );
    for event in [
        "final_acceptance_repair_start",
        "final_acceptance_repair_complete",
        "final_acceptance_repair_failed",
        "final_acceptance_repair_no_source_change",
        "final_acceptance_repair_exhausted",
    ] {
        assert!(!has(after, event), "H4: {event} after the refusal: {event_text}");
    }
    let repair_stage: Vec<serde_json::Value> = after
        .iter()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|event| event["lifecycle_stage"] == "final_acceptance_repair")
        .collect();
    assert_eq!(
        repair_stage.len(),
        1,
        "H4: only the refused regeneration's record: {repair_stage:?}"
    );
    let record = &repair_stage[0];
    assert_eq!(record["event"], "repair_regeneration", "{record}");
    assert_eq!(record["fired"], true, "{record}");
    assert_eq!(record["accepted"], false, "{record}");
    assert!(
        record["reason"]
            .as_str()
            .is_some_and(|reason| reason.starts_with("regeneration_turn_error:")),
        "{record}"
    );
    assert!(
        !has(after, "final_acceptance_deterministic_remedies"),
        "no report is made after the refusal: {event_text}"
    );
}

/// Issue #976 (G5, R1): a workspace whose declared dependencies differ from the ones last
/// reconciled, under a wallet whose command a send of 99 tokens refused when `refused`. The run is
/// offline, so a reconciliation stops before any package manager runs.
fn r1_bench_976(
    refused: bool,
) -> (
    crate::structured_budget::test_support::TestWallet,
    crate::config::Config,
    PathBuf,
) {
    let wallet = crate::structured_budget::test_support::TestWallet::open(|value| {
        value["max_output_tokens"] = serde_json::json!(99);
        value["limits"]["cost_usd"] = serde_json::json!("0.001");
    });
    if refused {
        wallet.send(99, None).unwrap_err();
    }
    let workspace = wallet.workspace.clone();
    let parsed = crate::provider_cli::parse_from([
        "commandagent",
        "--cwd",
        workspace.to_str().unwrap(),
        "--state-dir",
        workspace.parent().unwrap().join("state").to_str().unwrap(),
    ])
    .unwrap();
    let mut cfg =
        crate::config::Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options)
            .unwrap();
    cfg.field_sources.run_budget = Some(Arc::clone(&wallet.handle));
    cfg.offline = true;
    cfg.profile = crate::planner::profiles::nextjs::PROFILE_ID.to_string();
    let events = workspace.parent().unwrap().join("events.jsonl");
    cfg.eval_events_path = Some(events.clone());
    std::fs::write(
        workspace.join("package.json"),
        r#"{"scripts":{"build":"next build"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(workspace.join(".commandagent")).unwrap();
    std::fs::write(workspace.join(R1_FINGERPRINT_976), "stale-976").unwrap();
    assert!(dependency_setup::node_dependency_declarations_fingerprint_mismatch(&workspace));
    (wallet, cfg, events)
}

const R1_FINGERPRINT_976: &str = ".commandagent/node-dependency-declarations.fingerprint";

fn r1_report_976(cfg: &crate::config::Config) -> anyhow::Result<()> {
    let plan = UltraPlan::deterministic(
        "Create a Next.js route",
        crate::planner::profiles::nextjs::PROFILE_ID,
        "default",
        "create",
    );
    let mut setup_authority = UltraRunSetupAuthorityState::default();
    ultra_final_acceptance_report_with_deterministic_remedies(&plan, cfg, 0, &mut setup_authority)
        .map(drop)
}

fn r1_events_976(path: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// R1: with the refusal kept, the final report's deterministic remedies start nothing: the
/// wrapper returns the command's refusal before the reconciliation, with no event at all, no
/// dependency setup attempted, and the fingerprint as it was.
#[test]
fn a_refused_command_reconciles_no_dependency() {
    let (wallet, cfg, events) = r1_bench_976(true);
    let error = r1_report_976(&cfg).unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<crate::structured_budget::outcome::CommandRefusal>(),
            Some(crate::structured_budget::outcome::CommandRefusal::SendRefused(_))
        ),
        "{error:#}"
    );
    assert_eq!(r1_events_976(&events), Vec::<serde_json::Value>::new());
    assert_eq!(
        std::fs::read_to_string(wallet.workspace.join(R1_FINGERPRINT_976)).unwrap(),
        "stale-976"
    );
}

/// The control: with nothing refused, the same workspace starts the reconciliation, which the
/// offline run then stops.
#[test]
fn a_command_with_nothing_refused_reconciles_as_before() {
    let (_wallet, cfg, events) = r1_bench_976(false);
    let error = r1_report_976(&cfg).unwrap_err();
    assert!(
        format!("{error:#}").contains("dependency_setup_blocked_offline"),
        "{error:#}"
    );
    let reconciliations = r1_events_976(&events)
        .into_iter()
        .filter(|event| event["event"] == "dependency_setup_reconciliation")
        .count();
    assert_eq!(reconciliations, 1);
}
