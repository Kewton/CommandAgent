# uat-test0726-ingest-elev-006: ingest×create elevated再計測

実施日: 2026-07-28 (JST)

裁定契約: `docs/ingest-profile-contract.md` (fixed 2026-07-25)

計測revision: `6628442c4ef36821bbad75018fe2f48ef86d4fad`
(`develop`)

## 結論

**P0-a / P0-b / P0-c / P1-bはPASS、P1-aはFAIL。full相当は0/6だった。**

INGEST-6の実構造材料注入はproductionで6/6発火した。listには
`events-list.html`の先頭12行と反復`article`候補2 window、tableには
`events-table.html`の先頭12行と反復`tr`候補2 windowが、ファイル名と全上限を
伴って注入された。elev-005で到達4/4が実構造と不一致だったselectorは、
今回6/6が実HTMLに存在するselectorへ変わった。モデルの成果物上の候補分布も
全runで10候補相当となり、材料注入の主目的は成立した。

final acceptanceは6/6で起動したが、validな複合CSS selector
`table tbody tr` / `table tr`をN runtimeの限定parserが
`candidate_set_violation:css_selector_compound`で拒否し、2runではN evidenceを
生成できなかった。従って、到達runでN1〜N5 evidenceを要求するP1-aは
**4/6でFAIL**とする。これは新machine class候補
`ingest_candidate_selector:valid_compound_css_rejected`であり、terminalだけを
見た自動分類のmodel帰属を人手監査でmachineへ訂正する。

残る4runは候補10件を実際に凍結し、N1〜N5 evidenceを生成した。N1/N4/N5は
4/4 pass、N3は3/4 pass、N2は0/4 passだった。N2は25件の
`japanese_date_to_iso`変換を値保存・宣言・記録つきで成立させ、うち1件は
`令和8年8月2日`→`2026-08-02`だった。一方、短縮日付から候補外の年を補った
2run、candidate idのpath prefixを落とした1run、日付空欄を採用した1runを
正しくfailedにした。

不備2件をともに理由付き除外したrunは0/6だった。5runは日付欠落だけを除外し、
ソースに実在する文字列`会場未定`を採用した。1runは日付空欄を含めて全10件を
採用しN2に拒否された。この観測は記録値であり、N3の勘定整合を意味的な
除外品質へ拡張しない。

## 0. 開始条件

作業開始時点の`develop`先端
`17d23f5a05c6622b66d4efbefb94d51c00359c34`に対する最終確定値:

| workflow | run id | status | conclusion |
|---|---:|---|---|
| CI | `30330033272` | completed | success |
| acceptance | `30330033256` | completed | success |

## 1. INGEST-6材料注入

### 1.1 決定的・有界な規則

新しいleaf moduleが`data/snapshots/`を安定sortで走査し、ingest profileの
`implement-ingest-delivery`だけに次を注入する。

- file上限8、directory entry上限256、深さ上限4
- 1 file 64 KiB、先頭12行、1行200文字
- 反復候補要素はfileあたり2 window
- windowは前1行・後5行、filenameと`L####`行番号を併記
- symlinkと非regular fileは対象外
- truncation、omitted file、探索上限到達をeventへ記録

注入原文の主要部:

```text
Machine-injected snapshot structure material.
Snapshot file: data/snapshots/events-list.html
L0010 |     <article class="event" id="list-01">
HTML tag=article occurrences=10
セレクタは上記の実在構造から導出すること。
例示セレクタを写さないこと（構造が一致する場合を除く）。
```

入力抜粋は命令ではなくdataであることも明記した。既存2500文字のStepPlan lintを
緩めず、plan lint後のproduction step materialとして付加する。ingest以外と
implement以外はbyte unchangedである。

### 1.2 elev-005実測fixtureと起動実在

実測fixtureはelev-005の次の一次資料を固定した。

- snapshot本文のtool read 0件
- list実構造`article.event`に対する宣言`tr.event-row`
- table実構造`tbody > tr`に対する宣言`.event-item` /
  `div.event-item` / `div.event-card`
- 到達4/4のdetected=0

production `run_step_plan`を通すintegration testでproviderへ渡るprompt中の
filename、`L0010`の実`article.event`、候補出現数、導出禁止文言と、
`ingest_snapshot_structure_injected` eventの発火を確認した。単なるrender
helperの直接testではなくproduction境界を固定している。

### 1.3 class・台帳・scaffold

`ingest_selector:selector_without_snapshot_content_read`を
first_seen=`uat-test0726-ingest-elev-005`で登録した。近因model、設計根因machine
（読むべき実構造材料の未提示）の二層をnoteへ記録した。

台帳にはDATA-1系第5適用として
`data字義例→INV-1→cli→ingest正準形→入力実構造`を追記した。scaffoldの
admission checklistへ次を第5の定形装備として追加し、生成器・既存template・
ingest templateを同期した。

```text
inputs that generation must read are placed in bounded
machine-injected guidance with a measured fixture
```

### 1.4 権限付きfull verification

| check | 結果 |
|---|---|
| `cargo fmt --all -- --check` | green |
| `cargo clippy --all-targets -- -D warnings` | green |
| `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --all-targets` | **1847 passed / 0 failed / 30 ignored** |
| ingest plan synthesis | 6/6 |
| snapshot structure / step material | 3/3 |
| production injection integration | 1/1 |
| ingest conformance | 2/2 |
| generality guardrails | 9/9 |
| scaffold + classify unittest | 7/7 |
| Ruff | green |

lib/integration内訳は1689 passed / 15 ignoredと158 passed / 15 ignored。
growth tripwire baselineは変更せず、`runner.rs`はproduction配線の6行だけを
追加した。

## 2. Suite・preflight

### 2.1 実効構成

- suite: `ingest-create-elevated`
- profile / intent: `ingest / create`
- workspace mode: `sourced`
- planner config: `qwen3.6:27b-coding-nvfp4 / ollama`
- executor: `gemma4:31b-cloud / ollama`
- admission: `off`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0726_ingest_elev6`
- campaign: `ingest-create-elevated-20260728-061159`
- retry / interruption: なし

| family | asset | sha256 |
|---|---|---|
| list | `events-list.html` | `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07` |
| table | `events-table.html` | `394b03ccd7ac141a2677c55d9a2059034ea2c7a92656b466201ee40b7050cddd` |

input sha256一致とzero-exit precheckは6/6。`run_start` 6/6とexecutor
provider turn 89/89で実効model/providerが一致した。planner configも6/6一致し、
profile presetのためplanner provider turnは0件だった。

### 2.2 preflight

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD | `6628442 Inject bounded ingest snapshot structure` |
| minimum ancestor | `78b95ce` verified |
| bench内`cargo test` | exit 0 |
| release build | exit 0 |
| installed binary | `commandagent 0.1.0 6628442 2026-07-28T06:14:43Z` |
| built / installed sha256 | `34a624e23a0067b2a3ee182cc6e3dc367672f954fbff9c15024186ea20a0f1bd` / 同一 |
| `NODE_ENV` | `production` |

## 3. production材料注入の実測

6run全てで次のeventが1件ずつ発火した。list 3runは同一の2485 bytes、
table 3runは同一の2343 bytesを読み、いずれもtruncationなしだった。

```json
{"event":"ingest_snapshot_structure_injected",
 "step_id":"implement-ingest-delivery",
 "files":[{"candidate_windows":2,"head_lines":12,"read_bytes":2485,
           "relative_path":"data/snapshots/events-list.html",
           "source_bytes":2485,"truncated":false}],
 "omitted_files":0,"traversal_capped":false,
 "limits":{"context_after":5,"context_before":1,"head_lines":12,
           "max_candidate_windows":2,"max_depth":4,
           "max_directory_entries":256,"max_file_bytes":65536,
           "max_files":8,"max_line_chars":200}}
```

モデルが最終的に宣言したselector:

| run | selector | 実構造との対応 | model出力 |
|---|---|---|---|
| list 001 | `article.event` | 一致 | accepted 9 / excluded 1 |
| list 002 | `article.event` | 一致 | accepted 9 / excluded 1 |
| list 003 | `article.event` | 一致 | accepted 9 / excluded 1 |
| table 001 | `table tbody tr` | valid CSS・一致 | accepted 9 / excluded 1 |
| table 002 | `tr` | 一致 | accepted 10 / excluded 0 |
| table 003 | `table tr` | valid CSS・一致 | accepted 9 / excluded 1 |

elev-005の実構造不一致selectorは0/6へ減った。モデル成果物はいずれも
10候補相当を扱った。N runtimeで機械列挙できた4runは全てdetected=10。
残り2runは候補0ではなく、validな複合CSSのruntime拒否で未列挙である。

## 4. Run行列

`—`はfinal acceptanceに入ったが、selector parserのpre-evidence errorにより
N evidenceが生成されなかったことを表す。

| run | family | verdict | 表示assurance | N1 | N2 | N3 | N4 | N5 | 停止形 / 監査帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---|---:|
| `list_cloud_001` | list | failed | failed (`ingest_assurance_failed`) | pass | **failed** | pass | pass | pass | N2拒否後read-only repair / model | 859 |
| `list_cloud_002` | list | failed | failed (`ingest_assurance_failed`) | pass | **failed** | pass | pass | pass | N2拒否後read-only repair / model | 1381 |
| `list_cloud_003` | list | failed | failed (`ingest_assurance_failed`) | pass | **failed** | **failed** | pass | pass | candidate id不整合 / model | 66 |
| `table_cloud_001` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | valid compound CSS拒否 / **machine** | 48 |
| `table_cloud_002` | table | failed | failed (`ingest_assurance_failed`) | pass | **failed** | pass | pass | pass | 空日付採用後read-only repair / model | 51 |
| `table_cloud_003` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | valid compound CSS拒否 / **machine** | 34 |

全runのharness statusは`completed`、product exitは全て1。panic、理由なし終端、
環境中断、retry、偽成功は0件。

## 5. N1〜N5実物監査

### 5.1 起動実在

| evidence / event | 実在 |
|---|---:|
| final acceptance / profile probe起動 | 6/6 |
| snapshot structure injection | 6/6 |
| selector / candidate freeze | 4/6 |
| ingest probe (N1) | 4/6 |
| source binding (N2) | 4/6 |
| candidate accounting (N3) | 4/6 |
| format schema (N4) | 4/6 |
| rerun consistency (N5) | 4/6 |
| ingest assurance projection | 4/6 |

複合CSS拒否2runはN runtimeへdispatchされたが、evidence生成前に停止した。
従ってP1-aは4/6でFAILである。

### 5.2 セレクタ宣言・実行前凍結・候補分布

`list_cloud_001`原文:

```json
{"capability_id":"ingest_candidate_freeze",
 "selector":{"kind":"css","value":"article.event"},
 "record_format":{"fields":[
   {"name":"name","normalizations":["identity"],"type":"string"},
   {"name":"date","normalizations":["japanese_date_to_iso"],"type":"string"},
   {"name":"location","normalizations":["identity"],"type":"string"},
   {"name":"source_file","normalizations":["identity"],"type":"string"}]},
 "snapshots":[{"path":"data/snapshots/events-list.html",
               "bytes":2485,"fnv1a64":"bd5dd2ba4b62a316"}],
 "candidates":[
   {"id":"data/snapshots/events-list.html#0","ordinal":0,
    "byte_start":209,"byte_end":429,"fnv1a64":"edac65ae46d68d7d"},
   "... total 10 ...",
   {"id":"data/snapshots/events-list.html#9","ordinal":9,
    "byte_start":2223,"byte_end":2458,"fnv1a64":"d3b350c5097557fb"}]}
```

凍結後のcandidate分布はlist 10 / 10 / 10、table 10の計4runすべて10。
table 001/003は未列挙であり、0候補とは数えない。

### 5.3 N1実実行・N4・N5

`list_cloud_001`原文:

```json
{"capability_id":"ingest_probe","status":"pass","ok":true,
 "execution":{"capability_id":"pipeline_probe","status":"pass","ok":true,
 "outcome":"exited","command":["python3","-B","pipeline/main.py"],
 "duration_ms":119,"exit_code":0,
 "stdout":{"text":"","captured_bytes":0,"total_bytes":0,"truncated":false},
 "stderr":{"text":"","captured_bytes":0,"total_bytes":0,"truncated":false},
 "isolation":{"level":"workspace_cwd_env_allowlist_bounded_offline_policy",
              "workspace_cwd":true,"environment_allowlist":true,
              "process_group":true,"bounded_timeout_ms":30000,
              "offline_policy_applied":true,
              "network_namespace_enforced":false}}}
```

4runのN1はexit 0、所要119 / 130 / 121 / 117ms。N4はrecords 9 / 9 / 9 /
10の全フィールドを4/4 pass、N5はrecords/report再実行一致を4/4 passした。

### 5.4 N3候補勘定とsilent drop監視

`list_cloud_001`原文:

```json
{"capability_id":"ingest_candidate_accounting","status":"pass","ok":true,
 "selector":{"kind":"css","value":"article.event"},
 "detected":10,"accepted":9,
 "excluded_by_reason":{"missing required fields":1},
 "equation":"10 = 9 + 1",
 "candidate_ids":["data/snapshots/events-list.html#0",
                  "...",
                  "data/snapshots/events-list.html#9"],
 "failure_kinds":[]}
```

list 001/002は`10 = 9 + 1`、table 002は`10 = 10 + 0`でpassした。
list 003は式上の数は`10 = 9 + 1`だが、inspectionが
`events-list.html#N`とpath prefixを落としたため、機械凍結ID
`data/snapshots/events-list.html#N`と一致しなかった。N3は全10 candidateを
unaccounted、全10申告IDをunknownとして20 violationを生成し、silent drop /
candidate-set置換を拒否した。

全6成果物での不備candidateの扱い:

- list 3run: 日付欠落を理由付き除外1、`会場未定`を採用
- table 001/003: 日付欠落を理由付き除外1、`会場未定`を採用
- table 002: 日付空欄と`会場未定`をともに採用

従って「不備2件を理由付き除外」は0/6。N3は申告されたcandidate setの
勘定を検証するもので、`会場未定`という実在値を意味的に不正と推定する
契約ではない。この値をN3 failureへ発明で変換しない。

### 5.5 N2三条件正規化とviolation全件

成功した和暦変換の実測原文:

```json
{"record_index":0,
 "candidate_id":"data/snapshots/events-table.html#0",
 "field":"date",
 "output_value":"2026-08-02",
 "declared_normalizations":["japanese_date_to_iso"],
 "raw_source":"令和8年8月2日",
 "normalized_source":"2026-08-02",
 "transformations":["japanese_date_to_iso"],
 "matched":true,
 "nearest_miss":null}
```

値保存・宣言・field別記録の三条件がliveで成立した。
`japanese_date_to_iso`のmatched traceはlist 001=8、list 002=8、
list 003=0、table 002=9、合計25件。Reiwaは1件である。

violation集計:

| run | bindings | matched | violations | 原因 |
|---|---:|---:|---:|---|
| list 001 | 36 | 35 | 1 | 候補内`8/3(月)`から年2026を補完 |
| list 002 | 36 | 35 | 1 | 同上 |
| list 003 | 36 | 0 | 36 | candidate id path prefix欠落 |
| table 002 | 40 | 39 | 1 | 日付空欄をrecordとして採用 |

list 001/002の全violation原文:

```text
source_binding_violation:record=1:field=date:value=2026-08-03
```

候補ブロック内のsourceは`8/3(月)`であり、年はpage titleにしかない。
契約N2は同一候補ブロック内束縛を要求するため、cross-blockの年補完を
形式変換として通さない判定は正当である。

table 002の全violation原文:

```text
source_binding_violation:record=8:field=date:value=
```

list 003の36件全件:

```text
source_binding_violation:record=0:field=date:value=2026-08-01
source_binding_violation:record=0:field=location:value=市民広場
source_binding_violation:record=0:field=name:value=市民夏まつり
source_binding_violation:record=0:field=source_file:value=events-list.html
source_binding_violation:record=1:field=date:value=2026-08-03
source_binding_violation:record=1:field=location:value=中央図書館
source_binding_violation:record=1:field=name:value=親子読み聞かせ会
source_binding_violation:record=1:field=source_file:value=events-list.html
source_binding_violation:record=2:field=date:value=2026-08-05
source_binding_violation:record=2:field=location:value=防災センター
source_binding_violation:record=2:field=name:value=地域防災講座
source_binding_violation:record=2:field=source_file:value=events-list.html
source_binding_violation:record=3:field=date:value=2026-08-07
source_binding_violation:record=3:field=location:value=青少年会館
source_binding_violation:record=3:field=name:value=こども科学教室
source_binding_violation:record=3:field=source_file:value=events-list.html
source_binding_violation:record=4:field=date:value=2026-08-09
source_binding_violation:record=4:field=location:value=駅前広場
source_binding_violation:record=4:field=name:value=駅前朝市
source_binding_violation:record=4:field=source_file:value=events-list.html
source_binding_violation:record=5:field=date:value=2026-08-12
source_binding_violation:record=5:field=location:value=文化ホール
source_binding_violation:record=5:field=name:value=平和映画会
source_binding_violation:record=5:field=source_file:value=events-list.html
source_binding_violation:record=6:field=date:value=2026-08-15
source_binding_violation:record=6:field=location:value=保健センター
source_binding_violation:record=6:field=name:value=夏の健康相談
source_binding_violation:record=6:field=source_file:value=events-list.html
source_binding_violation:record=7:field=date:value=2026-08-20
source_binding_violation:record=7:field=location:value=河川公園
source_binding_violation:record=7:field=name:value=星空観察会
source_binding_violation:record=7:field=source_file:value=events-list.html
source_binding_violation:record=8:field=date:value=2026-08-28
source_binding_violation:record=8:field=location:value=会場未定
source_binding_violation:record=8:field=name:value=市民音楽交流会
source_binding_violation:record=8:field=source_file:value=events-list.html
```

nearest_missは全148 bindingでnullだった。

### 5.6 assuranceとadmission cap

- list 001/002/003とtable 002: N2またはN3 violationにより
  `failed (ingest_assurance_failed)`。
- table 001/003: N1未実行により`static (ingest_probe_not_run)`。

failedをadmission=offのstatic capで隠していない。full/partial相当runが0件の
ため`profile_not_admitted` capの実発火は今回は未観測だが、§assuranceの
failed優先と未実行staticは6/6で整合した。

## 6. 死因の機械 / モデル帰属

### 6.1 自動分類

report skeletonの分類:

- registry known: 6/6
- UNKNOWN: 0/6
- `model_stagnation_read_only` / model: 5
- `process_failure` / model: 1

これはterminal patternの非裁定分類である。

### 6.2 一次資料による人手監査

- list 001/002: machine N2は同一候補block外の年補完を正しく拒否した。
  その後のread-only repairを含め近因model。
- list 003: modelがcandidate idのpath prefixを落とした。N2/N3が正しく
  拒否したためmodel。
- table 002: modelが空日付candidateを採用した。N2が正しく拒否したためmodel。
- table 001/003: `table tbody tr` / `table tr`は実HTMLに一致するvalid CSS。
  guidanceの許容例自体も複合CSSを含むのに、N runtimeだけが
  `candidate_set_violation:css_selector_compound`でpre-evidence停止した。
  terminalのread-only loopは二次症状で、設計根因はmachine。

新class候補:

| class | attribution | first seen | 扱い |
|---|---|---|---|
| `ingest_candidate_selector:valid_compound_css_rejected` | machine | `uat-test0726-ingest-elev-006` | review入力。INGEST-6では修正しない |
| `ingest_candidate_accounting:candidate_id_path_prefix_omitted` | model | 同上 | 実測形。既存N3が拒否 |
| `ingest_source_binding:required_empty_value_accepted` | model | 同上 | 実測形。既存N2が拒否 |
| `ingest_source_binding:cross_block_year_inference` | model | 同上 | 契約どおり既存N2が拒否 |

commit 1で登録した
`ingest_selector:selector_without_snapshot_content_read`は0/6再発であり、
材料注入により解消を実測した。

## 7. E-0・scrub・コスト

### 7.1 E-0

| 項目 | 結果 |
|---|---|
| 自動分類 | known 6 / UNKNOWN 0 |
| acceptance sheet自動生成 | **6/6** |
| N2 nearest_miss | 0 |
| N2 violation | 39件 / 4run |
| N3 violation | 20件 / 1run |
| calibration collector追記 | 0 |

collectorの既存CLI形はnearest_missを対象とするが、今回nearest_missが
全件nullのため追記はない。raw violationは人手レポートとscrub済み集計へ
保存した。

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
| preflight start | `1785219119` |
| preflight completed / run start | `1785219300` |
| last run end | `1785221739` |
| audit + scrub end | `1785222180` |
| preflight | 181秒 |
| formal run合計 | 2439秒 |
| list族 | 2306秒 |
| table族 | 133秒 |
| preflight + formal run | 2620秒 |
| preflight start → audit/scrub end | 3061秒 |

## 8. 事前合否

| criterion | 判定 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | completed / failed 6/6、全件理由あり |
| P0-b §assurance | **PASS** | N violation failed 4、N1未実行static 2。failed隠蔽なし |
| P0-c 偽成功ゼロ | **PASS** | full/partial 0、全product exit 1 |
| P1-a 到達runでN1〜N5 | **FAIL** | final acceptance 6/6に対しevidence 4/6。複合CSSのmachine gap |
| P1-b sheet 6/6 | **PASS** | 自動生成6/6 |

記録値:

- snapshot structure injection: 6/6
- 実構造一致selector: 6/6
- runtime candidate freeze: 4/6、分布10 / 10 / 10 / 10
- full相当率: 0/6 (0%)
- failed: 6/6
- N1: 4/4 pass
- N2: 0/4 pass、109/148 field binding match、39 violations
- N2 normalization trace: 25 matched、Reiwa→ISO 1
- N3: 3/4 pass、1/4 failed（20 violations）
- N4 / N5: 4/4 pass
- 不備2件の理由付き除外: 0/6
- family差: list N evidence 3/3・table 1/3
- 新machine class候補: 1

## 9. 一次資料

外部campaign:

```text
/Users/maenokota/share/work/localwork/commandagent_mvp/01/
test0726_ingest_elev6/ingest-create-elevated-20260728-061159
```

| 資料 | sha256 |
|---|---|
| `uat-meta.json` | `ce7c89a459135668ef745f891b10f432934ad4ecab641dacb385cca85cb9754e` |
| `report-skeleton.md` | `dbadd5f27cf154449e712ba54cf05e8f10f1c3810dc77b3c6bafe10605c5c04f` |
| list acceptance sheet 001/002/003 | `2d972d41387523cbc6d2c4f2bda2b2793565bd40b11d9e62f50e75fabfbe55e6` / `8ab0349691288dd2357fa55eb891e7fecb2a87ff45d73602f4991d141df9e4c1` / `3e1da696c78e5f7242a56ab74f16b14dfe74f10b29d320d5302247b80857aaa6` |
| table acceptance sheet 001/002/003 | `4f514e60087fc2e3d67e71dce7b33adb08b23a44166e933fee56805b6d58e68f` / `41c7c422ebdf054e162fa921cd2b9b65dda864364ddeb90b0cfd19f915974c9a` / `ff4faffb466383bf839bd6d6818fe40f83474749820fe5e903d122b548c306a0` |

機械可読な集計は`evidence/campaign-summary.json`へ保存する。
