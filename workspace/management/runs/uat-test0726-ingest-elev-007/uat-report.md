# uat-test0726-ingest-elev-007: ingest×create elevated再計測

実施日: 2026-07-28 (JST)

裁定契約: `docs/ingest-profile-contract.md`
(`fixed v0.1`, 2026-07-28)

計測revision: `f9d5e597f74774d39786bb121222e1c42e8cf957`
(`develop`)

## 結論

**P0-a / P0-b / P0-c / P1-a / P1-bはPASS、full相当は0/6だった。**

INGEST-7で拡張したCSS engineはproductionで複合selector
`table tbody tr`をtable 3runすべてで実行し、各10候補を凍結した。
elev-006の`candidate_set_violation:css_selector_compound`は0/6へ減り、
全runでN1〜N5 evidenceが生成された。N1/N4/N5は6/6 passである。

一方、6runすべてがinspectionのcandidate idを
`events-list.html#N` / `events-table.html#N`と記録し、機械凍結された
`data/snapshots/events-list.html#N` /
`data/snapshots/events-table.html#N`のpath prefixを落とした。そのためN2は
216/216 field bindingを拒否し、N3も6/6でunknown candidateとunaccounted
candidateを検出した。近因・裁定帰属はmodelであり、elev-006で登録済みの
`ingest_candidate_accounting:candidate_id_path_prefix_omitted`の再発である。
新しいmachine classは収穫されなかった。

契約v0.1の`document_year_context`は全runで9 date fieldずつ、計54件に
宣言された。しかしcandidate id不一致のため候補blockを同定できず、
候補内断片と文書文脈断片の両方を位置付きで記録する条件(c)に到達したrunは
0/6だった。従って、年補完のlive成立を主文にはしない。

日付欠落候補を理由付きで除外したモデル出力は5/6だった。残るlist 003は
accepted 2 / excluded 0でsilent dropし、N3が14 violationsで拒否した。
`会場未定`はsourceに実在する忠実な値なので、採用を失敗とは数えない。

## 0. 開始条件

作業開始時点の`develop`先端
`19c86f46ad60c06d12dcd7073f8f233c89634ac9`に対する最終確定値:

| workflow | run id | status | conclusion |
|---|---:|---|---|
| CI | `30337294007` | completed | success |
| acceptance | `30337294004` | completed | success |

## 1. INGEST-7変更とfull verification

### 1.1 CSS engine被覆

elev-006の実測原文をfixtureへ固定した。

| run | 実測selector | elev-006原文 | elev-007期待 |
|---|---|---|---:|
| `table_cloud_001` | `table tbody tr` | `candidate_set_violation:css_selector_compound` | 10 candidates |
| `table_cloud_003` | `table tr` | `candidate_set_violation:css_selector_compound` | 10 candidates |

engineはtag / `#id` / `.class`のcompound、空白descendant、`>` childを
最大8 compoundまで扱う。attribute、pseudo、comma、sibling combinatorは
宣言時のstructure gateで拒否し、許容形をエラーとguidanceに列挙する。
実行後に突然拒否する形にはしなかった。

### 1.2 契約v0.1

N2にdocument-level shared contextを追加した。部分値補完は、値保存、
決定的な規則宣言、候補断片と文書文脈断片の双方のsource path・byte位置記録、
の三条件が必須である。候補間の継ぎ合わせ、文書に存在しない値、日付ずらしは
従来どおりviolationである。

実測fixtureはelev-006 listの`8/3(月)`と文書見出しの`2026年`を使い、
両断片位置つきの`2026-08-03`だけをpassにした。`8/4`へのずらしと別候補の
値の流用は拒否する非緩和fixtureを固定した。

### 1.3 elev-006 36件裁定と資産設計

既存run記録は改変せず、
`uat-test0726-ingest-elev-006/ingest-7-adjudication.md`に裁定を追加した。
list 003の36件は9 record×4 fieldで、すべてcandidate idのpath prefix欠落から
派生した。35件は正しい候補lineageなら字義または従来正規化で束縛し、
短縮日付1件はv0.1の宣言済み文書文脈を必要とする。照合器の厳格性は維持した。

scaffoldのmeasurement asset designへ「意図的な不備候補は意味的に曖昧な
値ではなく、機械的に抽出不能な形で設計する」を追加した。

### 1.4 verification

| check | 結果 |
|---|---|
| `cargo fmt --all -- --check` | green |
| `cargo clippy --all-targets -- -D warnings` | green |
| `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --all-targets` | **1854 passed / 0 failed / 30 ignored** |
| Python 3.12 scripts unittest | **54 passed / 0 failed** |
| Ruff | green |

Rust内訳はlib 1696 passed / 15 ignored、integration 158 passed /
15 ignored。growth tripwire baselineとadmission=`off`は変更していない。

## 2. Suite・preflight

- suite: `ingest-create-elevated`
- suite sha256:
  `f7e9c448defd833c353b0e4a8f28b8a9adba19595c438b648cc50b6102325146`
- profile / intent: `ingest / create`
- workspace mode: `sourced`
- planner config: `qwen3.6:27b-coding-nvfp4 / ollama`
- executor: `gemma4:31b-cloud / ollama`
- admission: `off`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0726_ingest_elev7`
- campaign: `ingest-create-elevated-20260728-090929`
- retry / interruption: なし

| family | asset | sha256 |
|---|---|---|
| list | `events-list.html` | `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07` |
| table | `events-table.html` | `394b03ccd7ac141a2677c55d9a2059034ea2c7a92656b466201ee40b7050cddd` |

input hash一致とzero-exit precheckは6/6。`run_start` 6/6とexecutor
provider turn 42/42で`gemma4:31b-cloud / ollama`を確認した。planner configも
6/6一致し、ingest presetのためplanner provider turnは0件だった。

| preflight項目 | 結果 |
|---|---|
| git status | clean |
| HEAD | `f9d5e59 Record ingest violation adjudication` |
| minimum ancestor | `78b95ce` verified |
| bench内`cargo test` | exit 0 |
| release build | exit 0 |
| installed binary | `commandagent 0.1.0 f9d5e59 2026-07-28T09:12:16Z` |
| built / installed sha256 | `c583d5d5dcd0ba151e04503c30f56f69a971c8a77d3d4c2a85b51dcd1a56738d` / 同一 |
| `NODE_ENV` | `production` |

## 3. Run行列

| run | family | verdict | assurance | N1 | N2 | N3 | N4 | N5 | 自動停止class / 監査帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---|---:|
| `list_cloud_001` | list | failed | failed (`ingest_assurance_failed`) | pass | **failed** | **failed** | pass | pass | `model_stagnation_read_only` / model | 41 |
| `list_cloud_002` | list | failed | failed (`ingest_assurance_failed`) | pass | **failed** | **failed** | pass | pass | `process_failure` / model | 30 |
| `list_cloud_003` | list | failed | failed (`ingest_assurance_failed`) | pass | **failed** | **failed** | pass | pass | `model_stagnation_read_only` / model | 29 |
| `table_cloud_001` | table | failed | failed (`ingest_assurance_failed`) | pass | **failed** | **failed** | pass | pass | `model_stagnation_read_only` / model | 24 |
| `table_cloud_002` | table | failed | failed (`ingest_assurance_failed`) | pass | **failed** | **failed** | pass | pass | `model_stagnation_read_only` / model | 54 |
| `table_cloud_003` | table | failed | failed (`ingest_assurance_failed`) | pass | **failed** | **failed** | pass | pass | `model_stagnation_read_only` / model | 21 |

全runのharness statusは`completed`、product exitは1。panic、理由なし終端、
環境中断、retry、偽成功は0件。自動分類はknown 6 / UNKNOWN 0。
terminal classは二次的な停止形であり、N2/N3の一次原因は6/6共通の
candidate id path prefix欠落である。

## 4. N1〜N5実物監査

### 4.1 起動実在と複合CSS

| evidence / event | 実在 |
|---|---:|
| final acceptance / profile probe | 6/6 |
| selector / candidate freeze | 6/6 |
| ingest probe (N1) | 6/6 |
| source binding (N2) | 6/6 |
| candidate accounting (N3) | 6/6 |
| format schema (N4) | 6/6 |
| rerun consistency (N5) | 6/6 |
| ingest assurance projection | 6/6 |
| `candidate_set_violation:css_selector_compound` | **0/6** |

table全runのselectorは`table tbody tr`で、いずれも10候補を凍結した。
`table_cloud_001`原文:

```json
{"capability_id":"ingest_candidate_freeze",
 "selector":{"kind":"css","value":"table tbody tr"},
 "snapshots":[{"path":"data/snapshots/events-table.html",
               "bytes":2343,"fnv1a64":"fd75ab5a1aa3152a"}],
 "candidates":[
   {"id":"data/snapshots/events-table.html#0","ordinal":0,
    "byte_start":394,"byte_end":571,"fnv1a64":"f51f7857ae1df031"},
   "... total 10 ...",
   {"id":"data/snapshots/events-table.html#9","ordinal":9,
    "byte_start":2091,"byte_end":2288,"fnv1a64":"59aa15f9e1ce0612"}]}
```

N1は全runで`python3 -B pipeline/main.py`をoffline・30秒上限で実実行し、
exit 0だった。実行時間はlist 141 / 136 / 119 ms、table 137 / 142 /
136 ms。

### 4.2 N3勘定とsilent drop

| run | selector | detected | model accepted | model excluded | model equation | N3 violations |
|---|---|---:|---:|---:|---|---:|
| list 001 | `article.event` | 10 | 9 | 1 | `10 = 9 + 1` | 20 |
| list 002 | `article.event` | 10 | 9 | 1 | `10 = 9 + 1` | 20 |
| list 003 | `article.event` | 10 | 2 | 0 | `10 = 2 + 0` | 14 |
| table 001 | `table tbody tr` | 10 | 9 | 1 | `10 = 9 + 1` | 20 |
| table 002 | `table tbody tr` | 10 | 9 | 1 | `10 = 9 + 1` | 20 |
| table 003 | `table tbody tr` | 10 | 9 | 1 | `10 = 9 + 1` | 20 |

list 001原文:

```json
{"capability_id":"ingest_candidate_accounting",
 "status":"failed","ok":false,
 "selector":{"kind":"css","value":"article.event"},
 "detected":10,"accepted":9,
 "excluded_by_reason":{"missing required fields":1},
 "equation":"10 = 9 + 1",
 "failure_kinds":[
   "accounting_violation:unaccounted_candidate:data/snapshots/events-list.html#0",
   "... frozen candidates #1 through #9 ...",
   "candidate_set_violation:unknown_candidate:events-list.html#0",
   "... model candidates #1 through #9 ..."]}
```

不備の日付欠落を理由付きでモデルが除外したのは5/6。list 003の8件silent
dropは`10 = 2 + 0`不整合、10件unaccounted、2件unknownとして拒否された。

### 4.3 N2 document contextとviolation原文

全runのdate 9件、計54件に次の宣言が実在した。

```json
{"record_index":1,
 "candidate_id":"events-list.html#1",
 "field":"date","output_value":"2026-08-03",
 "declared_normalizations":["japanese_date_to_iso","document_year_context"],
 "source_path":null,
 "candidate_byte_start":null,"candidate_byte_end":null,
 "raw_source":null,"normalized_source":null,"transformations":[],
 "candidate_fragment":null,"document_context":null,
 "matched":false,"nearest_miss":null}
```

candidate idが凍結集合に存在しないためsource pathと位置を解決できず、全216
fieldがfailedとなった。従って年補完の条件(c)である両断片記録は0/54である。
list 3runの`failure_kinds`はbyte-identical
(`sha256=2dd92b5ebbafe04845814a262bc0062dc1eab87dda61de8c6fbfebbd2f838d14`)、
table 3runもbyte-identical
(`sha256=47d460b8c75332aace510d1198ca1825d6a6e0759f210219aa662a0076906d4b`)。
以下の各36行が各family 3runへそのまま適用され、216件の原文全体を表す。

list family原文:

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

table family原文:

```text
source_binding_violation:record=0:field=date:value=2026-08-02
source_binding_violation:record=0:field=location:value=郷土資料館
source_binding_violation:record=0:field=name:value=郷土資料展
source_binding_violation:record=0:field=source_file:value=events-table.html
source_binding_violation:record=1:field=date:value=2026-08-04
source_binding_violation:record=1:field=location:value=町民調理室
source_binding_violation:record=1:field=name:value=親子料理教室
source_binding_violation:record=1:field=source_file:value=events-table.html
source_binding_violation:record=2:field=date:value=2026-08-06
source_binding_violation:record=2:field=location:value=あおば池公園
source_binding_violation:record=2:field=name:value=水辺の自然観察
source_binding_violation:record=2:field=source_file:value=events-table.html
source_binding_violation:record=3:field=date:value=2026-08-08
source_binding_violation:record=3:field=location:value=福祉交流館
source_binding_violation:record=3:field=name:value=手話入門講座
source_binding_violation:record=3:field=source_file:value=events-table.html
source_binding_violation:record=4:field=date:value=2026-08-11
source_binding_violation:record=4:field=location:value=町民センター
source_binding_violation:record=4:field=name:value=こども将棋大会
source_binding_violation:record=4:field=source_file:value=events-table.html
source_binding_violation:record=5:field=date:value=2026-08-14
source_binding_violation:record=5:field=location:value=第二公民館
source_binding_violation:record=5:field=name:value=盆踊り練習会
source_binding_violation:record=5:field=source_file:value=events-table.html
source_binding_violation:record=6:field=date:value=2026-08-18
source_binding_violation:record=6:field=location:value=あおば図書室
source_binding_violation:record=6:field=name:value=夏休み読書会
source_binding_violation:record=6:field=source_file:value=events-table.html
source_binding_violation:record=7:field=date:value=2026-08-22
source_binding_violation:record=7:field=location:value=文化会館
source_binding_violation:record=7:field=name:value=夕涼みコンサート
source_binding_violation:record=7:field=source_file:value=events-table.html
source_binding_violation:record=8:field=date:value=2026-08-30
source_binding_violation:record=8:field=location:value=会場未定
source_binding_violation:record=8:field=name:value=スポーツ体験会
source_binding_violation:record=8:field=source_file:value=events-table.html
```

### 4.4 N4 / N5

N4は全runでJSON parseと宣言schema
`name,date,location,source_file`を満たし、record countは9だった。
N5は全runで`output/records.json`と`output/report.md`の再実行一致を確認した。

## 5. assurance・E-0・帰属

最終projectionは6/6で`failed (ingest_assurance_failed)`を表示した。
途中のgeneric acceptance eventはadmission offにより
`static (profile_not_admitted)`だが、N2/N3 violationが存在する場合はfailedを
優先する契約写像がterminalへ反映された。offの上限はfull/partialの獲得を
staticへcapするが、failedをstaticへ隠していない。偽成功は0件である。

| E-0項目 | 実測 |
|---|---|
| 自動分類 | known 6 / UNKNOWN 0 |
| acceptance sheet自動生成 | 6/6 (100%) |
| per-run scrub | 6/6 green |
| campaign全体scrub | green、finding 0 |
| collector | N2 nearest_miss 0、N3はcollector対象外 |

監査帰属は6/6 model。正しいcandidate idの字義例はguidanceに配布済みで、
機械凍結evidenceにも実在する。全runが同じprefixを落としたことは知識gapの
再監査材料だが、本計測では既登録model classの再発として扱う。

## 6. コスト

`date +%s`基準:

| 区間 | epoch / 秒 |
|---|---:|
| preflight start | `1785229769` |
| preflight complete / run start | `1785229954` |
| run end | `1785230153` |
| evidence audit・scrub end | `1785230332` |
| preflight | 185秒 |
| formal run合計 | 199秒 |
| list / table | 100秒 / 99秒 |
| preflight開始→audit・scrub完了 | 563秒 |

## 7. 合否

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | completed 6/6、理由ありfailed 6/6 |
| P0-b §assurance準拠 | **PASS** | N2/N3違反をfailed 6/6、off capで隠蔽なし |
| P0-c 偽成功ゼロ | **PASS** | verdict成功0、full相当0 |
| P1-a 到達runでN1〜N5 | **PASS** | evidence 6/6、複合CSS拒否0/6 |
| P1-b sheet 6/6 | **PASS** | 6/6生成、sha256固定 |

記録値:

- full相当: **0/6**
- N1 / N2 / N3 / N4 / N5:
  **6/6 / 0/6 / 0/6 / 6/6 / 6/6**
- candidate detected: 10 / 10 / 10 / 10 / 10 / 10
- 日付欠落の理由付き除外: 5/6
- v0.1 document context宣言: 54件
- v0.1両断片位置記録: 0件（candidate lineage違反で未到達）
- 新class: 0
- 再発class:
  `ingest_candidate_accounting:candidate_id_path_prefix_omitted` 6/6

資格情報・token・password・private key・home絶対pathについて、benchの
per-run scrub 6/6とcampaign全体scrubを実施し、finding 0だった。repoへ保存する
のは本レポートと集約済み`evidence/campaign-summary.json`だけであり、
raw log、workspace、`.anvil/` stateは保存しない。
