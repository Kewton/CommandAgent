# FF-1 パリティ計測レポート（uat-test0715-ff1-001）

## 結論

**PASS。P0-a / P0-bともに合格。** 6 runを各1回だけ実行し、3 runが`full`を獲得した。full全件が`probe_mode=contract`、primary hook、非空の`state_dimensions_changed`を伴っていた。また、`interaction_verified_heuristic_only`は全runで0件であり、heuristic-only evidenceからfullを得たrunは存在しなかった。

- P0-a: **PASS** — full 3本。全3本がcontract + primary + 非空state dimensions。
- P0-b: **PASS** — heuristic-only full 0本。全6本の最終browser interaction evidence自体がcontract-mode。
- run成否: completed/full 3本、failed/incomplete 3本。
- FF-1外の観測事項: restart contract欠落を検出した2 runでは、通常のfinal-acceptance repair chainは発火したが、イベント名`contract_attribute_repair_guidance`は0件だった。

## 対象とパス解決

- 実行日: 2026-07-15（Asia/Tokyo）
- リポジトリ: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- ブランチ: `develop`
- HEAD: `dac63de Align FF-1 contract parity fixtures`
- FF-1 revision: `7dd98ad Enforce contract instrumentation for interaction full`（HEADの祖先）
- workspace: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_ff1_001`
- report: `workspace/management/runs/uat-test0715-ff1-001/`

依頼冒頭の新規パス`test0715_ff1_001`を正とした。本文中に残っていた`test0714_ff1_001`および`uat-test0714-ff1-001`は前回の計測不成立記録と衝突するため転記残りと判断し、既存履歴を変更せず新規IDを使用した。

## Preflight

| 項目 | 結果 | 証跡 |
|---|---|---|
| `git status --porcelain` | PASS | 開始時出力なし |
| `git log -1 --oneline` | PASS | `dac63de Align FF-1 contract parity fixtures` |
| FF-1 ancestry | PASS | `git merge-base --is-ancestor 7dd98ad HEAD` exit 0 |
| 新規workspace/report ID | PASS | いずれも開始前に非存在 |
| Ollama models | PASS | planner `qwen3.6:27b-coding-nvfp4`、executor `qwen3.6:35b-a3b-coding-nvfp4` / `gemma4:31b`を確認 |
| 権限付き`cargo test` | PASS | exit 0。lib `1288 passed; 0 failed; 13 ignored`、全integration/doc tests green |
| `cargo build --release` | PASS | exit 0 |
| release binary install | PASS | `install -m 755 target/release/commandagent ~/.local/bin/commandagent` |
| binary一致 | PASS | target/installともSHA-256 `f44ee6737fffc3b77d2836cdd11376deefa79852d00f4ea87bc0953c12d40474` |
| `commandagent --version` | PASS | `commandagent 0.1.0 dac63de 2026-07-14T16:08:30Z`、`+dirty`なし |
| `commandagent --setup-interaction-probe` | PASS | `probe ready: playwright 1.61.1 (managed_interaction_probe)` |

## Goal原文

- Space: `あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。`
- Breakout: `あなたが考える最高に面白くかっこいいブロック崩しゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。`
- Quiz: `シンプルで美しいクイズアプリ（3問・スコア表示・リトライ可能）を3011ポートで起動可能なnext.jsアプリとして開発してください。`

全run共通でplannerはOllama `qwen3.6:27b-coding-nvfp4`、実行形式は`--ultra-plan-run --profile nextjs --plan-preset profile --yes`とした。executorのみ表のモデルへ切り替えた。

## Run結果

| # | run | executor | exit / terminal | final acceptance | assurance | runtime / release | 最終理由 | 時間 |
|---:|---|---|---|---|---|---|---|---:|
| 1 | `ff1_space_qwen35` | qwen35 | 1 / failed | incomplete | partial | pass / failed | `contract_instrumentation_missing:restart`; repairはread-only stagnation | 801,925 ms |
| 2 | `ff1_space_gemma31` | gemma31 | 0 / completed | full_success | full | pass / pass | pass | 1,069,370 ms |
| 3 | `ff1_breakout_qwen35` | qwen35 | 1 / failed | incomplete | partial | failed / failed | `missing_required_evidence:stateful_update_evidence`; `input_state_change_missing_after_start` | 467,200 ms |
| 4 | `ff1_breakout_gemma31` | gemma31 | 1 / failed | incomplete | partial | failed / failed | `restart_or_recoverable_state_evidence`; `input_state_change_missing_after_start` | 1,245,832 ms |
| 5 | `ff1_quiz_qwen35` | qwen35 | 0 / completed | full_success | full | pass / pass | pass | 633,675 ms |
| 6 | `ff1_quiz_gemma31` | gemma31 | 0 / completed | full_success | full | pass / pass | pass | 382,322 ms |

各runはプロセス起動を1回だけ行った。表中のrepairやplanner retryは同一run内の製品所定bounded chainであり、runの再試行ではない。

## FF-1監査表

`release_gate_reasons`は最後の`ultra_final_acceptance`イベントを採用した。

| run | final_state | assurance | probe_mode | contract_hook_status | action_hooks | state_dimensions | `contract_instrumentation_missing:*` |
|---|---|---|---|---|---|---|---|
| `ff1_space_qwen35` | failed / incomplete | partial | contract | usable | `primary` | `playerX` | **yes: restart** |
| `ff1_space_gemma31` | completed / full_success | **full** | contract | usable | `primary` | `alienPositions` | no |
| `ff1_breakout_qwen35` | failed / incomplete | partial | contract | usable | `primary` | `[]` | no |
| `ff1_breakout_gemma31` | failed / incomplete | partial | contract | usable | `primary` | `[]` | no（cycle 0ではrestart欠落を検出） |
| `ff1_quiz_qwen35` | completed / full_success | **full** | contract | usable | `primary` | `score` | no |
| `ff1_quiz_gemma31` | completed / full_success | **full** | contract | usable | `primary` | `currentQuestionIndex`, `score` | no |

### full確認行

| full run | probe_mode=contract | primary hook | 非空state dimensions | 確認 |
|---|---|---|---|---|
| `ff1_space_gemma31` | yes | yes（evidenceおよび最終source） | `alienPositions` | PASS |
| `ff1_quiz_qwen35` | yes | yes（evidenceおよび最終source） | `score` | PASS |
| `ff1_quiz_gemma31` | yes | yes（evidenceおよび最終source） | `currentQuestionIndex`, `score` | PASS |

fullを名乗った全runが3条件を満たした。primary hookは`browser-interaction.json`の`action_hooks`に加え、退避した`final-page.tsx`の`data-anvil-action="primary"`でも確認した。

## 失敗runと修復チェーン

### `ff1_space_qwen35`

- contract欠落内訳: primary=no、state_change=no、**restart=yes**。
- browser probe自体はcontract/usableで`playerX`変化を観測したが、start後のrestart hookは`restart_hook_count_after_start=0`、`restart_hook_reachable_after_start=false`。
- 最終sourceにはrestart属性の記述があるが、probe時に到達可能なrestart contractとして成立しなかったため、FF-1がfullを拒否した。
- `final_acceptance_repair_start`: 1件、`final_acceptance_repair_failed`: 1件。
- `contract_attribute_repair_guidance`: **0件**。
- repairは`test_or_evidence`を対象に発火したが、`model_stagnation:read_only_loop`で終端した。

### `ff1_breakout_qwen35`

- 最終失敗理由に`contract_instrumentation_missing:*`なし。
- contract/usable、primary/restart hooksは観測されたが、`input_state_change=false`、state dimensionsは空。
- `contract_attribute_repair_guidance`: **2件**。いずれも`src/app/page.tsx`の`data-anvil-state`に対するguidance。
- `final_acceptance_repair_start`: 1件、`final_acceptance_repair_failed`: 1件。read-only stagnationで終端。

### `ff1_breakout_gemma31`

- cycle 0のcontract欠落内訳: primary=no、state_change=no、**restart=yes**。ここでは`contract_instrumentation_missing:restart`を検出。
- 通常repair後の最終cycleではcontract missingは解消したが、`input_state_change=false`、state dimensions空、restart/recovery evidence未充足で終端。
- `final_acceptance_repair_start`: 2件。
- `contract_attribute_repair_guidance`: **0件**。
- 最終handoffのprompt/YAMLはparseおよびcommand target validation済み。

restart contract欠落時にstrict gateと通常repair chainは機能した一方、専用イベント`contract_attribute_repair_guidance`は上記2ケースで発火しなかった。これは事前P0基準には含まれないため合否を変えないが、repair guidance telemetryの追跡事項とする。

## イベント確認

指定された`grep -c`を各runの`events.jsonl`へ実行した結果:

| run | `interaction_verified_heuristic_only` | `contract_instrumentation_missing` | `contract_attribute_repair_guidance` |
|---|---:|---:|---:|
| `ff1_space_qwen35` | 0 | 7 | 0 |
| `ff1_space_gemma31` | 0 | 0 | 0 |
| `ff1_breakout_qwen35` | 0 | 0 | 2 |
| `ff1_breakout_gemma31` | 0 | 4 | 0 |
| `ff1_quiz_qwen35` | 0 | 0 | 0 |
| `ff1_quiz_gemma31` | 0 | 0 | 0 |

`grep -c`は該当文字列を含むJSONL行数であり、同一理由が複数イベントのfieldに投影された場合も各行を数える。

## 事前宣言基準の判定

- **P0-a: PASS。** fullは3本でゼロではない。その全件が`probe_mode=contract`、primary hook、非空state dimensionsを伴う。正常系contract成果物は厳格化後もfullを獲得できた。
- **P0-b: PASS。** heuristic-only合格でfullを得たrunは0本。`interaction_verified_heuristic_only`は全run 0件で、6本すべての最終browser evidenceがcontract-modeだった。contract不備を残したSpace/Qwenはpartial/incompleteとして拒否された。
- **総合: PASS。** FF-1の2方向仮説を本計測で確認した。

3本のrun失敗はアプリ固有のrestart到達性・state change evidenceまたは修復stagnationによる正直終端であり、heuristic evidenceのfull昇格ではない。

## 実行規律と環境

- 各runのプロセス起動は最大1回。異常終了を含め、再実行・やり直しなし。
- 各run終了後に3011 listenerの解放を確認して次runへ進んだ。
- preflight後の環境要因による全体中断なし。
- 長いOllama turnは製品設定の600秒上限内で待機し、操作判断による割込みなし。
- リポジトリのproduction source、tests、docsは変更していない。新規計測レポートとartifactsのみを追加した。

## Artifacts

各runのartifactディレクトリには、完全な`.anvil/`コピー（events、summary、browser evidence、completion contracts、plans、repair markdown、recovery YAML、snapshotsを含む）と`final-page.tsx`を保存した。

| run | run UUID | artifact |
|---|---|---|
| `ff1_space_qwen35` | `019f6165-f698-7eb1-a521-79afc5327236` | `artifacts/ff1_space_qwen35/` |
| `ff1_space_gemma31` | `019f6173-427c-79f2-98c7-41b82629bcd7` | `artifacts/ff1_space_gemma31/` |
| `ff1_breakout_qwen35` | `019f6184-9583-71d0-84e5-db6b8f4e9e9f` | `artifacts/ff1_breakout_qwen35/` |
| `ff1_breakout_gemma31` | `019f618c-9d51-7fe1-8827-9f618d51a9ea` | `artifacts/ff1_breakout_gemma31/` |
| `ff1_quiz_qwen35` | `019f61a0-b4f6-7221-a268-76d991574011` | `artifacts/ff1_quiz_qwen35/` |
| `ff1_quiz_gemma31` | `019f61ab-40b4-7fe0-b38a-6c04346648d6` | `artifacts/ff1_quiz_gemma31/` |

機械可読の設定・結果一覧は`uat-meta.json`に保存した。
