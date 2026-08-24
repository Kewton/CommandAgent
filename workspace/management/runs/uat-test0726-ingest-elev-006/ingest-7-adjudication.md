# INGEST-7 adjudication addendum for ingest-elev-006

この文書は`uat-test0726-ingest-elev-006`の一次資料に対する2026-07-28の
レビュー裁定である。既存`uat-report.md`とscrub済み集計はimmutableな
計測記録として変更せず、本追補を裁定層として追加する。

## 裁定③: list_cloud_003のN2 36件

36件の内訳は、accepted 9 record × 4 field
（date / location / name / source_file各9件）である。全bindingが次の同形を
持つ。

```text
inspection candidate_id: events-list.html#N
frozen candidate_id:     data/snapshots/events-list.html#N
source_path:              null (36/36)
candidate position:       null (36/36)
raw_source:               null (36/36)
nearest_miss:             null (36/36)
```

N3も同じ一次資料から、宣言IDを`unknown_candidate` 10件、凍結IDを
`unaccounted_candidate` 10件として独立に拒否した。したがって36件は
フィールド値の個別捏造を示す36クラスではなく、record→frozen candidateの
lineageを失った単一クラス
`ingest_candidate_accounting:candidate_id_path_prefix_omitted`の派生結果で
ある。

出力値を正しい凍結candidateへ対応させると35/36は同一候補の字義値または
既存の宣言済み日付正規化へ束縛できる。残るrecord 1 date
`2026-08-03`は候補内`8/3(月)`と文書titleの`2026年`を必要とし、v0.1では
`document_year_context`の宣言と両断片位置記録がある場合だけ成立する。
elev-006当時はこの宣言語彙が存在しなかったため、歴史的v0判定を改ざんせず、
v0.1の実測fixtureで較正する。

裁定は「近因modelのcandidate ID lineage宣言違反、N2/N3判定正当」である。
照合器のIDを曖昧一致させる緩和、path prefixの推測補完、36件を個別の
source forgeryとして数えることのいずれも行わない。

## 裁定④: `会場未定`と資産設計

`会場未定`はsource candidate内に字義で実在し、宣言schemaのlocation文字列
にも準拠する。これを採用したモデル出力は忠実抽出として正当であり、
N2/N3 violationへしてはならない。「不備2件」の一方として配置した計測資産
側が、意味的には未確定でも機械抽出可能な値を使ったことによる曖昧さである。

この資産でsilent drop拒否の実弾として成立したのは、日付断片そのものがない
candidateの理由付き除外5/6である。残り1/6は空日付を採用しN2が拒否した。
従って「不備2件を理由付き除外」は実測事実ではなく、`会場未定`の採用を
model failureへ数えない。

scaffoldのmeasurement asset designには、意図した不備candidateを
「意味的に曖昧」ではなく「必須値が実在せず機械抽出不能」な形で設計する
項目を追加する。既存suite資産は歴史的入力hashを保つため、この裁定では
書き換えない。
