# 結果サマリ

## 事前宣言（実行前固定）

- 分母: 3 suite × 3変種 × 4 run = 36 run（各suite 12 run）。
- Go/No-Go: 一発full（修復0）≥60%、修復込みfull≥90%、所要p50≤180秒、1生成≤$0.067。
- 点予測は置かない。最近接の間接実測は ingest×Luna 100%（n=6）、Quiz 92%。Wilson 95% CIを実測に併記する。
- 予算上限: `$5`。超過見込みまたは系統的な同一停止クラス連続時は停止する。
- 計器pin: production HEAD `defec54369db5683ee00247f2016f92d16bc9acd`、計測suite instrument commit `6afe1e8b512efb8eda2549f43913db3313cfb940`、release binary SHA-256 `beb1a1048518b799d4945a9a9539132b4141db7601ec238e5e3f416141fc58e6`。
- executor: `gpt-5.6-luna` / OpenAI Responses/native、planner: `qwen3.6:27b-coding-nvfp4` / Ollama。

宣言とsuite実行前に、製品suite 3本のsha256とmanifestを照合した。製品側の封緘値は不変（warikan `215aba…d9e0c`、mochimono `1246b8…a9a6`、vote `da5166…d08df`、manifest `4ea74f…dad86`）。

## 実行結果と停止

`warikan_001` のみ開始。schema供給経路のfrozen dataclass代入を最小修正してpushした後、修正HEADで再実行した。L2 `app.spec.yaml` とschema pin供給までは成立したが、plannerが `weak_code_verify` / `weak_verify_only` / `artifact_owner_without_local_verify` を各phaseで連続発生させ、約11分後も同一 `planner_quality_retry` から進まなかったため停止した。bench原文は `bench: interrupted(environment); the run will not be retried` と記録され、未完了runは統計分母へ算入しない。

| 閾値 | 実測 | Wilson 95% CI | 判定 |
|---|---:|---:|---|
| 一発full ≥60% | 判定可能な完了run 0/0 | — | 判定停止 |
| 修復込みfull ≥90% | 判定可能な完了run 0/0 | — | 判定停止 |
| 所要p50 ≤180s | 完了runなし。中断runの経過約11分はp50へ不算入 | — | 判定停止 |
| 1生成 ≤$0.067 | `warikan_001` events導出 `$0.00809690` | — | 観測値は閾値内、全体判定停止 |

### 実行内訳

| suite | run | status | verdict/full | cost_usd | artifact |
|---|---|---|---|---:|---|
| warikan | 001 | environment-interrupted（非消費） | incomplete | 0.00809690 | `warikan-001-campaign/artifacts/warikan_001/` |
| warikan | 002–012 | 未実施 | — | — | — |
| mochimono | 001–012 | 未実施 | — | — | — |
| vote | 001–012 | 未実施 | — | — | — |

provider eventsの実測は `provider_total_tokens=35437`、`cached_input_tokens=21025`、`cost_usd=0.0080969`。未完了runを成功・失敗へ水増ししていない。p50/p95、Wilsonのfull率、36runのGo判定は未計測であり、Phase 2 GOは宣言しない。

## 生成物・昇格・失敗帰属

- L2率: 1/1開始run（`app.spec.yaml`生成）。
- L3昇格率: 0/1（`promotion_decision` evidenceなし）。
- 代表spec: `warikan-001-campaign/artifacts/warikan_001/app.spec.yaml`。
- 失敗／停止語彙: `planner_quality_retry`（連続）、bench最終状態 `environment-interrupted`。新failure classは登録していない（既存語彙で表現可能）。

## バンド・地図・台帳

- [band_summary_community.md](band_summary_community.md): Full meaningを明記し、完了0・中断1（統計除外）の五数要約を記録。
- [score_time_map_community.md](score_time_map_community.md): community観測点（L2生成、full未判定、cost実測）を追加。
- [ledger.md](ledger.md): CM-2b-002事前宣言、schema floor返済、planner quality連続停止を1行で記録。

## 変更・CI

- `defec543` Fix community schema procurement record（`dataclasses.replace`、既存bench 32 tests green）。
- CI: run `32030824330` completed/success（exact SHA `defec54369db5683ee00247f2016f92d16bc9acd`）。
- acceptance: run `32030824325` completed/success（exact SHA `defec54369db5683ee00247f2016f92d16bc9acd`）。
