# BoN declaration methodology v2

## 事前宣言

- 対象: `bon_series`を持つ今後の全系列。過去のv0/v1 evidenceは改変しない。
- 必須入力: `p-hat`のfull分子・全trial分母・source、Wilson 95% CI、
  Jeffreys priorを用いるBeta-binomialのsuite単位95%予測帯。
- fail-closed条件: 点`p`二項フィールド、分母なし、CI不一致、posterior不一致、
  suite本数不一致、予測確率・期待本数・予測帯の再計算不一致。
- snapshot規則: doctorとOpenAI `/v1/models`の双方を確認し、Lunaの日付版
  snapshotが列挙されれば以後のsuiteをそのIDへ変更する。なければaliasを維持し、
  時点と観測IDを証跡化する。

参考として、清算済みLuna pool `4/42`を6本へ外挿する場合、Jeffreys posteriorは
`Beta(4.5, 38.5)`、Beta-binomialで`P(full >= 1)=46.52%`、期待full本数
`0.6279`、95%最短連続予測帯は`0..2`となる。これは既知の点`p=4/42`を
代入した二項値`45.15%`とは別物であり、基準率推定の不確実性を伝播する。

## 実測

2026-08-04T08:01:55+09:00にrelease binaryのdoctorをsuiteと同じ
executor/planner指定で実行した。doctorはexit 0、全体`warn`だったが、executorと
plannerの解決、Ollama到達性、planner model存在、OpenAI key存在（値はredact）、
OpenAI models endpoint到達性はすべて`pass`だった。

同じ子process環境からOpenAI `/v1/models`を照会したところ、`luna`を含むIDは
`gpt-5.6-luna`のみで、日付版snapshot IDは0件だった。このため
`workspace/management/bench/suites/cli-filter-bon0.toml`のexecutor aliasは変更しない。
機械証跡は`evidence/luna-snapshot-check.json`に置く。

## 検算

- loaderはschema v2だけを受理し、全派生値を入力値から再計算する。
- preflightは検証済みbaseline/predictive objectを系列pinへ複写し、binary/revision/
  suite pinと同じく支出前に固定する。
- selectorはschema v2と両objectの存在を要求する。
- 正例、点二項の負例、CI欠落の負例、予測帯不一致の負例、coverage不一致の負例を
  focused testへ追加した。
- snapshot分岐は「日付版0件」なので、suite hashを変えないことが正しい帰結である。
