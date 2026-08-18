# 結果サマリ

- CM-4の並行4件は、当初の「model成果物4件」という読みを訂正する。schema error 3件は`machine: stale`、core manifest path 1件は`model`だった。
- 4 workspaceへ供給されたschemaはすべて`community.app-spec/v0.1`、SHA-256 `80e4cb41…7ac7e0b`で一致し、pinも一致した。競合・供給差・stale workspaceはない。
- 外側campaign binaryは`26b4705a`/SHA `03159d12…ccdbfa`でv0.1を期待したが、計画内の裸の`commandagent`はPATH上の`178e09c2`/SHA `87861649…57784e`へ解決し、旧v1を期待した。これが3件の直接原因だった。
- parallel probeの子process PATH先頭をピン済みcampaign binaryのdirectoryへ固定し、stale PATH負例を追加した。同一binary・同一suiteで4並行を再実行し、隔離0汚染と`final_acceptance=full_success` 4/4を同時確認した。
- 再実行のheadless `verdict`は4件とも`partial`だが、理由は`completion_contract_not_bound`。既存正本の`run_stop.final_acceptance_status=full_success`、runtime acceptance pass、release gate passと区別して記録し、`partial`を`full`へ改名していない。

# CM-4 parallel attribution

## 1. 供給schemaとverifier世代

4 workspaceの供給物はbyte一致した。

```yaml
schema_version: community.app-spec/v0.1
fields:
  entities: list
  views: list
  actions: list
```

| 供給物 | SHA-256 |
|---|---|
| `schema/app-spec.schema.yaml` | `80e4cb41eeb0f60eb04640e2ac8beac7d1414e7f5a9aa9fa563fd08d17ac7e0b` |
| `schema/app-spec.schema.sha256` | `515ab53e598e473e89447616c3f14550a3d999896ac4f32f3f65d645959955b0` |
| `core.sha256sums` | `f23e87be97f9442cc20132eceed57c0149ffe2451f739b63ead83fc656c1e0af` |
| `core/README.md` | `b1ecd4495d9f057b5c3542f4b120a8b8176f0e63f7a900f3c51492735020f16c` |

AppSpec rootは`entities/views/actions/validations/computed/permissions/minIdentity`の閉集合であり、schema側metadataの`schema_version`はAppSpecへ書かないのが正形である。campaign binaryのverifier sourceは供給schemaに`community.app-spec/v0.1`を要求する。

一方、失敗時のprocess environmentで裸の`commandagent`は次へ解決した。

```text
/Users/<user>/.local/bin/commandagent
commandagent 0.1.0 178e09c2 2026-08-17T22:23:46+09:00
sha256=87861649b3ffc5e850db52ecbb293d6c16f23d919e93754beb6ade943775784e
```

このbinaryはv0.1供給を`community_schema_version_invalid`で拒否し、同じ構造の旧v1供給ではschema gateを通過して後段の`community_build_inputs_missing`まで進んだ。リポジトリ履歴上もv0.1改訂直前のverifier literalは`community.app-spec/v1`である。外側campaign binaryは次で、同じv0.1供給を受理する。

```text
commandagent 0.1.0 26b4705a 2026-08-18T19:57:09+09:00
sha256=03159d12811aa3385d877b1d81ad7f2fdd9942e1b9af1c24314b4ece63ccdbfa
```

## 2. 三点対照

| run | 供給schema実物 | 生成specの記載 | 実際に起動したverifier期待 | 分類 |
|---|---|---|---|---|
| warikan_001 | v0.1 / `80e4…e0b` | 先頭`entities:`、`schema_version`なし（正形） | PATH stale binary: v1 | **machine: stale** |
| warikan_002 | v0.1 / `80e4…e0b` | 初期は先頭`entities:`。repairが`entities:\n`（10 bytes）を`schema_version: community.app-spec/v0.1\nentities:\n`（50 bytes）へ置換 | PATH stale binary: v1 | **machine: stale**が直接原因。repair後end-stateにはsecondary model errorあり |
| warikan_003 | v0.1 / `80e4…e0b` | 先頭`entities:`、`schema_version`なし（正形） | PATH stale binary: v1 | **machine: stale** |
| warikan_004 | v0.1 / `80e4…e0b` | write前にReadが失敗しspecなし | campaignはv0.1。verifier起動前のtool path failure | **model** |

warikan_001と003の保存specをcampaign binaryでoffline再検証すると、いずれも`verdict=full`だった。warikan_002はstale errorを見た後のrepairでAppSpec rootへschema metadataを追加したため、end-stateをcampaign binaryで再検証すると`community_spec_closed_vocabulary`となる。これはstale binaryという初発machine床が作ったrepair窓のsecondary model errorであり、初発3件の帰属をmodelへ戻す根拠にはしない。

## 3. core manifest pathの三点対照

供給pathと内容は全runで次だった。

```text
path: core.sha256sums
content: b1ecd4495d9f057b5c3542f4b120a8b8176f0e63f7a900f3c51492735020f16c  core/README.md
```

verifierは`<workspace>/core.sha256sums`（hidden formがあれば`.community/core.sha256sums`）を読む。warikan_004でモデルがRead toolへ渡したpath原文は次だった。

```text
core.sha256sums"}}]}ર્ય հես invalid quote path. just glob and ls? Use bash find. available schema. Also perhaps commanda
```

供給path、manifest hash、verifier期待pathは一致し、壊れた自然言語断片はtool argumentにだけ存在する。従って`model: malformed_read_path`を維持し、path fail-closedは補修しない。

## 4. setup返済

`cm4_parallel_probe.py`は、子process environmentのPATH先頭へcampaign binaryの親directoryを置き、`shutil.which("commandagent")`の解決pathがcampaign binaryそのものかを0run目に検査する。summaryへ解決pathとSHAを記録する。stale binaryを先に置いたbase PATHでもcampaign binaryが勝つ負例を追加した。

通常のgolden harnessは既にcampaign `bin/`をPATH先頭へ置いており、欠落していたのはCM-4の専用parallel probeだけだった。

## 5. 同一条件4並行再実行

- campaign binary: 元runと同じ`26b4705a` / `03159d12…ccdbfa`
- suite: 元runと同じsealed warikan / `215abae7…6cd9e0c`
- planner/executor: qwen3.6 / Luna、think未指定
- 唯一のsetup差: nested `commandagent`のピン済みPATH binding

| run | exit | runtime acceptance | final acceptance | release gate | headless verdict | duration |
|---|---:|---|---|---|---|---:|
| warikan_001 | 0 | pass | full_success | pass | partial | 542.37秒 |
| warikan_002 | 0 | pass | full_success | pass | partial | 498.62秒 |
| warikan_003 | 0 | pass | full_success | pass | partial | 419.35秒 |
| warikan_004 | 0 | pass | full_success | pass | partial | 635.83秒 |

headless partialは4件とも`completion_contract_not_bound`によるassurance投影で、品質gate失敗ではない。最終受理正本は4/4 `full_success`。isolationはworkspace/state/run ID unique、owner path binding 4/4、foreign path 0、cross contamination 0、scrub 4/4 passだった。

| 指標 | 再実測 |
|---|---:|
| individual p50 / p95 | 520.50 / 621.81秒 |
| makespan | 635.85秒 |
| observed sequential sum / makespan | 3.297倍 |
| OpenAI cost | $0.00507942 |

## 6. 基底率との対照

golden-008/matrix-001の同じqwen3.6/Luna基準Aは全体10/12 full、warikan 4/4 fullだった。旧parallel窓の0/4は、machine stale 3件と単発model path error 1件が混ざった値であり、モデル基底率との比較値として無効だった。setup返済後は同じ4入力でfinal acceptance 4/4 full_successとなり、基準warikan 4/4と整合した。

warikan_004のmodel path errorは再実行で0/4だったが、n=4の非再現だけで消滅を主張しない。classはmodel帰属のまま保持し、既知署名として監視する。
