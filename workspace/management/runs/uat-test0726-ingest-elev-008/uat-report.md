# uat-test0726-ingest-elev-008: ingest×create elevated再計測

実施日: 2026-07-29 (JST)

裁定契約: `docs/ingest-profile-contract.md`
(`fixed v0.1`, 2026-07-28)

計測revision: `c1157e03368f0b25915d71cb883b4822c2b96103`
(`develop`)

## 結論

**P0-a / P0-b / P0-c / P1-a / P1-bはすべてPASS。ingest初のfull相当を
4/6（66.7%）で観測した。**

機械は6/6でselector宣言後・pipeline実装/実行前に候補10件を凍結し、
正準candidate ID全件を`implement-ingest-delivery` promptへ注入した。
全モデルが接頭辞込みIDを字義どおり使用し、elev-007のpath prefix脱落は
6/6から0/6へ減った。N3は6/6 pass、N2は4/6 passとなった。

list 3runは`8/3(月)`と文書見出しの`2026年`を
`document_year_context`で`2026-08-03`へ補完し、候補内断片と文書文脈断片の
双方をsource path・byte位置つきで記録した。契約v0.1条件(c)のlive初成立は
3/3 relevant runである。

table 001/002は日付欠落候補を除外せず空文字dateのrecordを作った。N3の
勘定式は10=10+0で整合したが、N2は
`source_binding_violation:record=8:field=date:value=`として正しく拒否した。
table 003は同候補を理由付き除外してfull相当となった。従って、誠実性の
層分離（N3=勘定、N2=値の実在束縛）が実弾で成立している。

## 0. 開始条件

開始時`develop`
`cce04ebf8cf3423673e241169cdb39805a42ded4`の最終確定値:

| workflow | run id | status | conclusion |
|---|---:|---|---|
| CI | `30346559886` | completed | success |
| acceptance | `30346559888` | completed | success |

## 1. INGEST-8実装

### 1.1 凍結とprompt位置

従来はselectorとpipelineを同じimplement stepで作り、freezeはfinal
acceptanceで初めて実行していたため、生成promptへ凍結IDを返せなかった。
implement phaseを次のproduction順へ分けた。

1. `declare-ingest-inspection`: 実構造材料からselectorとrecord formatを暫定宣言
2. machine freeze: selectorに対する候補集合・正準IDをevidenceへ固定
3. `implement-ingest-delivery`: 正準ID全件をpromptへ注入し、pipelineと最終inspectionを作成
4. run phase: `python3 -B pipeline/main.py`
5. structure gate → final acceptance N1〜N5

production runner testは、単なるrender helperでなく実際の`run_step_plan`上で
2つの注入event、freeze evidence、次段promptのID #0〜#9を確認した。

注入event原文:

```json
{"candidate_count":10,
 "candidate_ids":[
   "data/snapshots/events-list.html#0",
   "data/snapshots/events-list.html#1",
   "...",
   "data/snapshots/events-list.html#9"],
 "event":"ingest_candidate_ids_injected",
 "freeze_evidence_path":"evidence/ingest-candidate-freeze.json",
 "frozen_before_run":true,
 "profile":"ingest",
 "selector":{"kind":"css","value":"article.event"},
 "step_id":"implement-ingest-delivery"}
```

### 1.2 決定的ID解決

N2/N3は次の順でIDを解決し、`provided_id / status / matched_ids /
resolved_id`をevidence化する。

1. 正準IDのexact一致
2. `/{provided_id}`が凍結集合でただ1件に一致する`unique_suffix`
3. 0件は`not_found`、複数は`ambiguous_suffix`として未解決

elev-007実測fixtureは接頭辞欠落参照を`unique_suffix`で束縛し、N2の
年補完とN3の10=9+1をともにpassさせた。2候補に一致する曖昧形と偽IDは
未解決・violationのまま固定した。live elev-008では語彙注入が先に効き、
N2 224 fieldとN3 60 candidate参照はすべて`exact`だった。

### 1.3 class・台帳

`ingest_candidate_accounting:candidate_id_path_prefix_omitted`を
近因model・設計根因machine（機械発行語彙の未配布）へ精密化し、INGEST-8
解消を記録した。台帳では「機械発行語彙は機械が配る。照合は寛容化でなく、
exactまたは一意suffixの決定的解決で受ける」をDATA-1系の完結形とした。

## 2. 権限付きfull verification

| check | 結果 |
|---|---|
| `cargo fmt --all -- --check` | green |
| `cargo clippy --all-targets -- -D warnings` | green |
| `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --all-targets` | **1859 passed / 0 failed / 30 ignored** |
| Python 3.12 scripts unittest | **54 passed / 0 failed** |
| Ruff | green |

Rust内訳はlib 1701 passed / 15 ignored、integration 158 passed /
15 ignored。growth tripwire baselineは変更していない。

## 3. Suite・preflight

- suite: `ingest-create-elevated`
- suite sha256:
  `f7e9c448defd833c353b0e4a8f28b8a9adba19595c438b648cc50b6102325146`
- profile / intent: `ingest / create`
- workspace mode: `sourced`
- planner config: `qwen3.6:27b-coding-nvfp4 / ollama`
- executor: `gemma4:31b-cloud / ollama`
- admission: `off`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0726_ingest_elev8`
- campaign: `ingest-create-elevated-20260728-153400`
- retry / interruption: なし

| family | asset | sha256 |
|---|---|---|
| list | `events-list.html` | `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07` |
| table | `events-table.html` | `394b03ccd7ac141a2677c55d9a2059034ea2c7a92656b466201ee40b7050cddd` |

input hash一致とzero-exit precheckは6/6。`run_start`は6/6、
executor provider turnは54/54が`gemma4:31b-cloud / ollama`だった。
planner configは6/6一致し、profile presetのためplanner turnは0件。

| preflight項目 | 結果 |
|---|---|
| git status | clean |
| HEAD | `c1157e0 Inject frozen ingest candidate IDs` |
| minimum ancestor | `78b95ce` verified |
| bench内`cargo test` | exit 0 |
| release build | exit 0 |
| installed binary | `commandagent 0.1.0 c1157e0 2026-07-28T15:36:47Z` |
| built / installed sha256 | `401de3d0028be5d06b8ef2654e20e82a753378793f2a0b9971e4a3fc458d4db5` / 同一 |
| `NODE_ENV` | `production` |

## 4. Run行列

`内部assurance`はN1〜N5から得た契約値、`表示`はadmission off cap適用後。

| run | family | verdict | 内部assurance | 表示 | N1 | N2 | N3 | N4 | N5 | 秒 |
|---|---|---|---|---|---|---|---|---|---|---:|
| `list_cloud_001` | list | complete | **full** | static (`profile_not_admitted`) | pass | pass | pass | pass | pass | 23 |
| `list_cloud_002` | list | complete | **full** | static (`profile_not_admitted`) | pass | pass | pass | pass | pass | 34 |
| `list_cloud_003` | list | complete | **full** | static (`profile_not_admitted`) | pass | pass | pass | pass | pass | 26 |
| `table_cloud_001` | table | failed | failed | failed (`ingest_assurance_failed`) | pass | **failed** | pass | pass | pass | 29 |
| `table_cloud_002` | table | failed | failed | failed (`ingest_assurance_failed`) | pass | **failed** | pass | pass | pass | 34 |
| `table_cloud_003` | table | complete | **full** | static (`profile_not_admitted`) | pass | pass | pass | pass | pass | 46 |

全runのharness statusは`completed`。成功4runはproduct exit 0、拒否2runは
product exit 1。panic、環境中断、retry、理由なし終端、偽成功は0件。

## 5. N1〜N5実物監査

### 5.1 起動・凍結・ID注入

| evidence / event | 実在 |
|---|---:|
| snapshot structure injection | 6/6 |
| pre-run candidate ID injection | 6/6 |
| candidate freeze | 6/6 |
| final acceptance / profile probe | 6/6 |
| N1〜N5各evidence | 各6/6 |
| ingest assurance projection | 6/6 |

selectorはlist 3runが`article.event`、table 3runが
`table tbody tr`。各runの凍結候補は10件だった。注入された正準IDの使用結果:

| 対象 | 参照数 | exact | unique suffix | 曖昧 / 不一致 |
|---|---:|---:|---:|---:|
| N2 field bindings | 224 | 224 | 0 | 0 |
| N3 accepted/excluded | 60 | 60 | 0 | 0 |

N3原文例:

```json
{"capability_id":"ingest_candidate_accounting",
 "status":"pass","ok":true,
 "selector":{"kind":"css","value":"article.event"},
 "detected":10,"accepted":9,
 "excluded_by_reason":{"missing/invalid date":1},
 "equation":"10 = 9 + 1",
 "candidate_id_resolutions":[{
   "provided_id":"data/snapshots/events-list.html#0",
   "status":"exact",
   "matched_ids":["data/snapshots/events-list.html#0"],
   "resolved_id":"data/snapshots/events-list.html#0"}],
 "failure_kinds":[]}
```

### 5.2 N1・N4・N5

N1は全runで`python3 -B pipeline/main.py`をworkspace cwd・env allowlist・
process group・offline policy・30秒上限で実行し、exit 0だった。実行時間は
list 118 / 120 / 115 ms、table 185 / 121 / 116 ms。

N4は全runでJSON parseと宣言schemaをpass。record countは
list 9 / 9 / 9、table 10 / 10 / 9。N5は全runで
`output/records.json`と`output/report.md`の再実行一致を確認した。

### 5.3 N2文書文脈の両断片記録

list 001原文（list 002/003も同じ断片・位置）:

```json
{"record_index":1,
 "candidate_id":"data/snapshots/events-list.html#1",
 "candidate_id_resolution":{
   "provided_id":"data/snapshots/events-list.html#1",
   "status":"exact",
   "matched_ids":["data/snapshots/events-list.html#1"],
   "resolved_id":"data/snapshots/events-list.html#1"},
 "source_path":"data/snapshots/events-list.html",
 "candidate_byte_start":434,"candidate_byte_end":656,
 "field":"date","output_value":"2026-08-03",
 "declared_normalizations":["japanese_date_to_iso","document_year_context"],
 "raw_source":"8/3(月)","normalized_source":"2026-08-03",
 "transformations":["japanese_date_to_iso","document_year_context"],
 "candidate_fragment":{
   "source_path":"data/snapshots/events-list.html",
   "byte_start":533,"byte_end":541,"raw_source":"8/3(月)"},
 "document_context":{
   "source_path":"data/snapshots/events-list.html",
   "byte_start":87,"byte_end":94,"raw_source":"2026年"},
 "matched":true,"nearest_miss":null}
```

両断片位置記録はlist 3/3、計3件。候補間の継ぎ合わせ、文書にない値、
日付ずらしは0件だった。

### 5.4 日付欠落・violation原文

日付欠落candidateを理由付き除外したのはlist 3runとtable 003の4/6。
table 001/002はcandidate #8をacceptedし、空文字dateを出力した。
両runのN2 failure arrayはbyte-identical
(`sha256=08e7cef3ed1e28edc19bffd41605bbadddd31c3c2a47d3613c5e03764b77eee9`)。

全violation原文:

```text
table_cloud_001: source_binding_violation:record=8:field=date:value=
table_cloud_002: source_binding_violation:record=8:field=date:value=
```

該当binding原文:

```json
{"record_index":8,
 "candidate_id":"data/snapshots/events-table.html#8",
 "candidate_id_resolution":{
   "provided_id":"data/snapshots/events-table.html#8",
   "status":"exact",
   "matched_ids":["data/snapshots/events-table.html#8"],
   "resolved_id":"data/snapshots/events-table.html#8"},
 "source_path":"data/snapshots/events-table.html",
 "candidate_byte_start":1904,"candidate_byte_end":2082,
 "field":"date","output_value":"",
 "raw_source":null,"normalized_source":null,
 "transformations":[],"matched":false,"nearest_miss":null}
```

近因・帰属はmodel。sourceの空欄をrecordへ採用したため判定は正当で、新しい
machine classではない。repair後の二次停止classは
`model_stagnation_read_only` 2/2。

## 6. assurance・E-0・scrub

N1〜N5が全passした4runの内部assuranceはfull。admission offのためterminal
表示を`static (profile_not_admitted)`へcapした。N2違反の2runはfailedを優先し、
off capで隠していない。契約§assurance準拠、偽成功0である。

| E-0項目 | 実測 |
|---|---|
| 自動分類 | known 2 / UNKNOWN 4 |
| UNKNOWN内訳 | 完了run 4（停止classなし。新死因ではない） |
| acceptance sheet | 6/6 (100%) |
| per-run scrub | 6/6 green |
| campaign全体scrub | green、finding 0 |
| retry / interruption | 0 / 0 |

## 7. コスト

`date +%s`基準:

| 区間 | epoch / 秒 |
|---|---:|
| preflight start | `1785252840` |
| preflight complete / run start | `1785253024` |
| run end | `1785253216` |
| evidence audit・campaign scrub end | `1785253275` |
| preflight | 184秒 |
| formal run合計 | 192秒 |
| list / table | 83秒 / 109秒 |
| preflight開始→audit・scrub完了 | 435秒 |

## 8. 合否

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | complete 4、理由ありfailed 2 |
| P0-b §assurance準拠 | **PASS** | full相当4をoff cap、N2違反2はfailed優先 |
| P0-c 偽成功ゼロ | **PASS** | 空日付2件をN2が拒否 |
| P1-a 到達runでN1〜N5 | **PASS** | 全evidence 6/6 |
| P1-b sheet 6/6 | **PASS** | 6/6生成・scrub green |

記録値:

- full相当: **4/6 (66.7%)**
- N1 / N2 / N3 / N4 / N5:
  **6/6 / 4/6 / 6/6 / 6/6 / 6/6**
- pre-run凍結ID注入: **6/6**
- path prefix脱落: **0/6**
- candidate detected: **全run 10**
- 日付欠落の理由付き除外: **4/6**
- v0.1年補完の両断片位置記録: **3/3 relevant run**
- violation: **2件**（同一空日付）
- 新class: **0**

資格情報・token・password・private keyについて、bench per-run 6/6とcampaign
全体のscrubを実施しfinding 0。repoへ保存するのは本レポートと集約済み
`evidence/campaign-summary.json`だけで、raw log、workspace、`.anvil/`
runtime stateは保存しない。
