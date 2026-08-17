# 結果サマリ

## 事前宣言（実行前固定）

- 分母: 3 suite × 3変種 × 4 run = 36 run。
- Go/No-Go: 一発full（修復0）≥60%、修復込みfull≥90%、所要p50≤180秒、1生成≤$0.067。
- 直接priorなし。最近接の間接実測は ingest×Luna 100%（n=6）、Quiz 92%。点予測は置かず、Wilson 95% CIを実測に併記する。
- 予算上限 `$5`。同一系統の重大乖離が連続した場合は途中停止する。
- 計器pin: product HEAD `ae977763ce7018e9effd23048bfe97ef81aef7a2`、measurement instrument commit `178e09c22c0008aa9f7cf270d1aee3af8f7a8689`、release binary SHA-256 `87861649b3ffc5e850db52ecbb293d6c16f23d919e93754beb6ade943775784e`。
- executor `gpt-5.6-luna` / Responses/native、planner `qwen3.6:27b-coding-nvfp4` / Ollama。
- CM-2b-golden-001/002のwarikan-001/002窓は、旧floor返済前の測定として分母から除外する。

製品suite 3本と既存manifestのsha256を先行照合した。製品封緘値は不変（warikan `215aba…d9e0c`、mochimono `1246b8…a9a6`、vote `da5166…d08df`、manifest `4ea74f…dad86`）。

## 実行結果・停止

warikan_001/002は完了扱いの失敗run、warikan_003は環境中断（非消費）として記録する。新guidanceにより両完了runで `node smoke-check.js` が計画・実行されたが、検証経路で連続する重大乖離が発生したため、残り33runを実施せず停止した。

| 閾値 | 実測 | Wilson 95% CI | 判定 |
|---|---:|---:|---|
| 一発full ≥60% | 0/2 = 0%（完了full 0） | [0%, 65.8%] | 未達・停止 |
| 修復込みfull ≥90% | 0/2 = 0% | [0%, 65.8%] | 未達・停止 |
| 所要p50 ≤180s | 181.5s（209s, 154s） | — | 未達・停止 |
| 1生成 ≤$0.067 | `$0.00591378`, `$0.00226446` | — | 観測値は達成 |

### run内訳

| run | status | stop/violation | duration | cost_usd |
|---|---|---|---:|---:|
| warikan_001 | failed | `entity.fields must be a mapping`（`node smoke-check.js`実行後） | 209s | 0.00591378 |
| warikan_002 | failed | `dependency_setup_authority_required: node smoke-check.js` | 154s | 0.00226446 |
| warikan_003 | environment-interrupted（非消費） | bench原文 `interrupted(environment)` | 0s | 0 |
| warikan_004–012 | 未実施 | 停止線 | — | — |
| mochimono_001–012 | 未実施 | 停止線 | — | — |
| vote_001–012 | 未実施 | 停止線 | — | — |

provider events実測合計は `provider_total_tokens=43713`、cost `$0.00817824`。未実施・中断runをWilson分母へ算入していない。4閾値全達成ではないためPhase 2 GOは宣言しない。

## 伝達・生成物・帰属

- planner伝達: `node smoke-check.js` が2件とも計画verifyへ現れ、001では実行まで到達。宣言語彙の伝達自体は確認できた。
- L2率: 2/2完了runで `app.spec.yaml` が生成。
- L3昇格率: 0/2（promotion evidenceなし）。
- 失敗帰属: 001はschema fixture形状（`entity.fields` mapping違反）、002はverify commandの依存権限境界。新クラスは登録していない（既存停止語彙で表現可能）。

## バンド・地図・台帳

- [band_summary_community.md](band_summary_community.md)
- [score_time_map_community.md](score_time_map_community.md)
- [ledger.md](ledger.md)

## CI

実装HEAD `ae977763` のCI/acceptance exact-SHA conclusionは、push後に確定値を追記する。

