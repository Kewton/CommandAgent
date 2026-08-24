# uat-test0730-cli-pack-001: cli-assist初A/B計測

実施日: 2026-07-30 (JST)

裁定契約:

- `docs/cli-profile-contract.md` (fixed 2026-07-24)
- `docs/pack-institution-contract.md` (fixed 2026-07-30)

計測revision: `5653798098e71a1a1f81ee5796f68d3fcf5b4c98`
(`develop`)

## 結論

**P0-a/b/cとP1（パックピンの記録）はPASS。製品結果はfailed 6/6、
full 0/6、偽成功0。パックの主効果は未識別。**

6runはいずれもfinal acceptance前のphase verifyで正直停止した。
そのためC1〜C4は0/6、C1観測をsourceとする`cli-validation`注入も0/6で、
パックはロード・ピンされたがrendererへ露出しなかった。

baseline `uat-test0725-cli-elev-004`のfull 0/6に対し、本runもfull 0/6
（差0 percentage points）。一方、baselineでC3が拒否したREADME捏造
6件と本runのC3 violation 0件は同じ母集団の判定ではない。本runの0件は
改善ではなくC runtime未到達なので、「捏造→転記」の効果は判定不能とする。

## 1. パックとA/B境界

| 項目 | A: baseline | B: treatment |
|---|---|---|
| campaign | `uat-test0725-cli-elev-004` | `uat-test0730-cli-pack-001` |
| pack | none | `cli-assist@1.0.0` |
| pack hash | — | `sha256:b1dcee70c1a0536954c25639e2d67508d8029328e414aaff030368e7fac844fd` |
| suite形 | cli elevated、6run | baselineと同じ6run |
| planner | `qwen3.6:27b-coding-nvfp4` | 同左 |
| executor | `gemma4:31b-cloud` | 同左 |
| goal / family | stats 3 + filter 3 | 同左 |
| full | 0/6 | 0/6 |
| C到達 | 2/6 | 0/6 |
| C3 violation | 6 | 0（未観測） |
| pack injection | — | 0/6 |

独立変数としてsuiteへ追加したのはpack pinだけである。suite metaと各run
metaの6/6で次の同一値を確認した。

```json
{
  "id": "cli-assist",
  "version": "1.0.0",
  "hash": "sha256:b1dcee70c1a0536954c25639e2d67508d8029328e414aaff030368e7fac844fd",
  "assist_present": true,
  "eval_present": false,
  "assist_schema_version": "commandagent.pack.assist/v0",
  "eval_schema_version": null
}
```

## 2. Campaign境界とpreflight

- campaign id:
  `cli-create-elevated-cli-assist-20260730-124714`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0730_cli_pack`
- suite: `cli-create-elevated-cli-assist`, `profile=cli`, `intent=create`,
  `workspace_mode=empty`
- admission: admitted
- environment interruption: 0
- campaign retry: 0
- human terminal切替: 0

bench preflightはgit clean、HEAD `5653798`、
minimum ancestor `afb6881`、`cargo test`、release build、
binary version `commandagent 0.1.0 5653798 2026-07-30T12:50:46Z`を確認した。
`NODE_ENV=production`で、deviationは0件。

最初のdryなpreflight試行
`cli-create-elevated-cli-assist-20260730-124402`は、追加した実測fixtureの
corpus登録漏れを`corpus_regression`が検出し、product runを1本も開始せず
停止した。`expectations.toml`を登録してpreflightを再実行したもので、
環境中断再実行には数えない。

## 3. Run行列

| run | family | verdict | assurance | C1 | C2 | C3 | C4 | stop class / attribution | 秒 |
|---|---|---|---|---|---|---|---|---|---:|
| `stats_cloud_001` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 1265 |
| `stats_cloud_002` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 1174 |
| `stats_cloud_003` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 838 |
| `filter_cloud_001` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 366 |
| `filter_cloud_002` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 1004 |
| `filter_cloud_003` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 303 |

全runのproduct exitは1、harness statusはcompleted。final acceptance未到達
から`static (cli_probe_not_run)`への投影は契約§4どおりで、failed以外への
漏出は0件。

## 4. C系とpack evidence監査

campaign配下を機械検索した結果:

| evidence | 件数 |
|---|---:|
| `cli-case-binding.json` | 0 |
| `cli-probe.json` | 0 |
| `help-binding.json` | 0 |
| `cli-assurance.json` | 0 |
| `pack-injection-cli-validation.json` | 0 |

したがってC1のexit/stdout/stderr、C2方向別、C3 claim対照、C4再実行の
live実物は本runには存在しない。パックrendererはC1 evidenceの存在後に
だけ動くwithin-phase hookなので、この0件はフックの誤発火ではなく
未露出である。C3の0 violationを改善として数えない。

## 5. 停止原文と帰属

自動分類はknown 6 / UNKNOWN 0。全件をregistryの`process_failure`
（形状既定model、解剖で覆り得る仮置き）へ分類した。

| run | phase / verify原文要点 |
|---|---|
| `stats_cloud_001` | `document-usage`: `python cli/main.py --column value data/sample.csv` → `Column 'value' not found` |
| `stats_cloud_002` | `create-documentation`: 同command → `列名 'value' が見つかりません` |
| `stats_cloud_003` | `implement-cli-tool`: model自作`smoke-check.py`がexpected Sum 1500、実出力Sum 1000を拒否 |
| `filter_cloud_001` | `create-sample-data`: model自作`smoke_check.py`のexpected linesと実sampleが不一致 |
| `filter_cloud_002` | `create-sample-data`: `command_timeout_loop: command timeout sink: grep -q "--count" README.md` |
| `filter_cloud_003` | `setup-sample-data`: model自作`smoke_check.py`のpattern matchが不一致 |

この計測ではdeath anatomyを行っていないため、registry既定以上の帰属裁定は
加えない。

## 6. E-0・実効モデル・scrub

- 自動分類: known 6 / UNKNOWN 0
- 検収シート自給: 6/6
- effective executor/provider:
  `gemma4:31b-cloud / ollama` 6/6
- planner: `qwen3.6:27b-coding-nvfp4 / ollama` 6/6
- run scrub: 6/6 green
- campaign scrub: green、findings 0
- credential pattern: 0

## 7. コスト

epochは`date +%s`基準。

| 境界 | epoch |
|---|---:|
| preflight start | 1785415634 |
| run start | 1785415865 |
| run end / audit end | 1785420815 |

- run合計: 4950秒
- run start→end: 4950秒（逐次実行）
- preflight start→run end: 5181秒

## 8. 合否

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a: 6/6正直終端 | PASS | completed 6/6、product exit 1 |
| P0-b: 契約§4準拠 | PASS | static (`cli_probe_not_run`) 6/6 |
| P0-c: 偽成功ゼロ | PASS | full 0、false full 0 |
| P1: pack pin記録 | PASS | suite + run meta 6/6 |
| パック効果 | 未識別 | injection exposure 0/6 |

## 9. 一次資料SHA-256

- `uat-meta.json`:
  `05b5c8518f97679d38f01af7f1332b0235e08d2bcb2311a951c6c90aa5079dd1`
- `report-skeleton.md`:
  `a61e08808d5b3da7c55e13c5334f90a3db804e16a0a8759348110d49833d514a`
- acceptance sheets:
  - `stats_cloud_001`: `7f7e7c40352ccadfc5c4601b86e9bef40b6d1b7681b7cd8fe855e0a4d662dff0`
  - `stats_cloud_002`: `65135e880cba27980f9f7f5176c608d7deec16e6bcde8b2661b6035309ffbd33`
  - `stats_cloud_003`: `b9aa243e154f5d58cab5a11bc7438dc1c4a3654d163b8038c6177dce77dfce88`
  - `filter_cloud_001`: `c5bd69bcc9ab487984b07b673ff7365e5056baa7d492a65af290b04b61dcaac7`
  - `filter_cloud_002`: `0b664ba66b208143b6e1e427f5d0494e59f0d41c1b9385dadb21d08b9823d0a4`
  - `filter_cloud_003`: `4e1aa0c419252dec2c75e6ebce73be7137e1ef9b9b5085495ace7246e317cea8`
