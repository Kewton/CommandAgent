# 結果サマリ

- 判定: **GO材料成立**。通常createはlocalで`full`、Lunaの初回createは
  honest failure、同runが保存したrecovery YAMLの1周で`full`へ到達した。
- headless: success/failureともstdout最終行が
  `commandagent.headless-summary/v1`の1行JSONとなり、成果物・sheet・eventsを
  絶対pathで取得できた。
- core discipline: 既存のprovider timeout、phase/step runner、acceptance、
  recoveryを通過し、判定・集計・gateのheadless専用分岐は追加していない。
- 照会回答: なし。代わりに親greenと`ccc70f5..8fdeb68`のpreflight棚卸しを
  以下へ固定する。

# CM-0 — Builder Plane成立性の実測

実測日: 2026-08-17
対象build: `commandagent 0.1.0 02a1b536+dirty 2026-08-17T15:01:23+09:00`
（`+dirty`は既存のtask外worktree変更を保持したままbuildしたことを示す）。

## Preflight

作業開始時のlocal/remote `develop`はともに
`8fdeb68bd672839ac2dbee438704ffa75bfeae3e`だった。full SHAを指定した
GitHub Actions APIの確定値は次のとおりで、両方greenのため作業を開始した。

| workflow | run id | status | conclusion |
| --- | ---: | --- | --- |
| CI | `31997905588` | completed | success |
| acceptance | `31997905614` | completed | success |

### `ccc70f5..8fdeb68` core-track棚卸し

範囲の起点は`ccc70f55f9f0f52fc9eb93c8b27e3c10e9150337`。指定4領域の
合計は14 files、+787/-16行だった。

| file | lines | 要旨 |
| --- | ---: | --- |
| `docs/cli-profile-contract.md` | +11/-0 | GUI Gate 1の日本語説明を追加し、C1–C4のID・判定・証跡を不変と明記。 |
| `docs/workflow-circle-contract.md` | +1/-1 | provider列挙へ`lm-studio`を加法追加。 |
| `src/planner/hook_snapshot.rs` | +1/-0 | test ConfigへLM Studio既定hostを追加。 |
| `src/planner/profiles/nextjs.rs` | +15/-2 | tsconfig includeを既知source treeへ限定し、依存導入済みnested projectを選択対象に保持するtestを追加。 |
| `src/planner/profiles/nextjs/knowledge.toml` | +1/-1 | 同じtsconfig include契約へ同期。 |
| `src/planner/profiles/nextjs/manifest.toml` | +1/-1 | 同じmanifest scaffold bytesへ同期。 |
| `src/planner/runner/tests/driver_tests.rs` | +1/-0 | test ConfigへLM Studio既定hostを追加。 |
| `src/providers/guidance.rs` | +9/-6 | LM Studioの接続・認証・model-not-found guidanceと表示名を追加。 |
| `src/providers/lm_studio.rs` | +567/-0 | OpenAI互換Chat/Responses client、optional token、retry/event/metadata、redactionとrequest-shape testsを追加。 |
| `src/providers/mod.rs` | +2/-0 | LM Studio moduleとConfig dispatchを登録。 |
| `src/providers/openai_chat_completions.rs` | +39/-1 | 共通builderを抽出し、LM Studioだけ`max_tokens`、reasoning未指定を使用。 |
| `src/providers/openai_responses.rs` | +42/-2 | 共通base builderを抽出し、OpenAIの`store/include` bytesを保持しつつLM Studioでは未送信。 |
| `src/providers/startup.rs` | +93/-0 | LM Studio reachability/model visibilityのstartup warningとtestsを追加。 |
| `workspace/management/scripts/test_first_loop_doc.py` | +4/-2 | GUI Gate 1の日本語確認文言へdoc drift assertionを同期。 |

## 実弾3構成

3本は同じgoalを使った。`OPENAI_API_KEY`はGit ignoredなrepo root `.env`を
同一shellでsourceし、値をstdout、argv、events、reportへ出していない。

```text
Build a minimal Next.js App Router mini app named Pulse Counter for port 3011. Show a count with Increment and Reset buttons, keep all behavior client-side, use no external APIs or assets, and verify the production build.
```

### 1. nextjs × create — existing profile / local model

```bash
target/release/commandagent --yes --quiet --footer off --summary-json \
  --cwd /private/tmp/cm0-headless-probe/local \
  --profile nextjs --intent create \
  --provider ollama --model qwen3.6:27b-coding-nvfp4 \
  --planner-provider ollama --planner-model qwen3.6:27b-coding-nvfp4 \
  --context-budget 65536 --ultra-plan-run \
  "Build a minimal Next.js App Router mini app named Pulse Counter for port 3011. Show a count with Increment and Reset buttons, keep all behavior client-side, use no external APIs or assets, and verify the production build."
```

- exit code: `0`
- provider turns: 11、timeout/retry: 0/0、最大turn: 179.101秒、bound: 600秒
- provider cost: `null`（local Ollama。電力量は未計測）

```json
{"schema_version":"commandagent.headless-summary/v1","run_id":"01a00e52-ba5d-7f83-9f5f-162908f01673","verdict":"full","assurance":"full","score":null,"acceptance_sheet_path":"/private/tmp/cm0-headless-probe/local/.anvil/runs/01a00e52-ba5d-7f83-9f5f-162908f01673/summary.md","artifacts_dir":"/private/tmp/cm0-headless-probe/local","events_path":"/private/tmp/cm0-headless-probe/local/.anvil/runs/01a00e52-ba5d-7f83-9f5f-162908f01673/events.jsonl","duration_secs":381.794,"provider_cost_usd":null,"stop_class":null,"directive_round":0}
```

### 2. nextjs × create — gpt-5.6-luna / Responses / native

```bash
set -a
source ./.env
set +a
target/release/commandagent --yes --quiet --footer off --summary-json \
  --cwd /private/tmp/cm0-headless-probe/openai \
  --profile nextjs --intent create \
  --provider openai --model gpt-5.6-luna \
  --planner-provider openai --planner-model gpt-5.6-luna \
  --api responses --tool-protocol native --context-budget 65536 \
  --ultra-plan-run \
  "Build a minimal Next.js App Router mini app named Pulse Counter for port 3011. Show a count with Increment and Reset buttons, keep all behavior client-side, use no external APIs or assets, and verify the production build."
```

- exit code: `1`
- verdict/assurance: `partial` / `partial`
- stop class: `direct_cli_command_failed`
- 原文: `phase setup-nextjs-app failed: path does not exist: package.json gente? no such file?`
- provider turns: 2、timeout/retry: 0/0、最大turn: 5.726秒、bound: 180秒
- token: input 2,403（cached 0）、output 929、total 3,332
- 価格換算: `$0.0015954`（JSONは価格正本を持たないため`null`のまま）

```json
{"schema_version":"commandagent.headless-summary/v1","run_id":"01a00e80-b4b3-7dd1-a6f8-e9dbb5aad533","verdict":"partial","assurance":"partial","score":null,"acceptance_sheet_path":"/private/tmp/cm0-headless-probe/openai/.anvil/runs/01a00e80-b4b3-7dd1-a6f8-e9dbb5aad533/summary.md","artifacts_dir":"/private/tmp/cm0-headless-probe/openai","events_path":"/private/tmp/cm0-headless-probe/openai/.anvil/runs/01a00e80-b4b3-7dd1-a6f8-e9dbb5aad533/events.jsonl","duration_secs":14.194,"provider_cost_usd":null,"stop_class":"direct_cli_command_failed","directive_round":0}
```

### 3. same goal — saved recovery/fix one-shot

構成2が保存した
`.anvil/plans/recovery-ultra-plan-phase-setup-nextjs-app-01a00e81-0ba0-7251-b7db-316da250d9c8.yaml`
を同じworkspaceへ1周だけ適用した。新しいgoalやdirectiveは注入していない。

```bash
set -a
source ./.env
set +a
target/release/commandagent --yes --quiet --footer off --summary-json \
  --cwd /private/tmp/cm0-headless-probe/openai \
  --profile nextjs \
  --provider openai --model gpt-5.6-luna \
  --planner-provider openai --planner-model gpt-5.6-luna \
  --api responses --tool-protocol native --context-budget 65536 \
  --run-ultra-plan \
  /private/tmp/cm0-headless-probe/openai/.anvil/plans/recovery-ultra-plan-phase-setup-nextjs-app-01a00e81-0ba0-7251-b7db-316da250d9c8.yaml
```

- exit code: `0`
- verdict/assurance: `full` / `full`
- provider turns: 9、timeout/retry: 0/0、最大turn: 20.647秒、bound: 180秒
- token: input 37,615（cached 22,415）、output 6,466、total 44,081
- 価格換算: `$0.0112475`
- create+recovery合計価格換算: `$0.0128429`

```json
{"schema_version":"commandagent.headless-summary/v1","run_id":"01a00e81-8b43-7631-8a11-f5f1f1a05f81","verdict":"full","assurance":"full","score":null,"acceptance_sheet_path":"/private/tmp/cm0-headless-probe/openai/.anvil/runs/01a00e81-8b43-7631-8a11-f5f1f1a05f81/summary.md","artifacts_dir":"/private/tmp/cm0-headless-probe/openai","events_path":"/private/tmp/cm0-headless-probe/openai/.anvil/runs/01a00e81-8b43-7631-8a11-f5f1f1a05f81/events.jsonl","duration_secs":84.42,"provider_cost_usd":null,"stop_class":null,"directive_round":0}
```

価格換算は2026-08-17取得の
[GPT-5.6 Luna公式価格](https://developers.openai.com/api/docs/models/gpt-5.6-luna)
（input `$0.20`、cached input `$0.02`、output `$1.20` / 1M tokens）を
event正本のprovider tokenへ適用した参考値である。請求明細を正本とし、
CommandAgentは価格表を保持しないので`provider_cost_usd`を発明せず`null`にした。

### jqによる成果物path抽出

呼び出し側はstdout最終行を`SUMMARY_JSON`として受け取り、次のように抽出できる。

```bash
printf '%s\n' "$SUMMARY_JSON" | jq -r \
  '[.artifacts_dir, .acceptance_sheet_path, .events_path] | @tsv'
```

構成3の出力は次の3 pathとなる。

```text
/private/tmp/cm0-headless-probe/openai
/private/tmp/cm0-headless-probe/openai/.anvil/runs/01a00e81-8b43-7631-8a11-f5f1f1a05f81/summary.md
/private/tmp/cm0-headless-probe/openai/.anvil/runs/01a00e81-8b43-7631-8a11-f5f1f1a05f81/events.jsonl
```

## Phase 0の問い

| 問い | 実測回答 | 企画書更新材料 |
| --- | --- | --- |
| headless成立性 | **GO**。exit 0とexit 1の両方で最終stdout行JSONを取得し、`verdict`と`assurance`をexit codeと分離できた。 | Builder Planeはprocess exitだけでfull扱いせず、`verdict`/`assurance`をgateする。 |
| タイムアウト挙動 | 指定3本22 provider turnsでtimeout 0。localは600秒bound内最大179.101秒、remoteは180秒bound内最大20.647秒。既存`provider_call`がturn durationとtimeoutを記録し、headlessはterminal `stop_class`を投影する。 | 新しいheadless timeout判定は不要。既存bounded provider failureをそのまま消費する。 |
| 成果物の機械可読取り出し | **GO**。`artifacts_dir`、`acceptance_sheet_path`、`events_path`が絶対pathで得られ、failed runでもrecovery YAMLはevents/sheetから保持された。 | Builder PlaneはJSON pathを正本参照として使い、workspace探索を再発明しない。 |
| step-runner実態 | E-5dは完了済み。`runner.rs`は144行facadeで、driver/phase/acceptanceへ分割済み。E-5fのtyped phase state machineも実装済みで、`phase/flow.rs`から`phase/step_plan_execution.rs`の既存StepPlan実行へ配線されている。実弾でもphase start→StepPlan→verification→acceptanceを観測した。 | 古い「step-runner配線中」は「E-5d分解・E-5f state machine・StepPlan実行配線済み」へ更新する。 |
| XMLフォールバック現状 | OpenAIは明示`--api responses --tool-protocol native`でnative function toolsが稼働済み。API/protocolはmodel名sniffをしない。明示`text`または既存error-driven fallbackでは、検証付きXML parserと方言repairを再利用する。 | 古い「XML閾値超過ならnative追加」は解消済み。「Responses/native装備済み、text/XML方言修復は互換fallbackとして残存」へ更新する。 |

## 互換性と逸脱

- `--summary-json`未指定時は既存stdout pathを変更しない。raw stdout goldenと
  integration testが`No runs found for this workspace.\n`の完全一致を固定する。
- OpenAI/Ollama provider実装はLM Studio監査コミットで変更していない。
- 設計外の床: なし。構成2のinvalid path failureは既存のpath validationで
  honest failureとなり、既存recovery契約内で1周後fullへ回収された。
- 追加観測（指定3本の分母外）: OpenAI keyが初回process環境になかった間、構成1の
  内部release-gate handoffからlocal recoveryを1周した。exit 0、full、552.345秒。
  指定3本の成功率・所要・費用には算入しない。
