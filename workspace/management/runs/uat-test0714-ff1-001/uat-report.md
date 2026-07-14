# FF-1 パリティ計測レポート（uat-test0714-ff1-001）

## 結論

**BLOCKED / 計測不成立。** preflight の権限付き `cargo test` が green にならなかったため、事前宣言された停止条件に従って計測を開始しなかった。6 run はすべて未実行であり、指定ワークスペースも作成していない。FF-1 の P0-a / P0-b は未判定である。

## 対象

- 実行日: 2026-07-14（Asia/Tokyo）
- リポジトリ: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- ブランチ: `develop`
- HEAD: `cbe5fe2 Record incomplete M4 rerun`
- FF-1 revision: `7dd98ad Enforce contract instrumentation for interaction full`（HEAD の祖先であることを確認）
- 指定ワークスペース: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0714_ff1_001`
- 作業ツリー開始時状態: clean（`git status --porcelain` は空）
- 実行規律: 各 run 最大1回、再試行なし

## 使用予定のgoal原文

過去キャンペーンの `uat-meta.json` から次の原文を確認した。計測を開始していないため、いずれもモデルへ送信していない。

- Space: `あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。`
- Breakout: `あなたが考える最高に面白くかっこいいブロック崩しゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。`
- Quiz: `シンプルで美しいクイズアプリ（3問・スコア表示・リトライ可能）を3011ポートで起動可能なnext.jsアプリとして開発してください。`

## Preflight

| 項目 | 結果 | 証跡・判断 |
|---|---|---|
| `git status --porcelain` | PASS | 出力なし |
| `git log -1 --oneline` | PASS | `cbe5fe2 Record incomplete M4 rerun` |
| FF-1 ancestry | PASS | `git merge-base --is-ancestor 7dd98ad HEAD` は exit 0 |
| Ollamaモデル | PASS | `qwen3.6:27b-coding-nvfp4`、`qwen3.6:35b-a3b-coding-nvfp4`、`gemma4:31b` が `ollama list` に存在 |
| `cargo test`（sandbox内） | INVALID / STOPPED | browser probe系がローカルbind権限不足で `Operation not permitted`。この結果では続行せず、権限付きfull suiteへ移行するため exit 130 で停止 |
| `cargo test`（権限付きfull suite） | **FAIL** | exit 101。`1270 passed; 18 failed; 13 ignored` |
| `cargo build --release` | NOT RUN | full suite green が実行開始条件のため未実行 |
| release binary install | NOT RUN | 同上 |
| `commandagent --version` | NOT RUN | 同上 |
| `commandagent --setup-interaction-probe` | NOT RUN | 同上 |

## 権限付きfull suiteの失敗

権限付き実行でも18件が失敗したため、sandbox要因だけではない。主要な失敗形は次のとおり。

1. FF-1裁定との既存fixture/期待値不整合
   - 複数の既存テストが heuristic形の interaction evidence から従来の full/pass を期待している。
   - 実際の失敗理由は `release gate failed: contract_instrumentation_missing:primary`。
   - 例: `plan_run_nextjs_browser_and_interaction_evidence_passes_release_gate`、`plan_run_nextjs_browser_ok_without_render_detail_is_partial`、`surface_fit_overflow_is_telemetry_not_gate_failure`、`ultra_final_acceptance_uses_effective_profile_over_stale_config_profile`。
2. strict gate後のrepair fixture枯渇
   - `ambiguous_generic_app_promotion_keeps_union_contract_and_earns_full_after_gates`、`known_profile_run_never_reinfers_profile`、`profile_promotion_occurs_once_and_ignores_later_manifests`、`generic_ultra_promotes_to_nextjs_after_workspace_manifest` が `fake client exhausted` で失敗。
3. browser probe assertion
   - `child_that_responds_500_reports_http_failure` は期待 `http_status=Some(500)` に対して実値 `None`。
4. final-acceptance test state
   - `final_acceptance_budget_exhaustion_uses_last_cycle_reason` は期待した error に対して success を返し失敗。
   - その後8件が共有ロックの `PoisonError` で連鎖失敗した。

失敗した18テスト:

- `minimal_loop::browser_probe::tests::child_that_responds_500_reports_http_failure`
- `planner::runner::tests::assurance_tests::moved::ambiguous_generic_app_promotion_keeps_union_contract_and_earns_full_after_gates`
- `planner::runner::tests::assurance_tests::moved::plan_run_nextjs_browser_and_interaction_evidence_passes_release_gate`
- `planner::runner::tests::assurance_tests::moved::plan_run_nextjs_browser_ok_without_render_detail_is_partial`
- `planner::runner::tests::assurance_tests::moved::surface_fit_overflow_is_telemetry_not_gate_failure`
- `planner::runner::tests::final_acceptance_budget_exhaustion_uses_last_cycle_reason`
- `planner::runner::tests::final_acceptance_repair_cycle_reprobes_restart_hook_recovery_to_pass`
- `planner::runner::tests::focused_behavioral_repair_exhaustion_handoff_uses_probe_failure`
- `planner::runner::tests::focused_behavioral_repair_prompt_and_reprobe_passes`
- `planner::runner::tests::known_profile_run_never_reinfers_profile`
- `planner::runner::tests::overlay_only_restart_after_probe_success_is_partial_terminal_unreached`
- `planner::runner::tests::profile_promotion_occurs_once_and_ignores_later_manifests`
- `planner::runner::tests::ultra_final_acceptance_uses_effective_profile_over_stale_config_profile`
- `planner::runner::tests::ultra_plan_flow_tests::moved::generic_ultra_promotes_to_nextjs_after_workspace_manifest`
- `planner::runner::tests::ultra_plan_flow_tests::moved::slash_ultra_final_flow_reaches_stop_after_fake_dev_server_cleanup`
- `planner::runner::tests::ultra_plan_flow_tests::moved::ultra_final_acceptance_runs_probe_before_behavior_arbitration`
- `planner::runner::tests::ultra_plan_flow_tests::moved::ultra_final_acceptance_runs_probe_when_runtime_evidence_is_missing`
- `planner::runner::tests::unattached_canvas_ref_guidance_leads_repair_and_reprobe_passes`

## Run結果

| # | run名 | シナリオ / executor | 状態 | 理由 |
|---:|---|---|---|---|
| 1 | `ff1_space_qwen35` | Space / `qwen3.6:35b-a3b-coding-nvfp4` | NOT RUN | preflight failed |
| 2 | `ff1_space_gemma31` | Space / `gemma4:31b` | NOT RUN | preflight failed |
| 3 | `ff1_breakout_qwen35` | Breakout / `qwen3.6:35b-a3b-coding-nvfp4` | NOT RUN | preflight failed |
| 4 | `ff1_breakout_gemma31` | Breakout / `gemma4:31b` | NOT RUN | preflight failed |
| 5 | `ff1_quiz_qwen35` | Quiz / `qwen3.6:35b-a3b-coding-nvfp4` | NOT RUN | preflight failed |
| 6 | `ff1_quiz_gemma31` | Quiz / `gemma4:31b` | NOT RUN | preflight failed |

## FF-1監査表

計測runが存在しないため、値を推測せずすべて未観測とする。

| run | final_state | assurance_level | probe_mode | contract_hook_status | action_hooks | state_dimensions | `contract_instrumentation_missing:*` |
|---|---|---|---|---|---|---|---|
| `ff1_space_qwen35` | NOT RUN | N/A | N/A | N/A | N/A | N/A | N/A |
| `ff1_space_gemma31` | NOT RUN | N/A | N/A | N/A | N/A | N/A | N/A |
| `ff1_breakout_qwen35` | NOT RUN | N/A | N/A | N/A | N/A | N/A | N/A |
| `ff1_breakout_gemma31` | NOT RUN | N/A | N/A | N/A | N/A | N/A | N/A |
| `ff1_quiz_qwen35` | NOT RUN | N/A | N/A | N/A | N/A | N/A | N/A |
| `ff1_quiz_gemma31` | NOT RUN | N/A | N/A | N/A | N/A | N/A | N/A |

### full確認行

該当runなし。これは「6本を実行した結果fullが0本」ではなく、preflight失敗により6本が未実行であるためである。

### 失敗runの修復チェーン

計測runは未実行のため該当なし。`contract_attribute_repair_guidance` の発火有無も未観測。

### イベント確認

runの `.anvil/runs/*/events.jsonl` が存在しないため、次の3イベントはいずれも count不能（N/A）。ゼロ件とは判定しない。

- `interaction_verified_heuristic_only`: N/A
- `contract_instrumentation_missing`: N/A
- `contract_attribute_repair_guidance`: N/A

## 事前宣言基準の判定

- P0-a: **未判定（計測不成立）**。full runの正常系パリティを評価していない。6本実行後にfullが0本だった場合のFAIL条件とは区別する。
- P0-b: **未判定（計測不成立）**。heuristic-only合格でfullを得るrunがないことを実runでは評価していない。
- 総合: **BLOCKED**。full suite greenという開始条件を満たしていない。

## 中断理由と操作判断

- 環境要因: sandbox内ではbrowser probe系のローカルbindが拒否された。
- 操作判断: sandbox結果のまま続行せず、権限付きfull suiteを再実行した。
- 非環境要因: 権限付きfull suiteにも18件の失敗が残った。
- 停止判断: 「greenにできない場合は計測を開始せず中断」の明示規律に従い、release build/install/probe setupおよび6 runを開始しなかった。

## 成果物

- 本レポートのみ。
- 計測run artifactsなし（6 run未実行）。
- 指定ワークスペースは未作成。
- 計測を中断した時点では、リポジトリのソース・テスト・docsを変更していない。

## Follow-up調査・修正（2026-07-15）

本節は、上記BLOCKED判定後にユーザー承認を得て行った別作業の記録である。元のpreflight結果、6 run未実行、P0-a / P0-b未判定という履歴は変更しない。

### 原因

FF-1の本番裁定ロジックではなく、既存テストの正常系fixtureと期待値がFF-1以前のままだったことが主因だった。

- full/passを期待する正常系fixtureが、`probe_mode=contract`、primary action hook、非空の`state_dimensions`を備えないheuristic evidenceを使用していた。
- FF-1はそのfixtureを仕様どおり`contract_instrumentation_missing:primary`として拒否した。
- restartが必要なケースでは、primary/state契約を満たしてもrestart hookがないfixtureを、従来のpartial/passとして期待する不整合もあった。
- 先頭の失敗で共有テストmutexがpoisonされ、後続8件が`PoisonError`で連鎖失敗していた。
- `final_acceptance_budget_exhaustion_uses_last_cycle_reason`の並列時失敗は、過去の中断実行から残っていたNext.jsプロセスが`*:34019`をlistenしていたことと、macOS上のIPv4 bind確認だけではIPv6 wildcard listenerを検出できなかったことが原因だった。残留プロセスは停止せず、テスト用port選定を修正して回避した。
- `child_that_responds_500_reports_http_failure`は単独および修正後full suiteで成功し、独立した製品不具合は再現しなかった。

### 対応

FF-1のstrict gateおよび本番のrelease qualificationは変更していない。

- full/pass正常系fixtureをcontract-modeへ更新し、primary/restart action hooks、usableなcontract hook status、非空のstate dimensionsを明示した。
- restart欠落用fixtureはprimary/state契約を満たしたままrestartだけを欠落させ、`contract_instrumentation_missing:restart`、failed/incomplete、repair chain発火を検証する負例へ更新した。
- 正常系corpusにrelease qualification期待値を追加した。
- 共有テストmutexをpoison後も回収できるようにし、一次失敗による連鎖失敗を防止した。
- テスト用port選定でbind後にlocalhost接続可否も確認し、別アドレスファミリでlisten中のportを除外した。

変更対象:

- `src/planner/runner.rs`（`#[cfg(test)]`配下のfixture・test harnessのみ）
- `src/planner/runner/tests/assurance_tests.rs`
- `src/planner/runner/tests/ultra_plan_flow_tests.rs`
- `tests/corpus/apps/local-q1-final-content-b-web-full-pass/expectations.toml`

### 検証

| 確認 | 結果 |
|---|---|
| FF-1関連のfocused tests | PASS |
| `cargo test --lib` | PASS: `1288 passed; 0 failed; 13 ignored` |
| `cargo test`（権限付きfull suite） | PASS: exit 0。全test binaryおよびdoc-test green |
| corpus regression | PASS: `generated_app_corpus_matches_detector_and_probe_expectations` |
| `cargo fmt --all -- --check` | PASS |
| `git diff --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | FAIL: 今回の差分外にある既存9 warning |

Clippyの9件は、`repair_pressure.rs`と`stagnation_escalation.rs`のderivable impl、`contract_attribute_repair.rs`のneedless borrow/range loop、`lint_rejection.rs`のlarge error、`runner.rs`既存本番行のcollapsible if、`verify.rs`のquestion-mark、Next.js knowledgeのobfuscated if/else、data manifestのunnecessary owned conversionである。FF-1修正差分を広げないため、このfollow-upでは変更していない。

### Follow-up後の判断

- preflightを止めた主因だったfull suite failureは解消した。
- ただし、この修正は元のUAT計測後に作業ツリーへ加えた未コミット差分であり、元の6 runを後から有効化するものではない。
- FF-1パリティは、修正のレビュー・コミット後、そのrevisionをrelease build/installし、新規workspace・新規run IDで改めて6 run実行する必要がある。

## Follow-up prompt（UAT失敗時・対応済み）

`$codex-issue-worker` を明示的に起動し、`cbe5fe2` の権限付き `cargo test` で再現する18件の失敗を修正してください。FF-1の strict gate は弱めず、heuristic-only evidenceからfullを期待するfixtureを contract-mode（primary hook、必要なrestart hook、非空state_dimensions）へ更新してください。`child_that_responds_500_reports_http_failure` と `final_acceptance_budget_exhaustion_uses_last_cycle_reason` は別原因として切り分け、PoisonError連鎖の起点を先に直してください。修正後に権限付きfull suite greenを確認し、新規ワークスペースと新規run IDでFF-1パリティ計測を再発注してください。
