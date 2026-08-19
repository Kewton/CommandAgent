# はじめての境界ループ

この手順は、CommandAgentを初めて触る人が、依頼の確認、無対話実行、
検収シートの判定までを自力で1周し、その後にpackのパラメータだけを
変えたローカルA/Bを1回行うためのものです。リポジトリrootから実行して
ください。Ollamaと記載モデルが利用できる健全なセッションを前提にします。

ここで行う自己実演は操作確認です。第三者P-3計測や正式bandへの登録には
数えません。

## 1. ingest依頼を4ゲートで回す

### 準備

固定入力を一時workspaceへコピーし、製品binaryをbuildします。既存の
workspaceや履歴を混ぜないよう、作業領域とstate領域を分けます。

```bash
export D3C_LOOP_ROOT=/tmp/commandagent-first-loop
export D3C_LOOP_STATE=/tmp/commandagent-first-loop-state
mkdir -p "$D3C_LOOP_ROOT/data/snapshots"
cp workspace/management/bench/assets/ingest/list/data/snapshots/events-list.html "$D3C_LOOP_ROOT/data/snapshots/events-list.html"
ollama ps
cargo build --release --bin commandagent
```

境界シェルを含むREPLを起動します。

```bash
cargo run --release --bin commandagent -- --cwd "$D3C_LOOP_ROOT" --state-dir "$D3C_LOOP_STATE" --planner-provider ollama --planner-model qwen3.6:27b-coding-nvfp4 --provider ollama --model gemma4:31b-cloud
```

REPLへ次の依頼を1行で入力します。

```text
data/snapshots/ 配下のHTMLイベント一覧から自治体イベント情報を抽出し、JSON形式(name, date, location, source_file)でoutput/records.jsonへ整形するパイプラインを作成してください。候補検出セレクタを宣言し、抽出不能候補は理由付きで除外し、output/report.mdに件数と要約を記載してください。
```

### Gate 1: 依頼を確定する

Gate 1は実行結果ではなく提案カードです。依頼内容、各必須チェックの説明、
類似実行の合格件数、ファイル変更範囲、モデル、検証パック、確認 ID を
読みます。shakedownでは次の形でした（workspaceと確認 ID は実行ごとに
変わります）。

```text
# Gate 1 — 実行前の確認

## 実行する内容
- 作業内容: 新しい機能を作成 (create): データ取り込みパイプライン (ingest) / 一覧 (list)
- 契約の参照先: docs/ingest-profile-contract.md

## 必須チェック
- N1 — パイプライン実行: 有界な取り込みコマンドが正常に完了する
- N2 — 入力元の正確さ: すべての出力値が選択した入力レコードに結び付く
- N3 — 候補の勘定: すべての検出候補が採用または明示的な除外になる
- N4 — 形式の正確さ: 出力フィールドと型が必須スキーマと一致する
- N5 — 再現性: 取り込みを再実行しても同じ結果になる

## 類似実行の結果
- 全必須チェックに合格した実行: 6件中4件 (66.7%)

## 確認
- 確認 ID (内容が1つでも変わると ID も変わります): sha256:<card-hash>

これは提案であり、実行結果ではありません。
実行前にこの ID と完全一致する内容を確認してください。CLI では /confirm sha256:<card-hash> を使用します。
```

内容が依頼と一致するときだけ、画面に出たhashをそのまま入力します。

```text
/confirm sha256:<card-hash>
```

確認を拒否するなら、`/confirm`を入力しません。依頼を修正して新しいGate 1を
出し、古いhashは捨てます。確認記録が永続化されるまでdispatchはできません。

### Gate 2: 無対話実行を待つ

確認後は次の境界表示が出て、同じカードに束縛された実行が始まります。
途中で追加の会話判断は要求されません。

```text
Persisted confirmation: `sha256:<card-hash>`
Dispatching ingest × create × list.
```

### Gate 3: シートで合否を決める

fullならGate 3が検収シート全文とともに表示されます。会話の要約ではなく、
次の5節を順に確認します。

```text
# Gate 3 — Acceptance
## 1. Confirmed identity
## 2. Terminal projection
- Assurance: full
- Runtime acceptance: pass
- Final acceptance: full_success
- Release gate: pass
## 3. Definition of done
- Contract checks: N1, N2, N3, N4, N5
## 4. Machine evidence
## 5. Stop reason
completed
```

`full`という一語だけで判定せず、節1が確認した依頼と一致すること、節2の
投影、節3のN1〜N5、節4のevidence path、節5の停止理由を照合します。
今回の機械証拠では、特にN2の出典束縛とN3の
`detected = accepted + excluded`を読みます。

記録はstate領域に残ります。

```bash
find "$D3C_LOOP_STATE" -maxdepth 2 -type f -print
```

終了時はREPLで`/exit`を入力します。現在のREPLは1セッション内の複数依頼を
未サポートなので、次の依頼は再起動して新しいGate 1から始めます。

### Gate 4になった場合

fullでなければGate 4がシート全文、節5、typed next actionsを表示します。
選択肢の意味は次のとおりです。

```text
# Gate 4 — Failure and next action
## Section 5
<machine-recorded stop reason>
## Typed next actions
- retry: available|unavailable — <reason>
- recovery_circle: available|unavailable — <reason>
- elevated_model: available|unavailable — <reason>
- pack_change: available|unavailable — <reason>
- human_directive: available — persisted confirmation required
- close: available — records no further action
```

- `retry`: 同じ構成でもう一度Gate 1から確認する。
- `recovery_circle`: 失敗証拠を引き継ぐ回復円環へ切り替える。
- `elevated_model`: 上位モデル構成を新しい値札とpinで確認する。
- `pack_change`: compatibleなadmitted packへ変更して再確認する。
- `human_directive`: 失敗runへ追加指示を保存・確認して同じworkspaceで継続する。
- `close`: 実行を増やさず閉じる。

現行REPLでは、失敗runに限り`/directive <instruction>`で追加指示を
提案できます。指示は資格情報検査後にhash付きで保存され、表示された
`/confirm-directive <hash>`を明示実行するまで継続runへ渡りません。
再試行・elevated・pack変更など、その他のGate 4選択肢は引き続き
新しいGate 1確認が必要で、勝手に次の処理へ進むことはありません。

## 2. packを1パラメータだけ変えてA/Bする

packの効果は、同じsuiteでpack pinだけを変えて測ります。ここでは既存
`cli-assist@1.1.0`を`1.1.1`へ複製し、C1出力の有界長だけを4,000から
2,000 byteへ狭めます。ローカル実験packはconformanceを通っても未admitted
です。境界シェルでは選択せず、bench A/Bだけに使います。
複製元の実体は`packs/cli-assist/1.1.0/assist.yaml`です。

現在利用できる承認済みpackとローカルpackは、profileとintentを固定して一覧できます。
各行の`SOURCE`は`admitted`または`local`です。

```bash
commandagent --extension-root packs --profile python-cli --intent create --packs
```

### scaffoldと1パラメータ変更

```bash
python3 workspace/management/scripts/scaffold.py pack cli-assist --from-version 1.1.0 --version 1.1.1
```

生成された`packs/cli-assist/1.1.1/assist.yaml`で、次の1行だけを変更します。

```yaml
      max_bytes_per_stream: 2000
```

他のsource、point、fields、C3 bindingは変えません。次にstrict decoder、
語彙、契約floor、exact-byte hashを検査し、greenになったbytesをpinします。
既存`pack.sha256`を上書きする操作ではなく、scaffoldの未pin directoryに
初回pinを作るコマンドです。

```bash
commandagent --pack-pin packs/cli-assist/1.1.1
```

管理用Python wrapperを使う代替経路は次です。

```bash
python3 workspace/management/scripts/pack_conformance.py --pack packs/cli-assist/1.1.1 --write-pin
```

新しいCLI直接アクションを通常は使い、管理用Python wrapperとの互換性確認が必要な場合だけ
後者を使います。どちらも既存pinの不一致を上書きしません。

出力の`exact_byte_hash`と`packs/cli-assist/1.1.1/pack.sha256`が一致することを
確認します。

```bash
commandagent --pack-verify packs/cli-assist/1.1.1
```

同じreportは管理用wrapperからも確認できます。

```bash
python3 workspace/management/scripts/pack_conformance.py --pack packs/cli-assist/1.1.1
```

### suiteをpinして再実行

既存A/B suiteを一時ファイルへ複製します。

```bash
cp workspace/management/bench/suites/cli-create-elevated-cli-assist-v1-1.toml /tmp/cli-assist-local-1.1.1.toml
```

一時suiteでは、次の3値だけを変更します。`<exact-byte-hash>`はconformanceが
出した`sha256:`付きの値です。モデル、goal、6run、入力は変えません。

```toml
id = "cli-assist-local-1-1-1"
pack_version = "1.1.1"
pack_hash = "<exact-byte-hash>"
```

まずdry-runでpinと6run計画を読み、その後に同じsuiteを実行します。

```bash
export D3C_PACK_RUN_ROOT=/tmp/commandagent-cli-pack-loop
python3 workspace/management/scripts/bench.py run --suite /tmp/cli-assist-local-1.1.1.toml --workspace-root "$D3C_PACK_RUN_ROOT" --dry-run
python3 workspace/management/scripts/bench.py run --suite /tmp/cli-assist-local-1.1.1.toml --workspace-root "$D3C_PACK_RUN_ROOT"
find "$D3C_PACK_RUN_ROOT" -maxdepth 2 -name uat-meta.json -print
```

実行が表示したcampaign directoryを`D3C_CAMPAIGN`へ設定し、自動分類します。
次の値は例なので、末尾を実際に表示されたdirectory名へ置き換えます。

```bash
export D3C_CAMPAIGN=/tmp/commandagent-cli-pack-loop/cli-assist-local-1-1-1-20260731-000000
python3 workspace/management/scripts/classify_runs.py "$D3C_CAMPAIGN" --out "$D3C_CAMPAIGN/classification.json"
```

### band差分の読み方

正式な比較基準は次の行で確認できます。

```bash
rg -n "Pack v1.1.0 pin|Pack arm full|Pack runs reaching|Pack renderer exposure|Reach-rate comparison" workspace/management/runs/band_summary_cli.md
```

新campaignについても、同じ順序で次を比べます。

1. pack ID、version、exact-byte hashが意図したpinか。
2. 6runの正直終端数とC系到達数。
3. renderer露出数。露出ゼロならpack効果はまだ観測していない。
4. full率とC3 violation分布。到達率差と合否差を混同しない。
5. family／executorの分母が同じか。

未admittedのローカルpackは正式`band_summary_cli.md`へ自動加算しません。
レビュー、admission、計測レポート登録後にだけ正式bandへ入れます。差がゼロでも
失敗ではなく、pin以外を固定した効果量ゼロという観測です。

## 3. 困ったとき

### `typed unknown`になった

実行は始まりません。曖昧な依頼へprofile、intent、材料、成果物を足します。
この例なら`data/snapshots/`、HTML、抽出、`output/records.json`を明記します。
登録外の語を推測でIDにせず、修正版のGate 1を待ちます。

### Gate 1の確認を拒否した

正常な安全動作です。確認記録がないためdispatchされません。モデル、値札、
contract、pack pin、または依頼が違う箇所を直し、新しいカードhashだけを
確認します。

### Gate 4でどれを選ぶか迷う

同条件の偶発失敗だけなら`retry`、失敗証拠を保った修復なら
`recovery_circle`、能力階級を変えるなら`elevated_model`、援助材料だけを
変えるA/Bなら`pack_change`、追加費用を使わないなら`close`です。いずれも
次の実行前に新しいGate 1の値札とpinを読み直します。
