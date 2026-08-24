# uat-test0726-ingest-elev-005: ingest×create elevated再計測

実施日: 2026-07-28 (JST)

裁定契約: `docs/ingest-profile-contract.md` (fixed 2026-07-25)

計測revision: `15b76541715b8767a3a26f312c1b0277a2148ee0`
(`develop`)

## 結論

**P0-a / P0-b / P0-c / P1-a / P1-bはすべてPASS。final acceptance到達は
4/6、N1〜N5の起動実在は到達4/4、full相当0/6だった。**

INGEST-5の段分解是正はproductionで成立した。到達4runの機械合成計画は
`implement`で`pipeline/main.py`と`output/inspection.json`だけを要求し、
`run`で`python3 -B pipeline/main.py`を実行した後に
`output/records.json`と`output/report.md`を`test -f`で検証した。旧形の
「run成果物をimplementで要求」は0/6であり、4runすべてがstructural gateと
final acceptanceへ到達した。残る2runはモデルがinspectionを作らず、
implement段で理由付きfailedとなった。

到達4runではN1〜N5 evidenceが全数生成された。N1・N4・N5は4/4成立、
N2は全runでrecordsが空のため`claims_absent`、N3は3/4成立した。
`table_cloud_002`では検出候補0件に対してinspectionが架空候補2件の採用を
申告し、N3が勘定不整合と未知候補を拒否した。このrunはfailedへ投影された。
他3runは誤ったセレクタで候補0件・records空となり、契約どおりearned
`partial`、admission=offの表示上限で`static (profile_not_admitted)`となった。
したがって偽成功は0件である。

不備2候補の理由付き除外と和暦→ISO変換は、4runとも実入力の候補を検出できず
**未計測**である。N3の`0 = 0 + 0`を不備2件の除外成功へ読み替えず、N2の
`claims_absent`をsource binding成功へ読み替えない。

## 0. 開始条件

作業開始時点の`develop`先端
`bbc3831baa8417327d073f8999e218f6454959ae`に対する最終確定値:

| workflow | run id | status | conclusion |
|---|---:|---|---|
| CI | `30320170305` | completed | success |
| acceptance | `30320170297` | completed | success |

## 1. INGEST-5段分解是正

### 1.1 段と生成主体

機械合成presetを次の所有権へ是正した。

| 段 | 生成 / 実行主体 | その段で問う物 |
|---|---|---|
| `ingest-implement` | executor model | `pipeline/main.py`, `output/inspection.json` |
| `ingest-run` command | machine | `python3 -B pipeline/main.py` |
| `ingest-run` postcondition | machine | `test -f output/records.json`, `test -f output/report.md` |
| `ingest-structural-gate` | machine | 全成果物の従来構造検証 |
| final acceptance | ingest runtime | N1〜N5 |

既存finalizerはverify stepからmodel ownership用`expected_paths`を除くため、
run出力はrun段のmodel期待へ戻さず、固定command直後のmachine
postconditionで検証する。検証を消したのではなく、生成主体と時系列が一致する
位置へ移した。

elev-004の実測原文をfixtureへ固定した。旧計画ではimplement expectedが
4件だったが、新snapshotでは2件のみとなる。focused testは一時workspaceで
実際にpipelineを実行し、出力生成後のpostcondition成立と、出力を作らない
pipelineで同postconditionがfailedになる両側を確認する。

### 1.2 帰属訂正・class・floor audit

elev-004レポートへ、model 6/6からmachine 6/6へのレビュー裁定を追記した。
一次資料は、モデルが直接書く`pipeline/main.py` 6/6・inspection 4/6に対し、
runが生むrecords/report 0/6、run到達0/6、16〜24秒の同形停止である。
INGEST-4のレビュー指示自体がrun成果物をimplement期待へ置いた事実も記録した。

class
`ingest_preset:runtime_outputs_bound_before_run`
をmachine / first_seen=`uat-test0726-ingest-elev-004` /
resolved_by=`INGEST-5`として登録した。実campaignへclassifierを再適用し、
elev-004 6/6が同classへ一致することを確認した。

常設floor auditは22床のまま、段×期待成果物×生成主体の整合を独立行として
追加した。planner由来・open・unknownは引き続き0床である。

### 1.3 権限付きfull verification

| check | 結果 |
|---|---|
| `cargo fmt --all -- --check` | green |
| `cargo clippy --all-targets -- -D warnings` | green |
| `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --all-targets` | **1842 passed / 0 failed / 30 ignored** |
| ingest plan synthesis | 5/5 |
| ingest manifest | 3/3 |
| ingest conformance | 2/2 |
| generality guardrails | 9/9 |
| scaffold unittest | 3/3 |
| classify unittest | 4/4 |
| Ruff | green |

lib/integration内訳は1685 passed / 15 ignoredと157 passed / 15 ignored。
growth tripwire baselineは変更していない。

## 2. Suite・preflight

### 2.1 実効構成

- suite: `ingest-create-elevated`
- profile / intent: `ingest / create`
- workspace mode: `sourced`
- planner config: `qwen3.6:27b-coding-nvfp4 / ollama`
- executor: `gemma4:31b-cloud / ollama`
- admission: `off`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0726_ingest_elev5`
- campaign: `ingest-create-elevated-20260728-043740`
- retry / interruption: なし

| family | asset | sha256 |
|---|---|---|
| list | `events-list.html` | `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07` |
| table | `events-table.html` | `394b03ccd7ac141a2677c55d9a2059034ea2c7a92656b466201ee40b7050cddd` |

input sha256一致とzero-exit precheckは6/6。`run_start` 6/6とexecutor
provider turn 77/77で実効model/providerが一致した。planner configも6/6一致し、
profile presetが計画を合成したためplanner provider turnは0件だった。

### 2.2 preflight

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD | `15b7654 Move ingest outputs behind pipeline execution` |
| minimum ancestor | `78b95ce` verified |
| bench内`cargo test` | exit 0 |
| release build | exit 0 |
| installed binary | `commandagent 0.1.0 15b7654 2026-07-28T04:40:18Z` |
| built / installed sha256 | `af8f7c674a41f6ba2338a559e32b5ed0ec6a63a7e6bb7b87b4680df539082d1b` / 同一 |
| `NODE_ENV` | `production` |

## 3. production段順序の実測

final acceptance到達4runで次の3eventが同形に発火した。

```json
{"event":"ingest_plan_synthesized",
 "phase_id":"ingest-implement",
 "expected_paths":["pipeline/main.py","output/inspection.json"],
 "verify":[]}
```

```json
{"event":"ingest_plan_synthesized",
 "phase_id":"ingest-run",
 "expected_paths":[],
 "verify":["python3 -B pipeline/main.py",
           "test -f output/records.json",
           "test -f output/report.md"]}
```

```json
{"event":"ingest_plan_synthesized",
 "phase_id":"ingest-structural-gate",
 "expected_paths":[],
 "verify":["anvil-ingest-check:phase_structure"]}
```

`phase_verification_result.ok=true`は到達4runでimplement・run・structural
gate・final acceptanceの全箇所に存在する。list 002/003はimplementのみを合成し、
`output/inspection.json`不在を正直に停止した。どのrunにも旧implement期待の
`output/records.json` / `output/report.md`は現れなかった。

## 4. Run行列

`—`はfinal acceptance未到達によるN未実行。

| run | family | verdict | 表示assurance | N1 | N2 | N3 | N4 | N5 | 停止形 / 監査帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---|---:|
| `list_cloud_001` | list | partial | static (`profile_not_admitted`) | pass | claims_absent | pass | pass | pass | completed / model内容 | 99 |
| `list_cloud_002` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | inspection不在 / model | 47 |
| `list_cloud_003` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | inspection不在 / model | 20 |
| `table_cloud_001` | table | partial | static (`profile_not_admitted`) | pass | claims_absent | pass | pass | pass | completed / model内容 | 28 |
| `table_cloud_002` | table | failed | failed (`ingest_assurance_failed`) | pass | claims_absent | **failed** | pass | pass | N3拒否後read-only repair / model | 29 |
| `table_cloud_003` | table | partial | static (`profile_not_admitted`) | pass | claims_absent | pass | pass | pass | completed / model内容 | 43 |

全runのharness statusは`completed`。product exitはpartial 3runが0、failed
3runが1。panic、理由なし終端、環境中断、retry、偽成功は0件。

停止原文:

```text
phase ingest-implement failed: artifact_follow_through_exhausted:
missing expected paths: output/inspection.json;
artifact_stagnation_feedback_count: 2; incomplete
```

```text
phase ingest-implement failed: artifact_follow_through_exhausted:
missing expected paths: output/inspection.json;
artifact_stagnation_feedback_count: 1; incomplete
```

`table_cloud_002`はN3 failureを受けたfinal repairで
`model_stagnation:read_only_loop`となった。N3のfailed投影は保持された。

## 5. N1〜N5実物監査

### 5.1 起動実在

| evidence | 実在 |
|---|---:|
| final acceptance到達 | 4/6 |
| selector / candidate freeze | 4/4 |
| ingest probe (N1) | 4/4 |
| source binding (N2) | 4/4 |
| candidate accounting (N3) | 4/4 |
| format schema (N4) | 4/4 |
| rerun consistency (N5) | 4/4 |
| ingest assurance projection | 4/4 |

従ってP1-aの「到達runでN1〜N5実行」は4/4で成立した。

### 5.2 セレクタ宣言と実行前凍結

`list_cloud_001`原文:

```json
{"capability_id":"ingest_candidate_freeze",
 "selector":{"kind":"css","value":"tr.event-row"},
 "record_format":{"fields":[
   {"name":"name","normalizations":["identity"],"type":"string"},
   {"name":"date","normalizations":["japanese_date_to_iso"],"type":"string"},
   {"name":"location","normalizations":["identity"],"type":"string"},
   {"name":"source_file","normalizations":["identity"],"type":"string"}]},
 "snapshots":[{"path":"data/snapshots/events-list.html",
               "bytes":2485,"fnv1a64":"bd5dd2ba4b62a316"}],
 "candidates":[]}
```

実行前にselector・record format・snapshot hash・candidate setを凍結した。
ただし実HTMLのlist候補は`article.event`、table候補は`tbody tr`であり、
到達4runの`tr.event-row` / `.event-item` / `div.event-item` /
`div.event-card`はいずれも0候補だった。

### 5.3 N1実実行

`list_cloud_001`原文:

```json
{"capability_id":"ingest_probe","status":"pass","ok":true,
 "execution":{"capability_id":"pipeline_probe","status":"pass","ok":true,
 "outcome":"exited","command":["python3","-B","pipeline/main.py"],
 "duration_ms":111,"exit_code":0,
 "stdout":{"text":"","captured_bytes":0,"total_bytes":0,"truncated":false},
 "stderr":{"text":"","captured_bytes":0,"total_bytes":0,"truncated":false},
 "isolation":{"level":"workspace_cwd_env_allowlist_bounded_offline_policy",
              "workspace_cwd":true,"environment_allowlist":true,
              "process_group":true,"bounded_timeout_ms":30000,
              "offline_policy_applied":true,
              "network_namespace_enforced":false}}}
```

4runのprobeはexit 0、所要111 / 134 / 118 / 147ms。records・report・
inspectionのartifact hashも各evidenceへ記録された。

### 5.4 N3勘定とsilent drop監視

成立3runの原文は次の形:

```json
{"capability_id":"ingest_candidate_accounting","status":"pass","ok":true,
 "selector":{"kind":"css","value":"tr.event-row"},
 "detected":0,"accepted":0,"excluded_by_reason":{},
 "equation":"0 = 0 + 0","candidate_ids":[],"failure_kinds":[]}
```

これは凍結した候補集合に対する勘定整合であって、実HTMLの不備2件を
理由付き除外した証拠ではない。契約の恒久scope外である網羅性をN3へ足さず、
今回の不備2件/silent drop実弾は未計測と記録する。

`table_cloud_002`は0候補に対して架空の2採用をinspectionで申告し、N3が次を
全件拒否した:

```json
{"capability_id":"ingest_candidate_accounting","status":"failed","ok":false,
 "selector":{"kind":"css","value":"div.event-item"},
 "detected":0,"accepted":2,"excluded_by_reason":{},
 "equation":"0 = 2 + 0","candidate_ids":[],
 "failure_kinds":[
   "accounting_violation:equation:detected=0:accepted=2:excluded=0",
   "accounting_violation:record_indices:expected={}:observed={0, 1}",
   "candidate_set_violation:unknown_candidate:data/snapshots/events.html#0",
   "candidate_set_violation:unknown_candidate:data/snapshots/events.html#1"]}
```

これは候補集合の縮小・架空候補の採用をfullへ通さないlive拒否である。

### 5.5 N2・N4・N5

4runのrecordsはすべて`[]`であり、N2原文は:

```json
{"capability_id":"ingest_source_binding","status":"claims_absent","ok":true,
 "records_path":"output/records.json","bindings":[],"failure_kinds":[]}
```

bindings、normalization trace、nearest_miss、violationはいずれも0件。
宣言に`japanese_date_to_iso`は存在したが、field値がないため和暦→ISOの
値保存・宣言・記録の三条件はlive未計測である。

N4は4/4でdeclared fields
`date, location, name, source_file`、record count 0を検証した。N5は4/4で
`output/records.json`と`output/report.md`の再実行一致を検証した。

### 5.6 assuranceとadmission cap

earned assurance:

- list 001 / table 001 / table 003: N1 pass + N2 claims_absentにより
  `partial`。admission=offのため表示は
  `static (profile_not_admitted)`。
- table 002: N3違反により`failed (ingest_assurance_failed)`。
- list 002 / 003: N1未実行により`static (ingest_probe_not_run)`。

offのstatic capはpartial 3runへ実際に適用された。failedをstaticへ隠すことは
なく、N1未実行とprofile未admitのreasonも区別された。§assurance写像準拠である。

## 6. 死因の機械 / モデル帰属

### 6.1 自動分類

report skeletonの分類:

- registry known: 3/6
- UNKNOWN: 3/6
- `process_failure` / model: list 002, list 003
- `model_stagnation_read_only` / model: table 002

UNKNOWN 3件は失敗classを持たない正直なpartial終端であり、新failure classでは
ない。failure run内ではknown 3/3である。

### 6.2 人手監査

- list 002/003: fixed implement期待はモデルが書く2物だけであり、
  `pipeline/main.py`は存在するがinspectionを作らなかった。段分解gapの再発
  ではなくmodel delivery failureと裁定する。
- table 002: machine N3が架空候補2件を正しく拒否し、その後モデルが
  read-only repairで停滞した。近因model、machine検証は正常。
- partial 3件: modelが実入力に一致しないselectorを宣言し、空recordsを生成。
  N2 claims_absentによりfullを得ていない。failure classは付けない。

新machine classは0件。旧
`ingest_preset:runtime_outputs_bound_before_run`はproductionで0/6再発だった。

## 7. E-0・scrub・コスト

### 7.1 E-0

| 項目 | 結果 |
|---|---|
| 自動分類 | known 3 / UNKNOWN 3（failure runはknown 3/3） |
| acceptance sheet自動生成 | **6/6** |
| N2 nearest_miss | 0 |
| N3 violation | 4件 / 1run |
| calibration collector追記 | 0 |

N2/C3型nearest_missが発生しなかったためcollector流出はない。

### 7.2 資格情報scrub

benchのrun別scrubは6/6 `ok=true / findings=[] / allow=[]`。
campaign全体を再走査した結果も:

```json
{"ok":true,"findings":[]}
```

raw console、`.anvil` runtime state、workspace途中物はcommitしない。
repoへ保存するのはscrub済み集計と人手監査レポートだけである。

### 7.3 date +%sコスト

| 区間 | epoch / 秒 |
|---|---:|
| preflight start | `1785213460` |
| preflight completed / run start | `1785213635` |
| last run end | `1785213901` |
| audit + scrub end | `1785214008` |
| preflight | 175秒 |
| formal run合計 | 266秒 |
| list族 | 166秒 |
| table族 | 100秒 |
| preflight + formal run | 441秒 |
| preflight start → audit/scrub end | 548秒 |

## 8. 事前合否

| criterion | 判定 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | completed 6/6、partial 3・failed 3、全件理由あり |
| P0-b §assurance | **PASS** | earned partial→off static 3、N1未実行static 2、N3 failed 1 |
| P0-c 偽成功ゼロ | **PASS** | full 0、N3違反のfailed保持、claims_absentはpartial |
| P1-a 到達runでN1〜N5 | **PASS** | evidence実在4/4 |
| P1-b sheet 6/6 | **PASS** | 自動生成6/6 |

記録値:

- final acceptance到達: 4/6
- full相当率: 0/6 (0%)
- partial相当: 3/6
- N1: 4/4 pass
- N2: 4/4 claims_absent、binding実弾0
- N3: 3/4 pass、1/4 failed（4 violation）
- N4 / N5: 4/4 pass
- 不備2件の理由付き除外: 未計測
- 和暦→ISO変換: 未計測
- family差: list到達1/3・partial 1、table到達3/3・partial 2・N3 failed 1
- 新class: 0

## 9. 一次資料

外部campaign:

```text
/Users/maenokota/share/work/localwork/commandagent_mvp/01/
test0726_ingest_elev5/ingest-create-elevated-20260728-043740
```

| 資料 | sha256 |
|---|---|
| `uat-meta.json` | `63f14a42de884896fd945a4cc42c5c20085ed0687a87dbee082f2ab3d9738776` |
| `report-skeleton.md` | `94c72f19eed60a59e82e5ba94f2dccff57d25d400c24aad87a5fa4f63cec8f1f` |
| list acceptance sheet 001/002/003 | `4a1b47a25713b4da0a8afad925fe78b53c756270849b46ad0e02dec151e4a70b` / `7264063a02941191268e5c2bba7d693f825830b5b6c491f8ee495f910376ff73` / `62801e2733555874f2fed2fe45153c4f9a63f007235071a9f89362446d72e6e1` |
| table acceptance sheet 001/002/003 | `ef2ffb0a3c4387166677be782cfd34badbb25d2b8d3a4a81690951431e8afe26` / `8c5a517594aaff176f28445f506a058ad4f3b7314f22f22f5f0d4354003ae71a` / `74b6a1b93389b69e41a482b90aee84ca27db0020203865b6a43c550805f0a6e8` |

機械可読な集計は`evidence/campaign-summary.json`へ保存する。
