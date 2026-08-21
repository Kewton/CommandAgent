# チュートリアル: ターミナルと GUI で最初の 1 本を動かす

[English](../en/tutorial.md) | [CLI 入門](../../user/getting-started-cli.md)
| [GUI 入門](../../user/getting-started-gui.md)

所要時間は約 20 分です。掲載しているターミナル出力とスクリーンショットは
すべて実際の画面で、このリポジトリのビルド（`commandagent 0.1.0`、コミット
`15b7e362`）をローカルの Ollama モデルで動かして取得しました。モデルが違えば
計画・phase・判定は変わりますが、流れと画面は同じです。

## はじめる前に

### 必要なもの

- ビルド済みまたはインストール済みの `commandagent`（`cargo install --path .`
  か [`scripts/install.sh`](../../../scripts/install.sh)）。
- 起動中の Ollama と、pull 済みのモデル 1 つ以上。ターミナルの録画は製品既定の
  `qwen3.6:27b-coding-nvfp4`、下の doctor 出力は `qwen3.8:27b-mlx` で取得
  しました。どちらも手元の `ollama list` にある正確な ID に置き換えてください。
- 使い捨ての空 Git リポジトリ。CommandAgent は起動したディレクトリの中の
  ファイルを編集します。[セキュリティモデル](../../../SECURITY.md)を読むまで、
  大事なチェックアウトの中で起動しないでください。

```bash
mkdir -p ~/commandagent-tutorial && cd ~/commandagent-tutorial
git init -q && git commit -q --allow-empty -m "empty workspace"
```

### このチュートリアルで扱わないこと

設定ファイル・preset・pack の詳細と、リモートプロバイダーは扱いません。
それらは [CLI 入門](../../user/getting-started-cli.md)と
[設定ガイド](configuration.md)にあります。

## Step 1 — `--doctor` でセットアップを確認する

`--doctor` は、モデルを呼ぶ前に、バイナリ・プロバイダー・ワークスペースの
準備状況を教えてくれます。使い捨てワークスペースで実行します。

```bash
commandagent --provider ollama --model "<your-model>" --doctor
```

録画マシンでの実際の出力です（パスは短縮）。

```text
CommandAgent doctor: warnings
✓ Model                  qwen3.8:27b-mlx (source=CLI; detail=flag)
✓ Provider               ollama (source=CLI; detail=flag)
✓ Planner model          qwen3.8:27b-mlx (source=default; detail=default)
✓ Planner provider       ollama (source=default; detail=default)
✓ Profile                generic (source=default; detail=default)
✓ Config file            .../cli-workspace/.commandagent/config.toml: not found (optional)
✓ Pack selection         no pack selected
✓ Ollama                 http://localhost:11434/api/tags reachable; 24 model tag(s)
✓ Ollama executor model  qwen3.8:27b-mlx is present in /api/tags
✓ Ollama planner model   qwen3.8:27b-mlx is present in /api/tags
✓ Playwright probe       playwright 1.61.1 available (managed_interaction_probe)
! TTY                    stdin=false, stdout=false, stderr=false
  Remediation: run from an interactive terminal when validating TUI behavior
✓ Workspace              .../cli-workspace is writable (temporary file created and removed)
✓ Workspace .env         .../cli-workspace/.env is not present
```

まず 2 行のモデル行を見てください。`--planner-model` を渡さない限り、計画
モデルは実行モデルと同じになります。`!` は対処法付きの警告、`✗` は先に
直す必要がある項目です。上の `TTY` 警告は、doctor をスクリプトから実行した
ためで、ターミナルから実行すれば出ません。

## Step 2 — REPL で最初の依頼をする

### REPL を起動する

```bash
commandagent --provider ollama --model "<your-model>" --profile python-cli
```

バナーには、有効なモデル・プロバイダー・profile・ワークスペース、この
セッションのイベントログの保存先、そして画面下に固定されるステータス
フッターが表示されます。

```text
  ╭──────────────────────────────────────╮
  │   COMMANDAGENT · local-first agent   │
  ╰──────────────────────────────────────╯

commandagent 0.1.0 build=15b7e362
model=qwen3.6:27b-coding-nvfp4 (flag) provider=ollama (flag) planner=qwen3.6:27b-coding-nvfp4 (default) planner_provider=ollama (default)
mode=Act cwd=/private/tmp/commandagent-demo/cli-workspace-2
context_budget=65536 (default) timeout=600s (default:local_provider) profile=python-cli (flag) ...
run_log=/private/tmp/commandagent-demo/cli-workspace-2/.anvil/runs/<run-id>/events.jsonl
start: plain-text request → review Gate 1 → /confirm <hash> | help: /help
[act] provider:ollama model:qwen3.6:27b-coding-nvfp4 ctx:65536 tokens:n/a
commandagent>
```

`/status` で同じ情報をいつでも表示でき、`/help` で全スラッシュコマンドを
一覧できます。このステップの録画全体は [README](../../../README.ja.md#デモ)
の CLI GIF です。

### 依頼を入力して Gate 1 カードを読む

目標を普通の文として入力します。スラッシュコマンドで始めないでください。
境界シェルが普通の依頼を **Gate 1 カード**に変えて待機します。

```text
commandagent> Create a CLI --pattern filter command
```

![ターミナルの Gate 1 カード](../../assets/tutorial/cli-gate1.png)

カードの構成は毎回同じです。

| 節 | 意味 |
| --- | --- |
| 実行する内容 | 依頼、判定された intent / profile / family、束縛される契約。 |
| 必須チェック | full 判定に必要なチェック（`python-cli` なら `C1`〜`C4`）。 |
| 類似実行の結果 | 比較可能な記録のうち全チェックに合格した件数と、証跡の場所。 |
| ファイルへのアクセス | 実行が変更してよい唯一のディレクトリ。 |
| モデルとプリセット | 計画／実行モデル、preset、固定された検証パック。 |
| 確認 | カードのハッシュ。内容が 1 つでも変わるとこの値も変わります。 |

この時点ではまだ何も実行されていません。カードが意図と違うなら確認せず、
依頼を書き直してください。新しいカードが出ます。

### 確認して実行を見守る

カードの最後の行に、入力すべきコマンドがそのまま書かれています。画面に出た
ハッシュをコピーしてください。下の値は録画のもので、手元とは一致しません。

```text
commandagent> /confirm sha256:<card-hash>
```

REPL は `Persisted confirmation` と `Dispatching python-cli × create × filter.`
を表示し、phase 分割された `ultra-plan` 実行が始まります。フッターには現在の
phase、その中の step、実行中のツール、合計／プロバイダーの経過時間が出ます。
`Esc` で中断すると復旧用プランが残ります。録画では planner が
`project-scaffolding`、`core-filter-logic`、`edge-cases-and-validation` の
3 phase を生成し、それぞれが自身の `✓ verify` 行で終わりました。実行全体は
録画マシンで約 11 分でした。小さいモデルや短い目標なら早く終わります。

### 結果を読む

実行が終わると REPL は **D-3c acceptance sheet** を表示してプロンプトに
戻ります。読むべきは `Terminal projection` ブロックで、3 つの事実が意図的に
分けて書かれています。

1. **Status** — コマンドが完了したか（`completed`）、止まったか（`failed`、
   `interrupted`）。
2. **Acceptance** — `Runtime acceptance` と `Final acceptance` は、プラン自身の
   チェックが通ったか（録画では `full_success`）。
3. **Assurance** — 独立した検証が実際にどこまで走ったか。
   `static (cli_probe_not_run)` は CLI 動作プローブが実行されておらず、静的な
   証跡しか無いことを意味します。

Gate はこの 3 つから導かれます。録画がコマンド完了・final acceptance
`full_success` なのに `Gate 4 — Failure and next action` で終わるのはこのため
です。プローブ無しでは完全な `Gate 3` とは呼べません。シートはそのあとに
利用できる `Typed next actions` — `retry`、`elevated_model`、`pack_change`、
`human_directive`、`close` — を列挙し、判定を勝手に引き上げることはしません。

`/exit` で終了します。これらがどこに保存されるかは次のセクションで示します。

## Step 3 — 同じ流れを GUI から行う

管理 GUI はモデルと直接は話しません。同じ Gate 1 カードを表示し、同一の CLI
実行を別の **execution root** に委譲して、そのイベントログをブラウザに投影
します。

### gui_server を起動する

静的エクスポートを一度ビルドし、互いに重ならない 3 つのディレクトリ —
リポジトリ（読み取り専用の証跡）、execution root（委譲された CLI が書ける
唯一の場所）、extension root（非公開 pack）— を指定して起動します。詳細は
[GUI セットアップ](../../user/gui-setup.md)にあります。

```bash
cd gui && GUI_BASE_PATH=/ npm run build && cd ..
cargo run --features gui --bin gui_server -- \
  --port 4173 --base-path / --static-dir gui/out \
  --repository-root . \
  --execution-root /path/to/trial-workspace \
  --extension-root /path/to/commandagent-extensions \
  --trial-token-auth off \
  --commandagent-bin target/release/commandagent
```

同じコマンドに `--check` を付けると、ポートを開かずに事前検査だけを行い、
3 つのルートが重なっていないか、extension root が非公開（`0700`）かを含めて
項目ごとに `ok`/`ng` を表示します。

### 初回カード

`http://127.0.0.1:4173/` を開きます。概要の最上部にある **はじめに** カードが、
doctor のうち GUI に関係する 3 項目 — execution root、CLI バイナリ、Trial
アクセス — を再確認し、サンプル目標を提示します。

![はじめにカード付きの概要](../../assets/tutorial/gui-01-overview.png)

### サンプル目標と Gate 1

**サンプル目標を Trial に入力** を押すと、Step 2 と同じ目標、`python-cli`
profile、承認済みの検証パック（`cli-assist@1.0.0`）が入った状態でトライアル
ページが開きます。モデル欄は意図的に空です。**実行モデル** と **計画モデル**
に正確なモデル ID を入力してください。

![サンプル目標が入ったトライアルフォーム](../../assets/tutorial/gui-03-trial-form-filled.png)

**契約と見積りを確認** でサーバーに Gate 1 カードを要求します。ターミナルと
同じカードに加えて、右側に類似実行の平均所要時間、書き込み可能なディレクトリ、
カードのハッシュが出ます。確認チェックを入れるまで実行ボタンは無効のままです。

![GUI の Gate 1 カード](../../assets/tutorial/gui-04-gate1.png)

### Gate 2: 委譲された実行を見守る

**確認して CLI を実行** の後、ページは Gate 2 表示に切り替わります。意図的に
2 つのものが分けて表示されます。**実行状態**（`running` と、`events.jsonl`
から再構成した phase 一覧）と **監視の健全性**（`接続中` / `不安定` / `切断`）
です。ブラウザがサーバーを見失っても CLI は動き続けます。ページを再読み込みし、
URL のセッション ID で **監視を再接続** してください。
再接続後も、経過時間はセッションの開始時刻から継続し、平均所要時間は起動前確認と
同じ計測値に戻ります。

![phase 進捗付きの Gate 2 表示](../../assets/tutorial/gui-05-gate2-start.png)

### 結果と履歴

終了すると、判定・保証水準・プロセス状態が 3 つの別々の事実として表示され、
`summary.md`、イベント末尾、確認記録を開けます。任意で追加の依頼を入力でき、
それも独自の確認を通ります。

![Gate 3/4 の結果](../../assets/tutorial/gui-07-result.png)

GUI から起動したセッションは同じページの **GUI Trial 実行履歴** に、pack の
pin と再接続リンク付きで一覧されます。

![Trial 履歴](../../assets/tutorial/gui-09-history.png)

## Step 4 — 証跡はどこにあるか

どちらの経路も、実行したワークスペースの中に書き込み、リポジトリには
書き込みません。

| パス（ワークスペース配下） | 内容 |
| --- | --- |
| `.anvil/runs/<run-id>/events.jsonl` | 実行の全イベント。1 行 1 JSON。GUI の phase 一覧と REPL のフッターはこのファイルの投影です。 |
| `.anvil/runs/<run-id>/summary.md` | Status、verdict、assurance、`Stop reason`、`Next action`、復旧コマンド。Gate 4 のあとは最初にこれを読みます。 |
| `.anvil/runs/<run-id>/<card-hash>.json` | 確認済みの Gate 1 identity: 依頼、profile、モデル、pack pin、必須チェック。 |
| `.anvil/plans/`、`.anvil/repairs/` | `/resume` や提示されたコマンドが使う復旧プランと修復プロンプト。 |

スクリプトから同じ terminal facts を得るには `commandagent --summary-json` を
使います。[headless サマリー](../../user/headless.md)を参照してください。

## うまくいかないとき

| 表示 | 意味 | 対処 |
| --- | --- | --- |
| 起動時に `Model ID does not exist` | その ID が `ollama list` / プロバイダーのカタログに無い。 | 正確な ID を使う。[トラブルシューティング](troubleshooting.md#model-id-が存在しない)参照。 |
| `D-3c Gate 1 confirmation is required before execution. Start with a plain-text request, review the Gate 1 card, then enter /confirm <hash>.` | 依頼ではなく `/ultra-plan-run` や `/plan-run` を直接入力した。 | 依頼を普通の文で入力してカードを確認し、`/confirm <hash>` を入力する。 |
| `Assurance: static` の Gate 4 | 実行が自前の検証に到達する前に止まった。 | `summary.md` の `Stop reason` を読み、提示された復旧コマンドか修正した依頼を使う。 |
| GUI: `Recovery required` のリース | 以前の委譲実行に終端イベントが無い。 | 読み取り専用の[リース復旧手順](../../user/gui-trial.md#workspace-lease-inspection-and-recovery)に従う。`.anvil/` を消さない。 |
| GUI: `403 trial_origin_not_allowed` | サーバーが許可していないオリジンから到達した。 | `GUI_TRIAL_ALLOWED_ORIGINS` にブラウザの正確なオリジンを設定して再起動。 |

## 次のステップ

- 同じ目標で pack の 2 バージョンを比較する:
  [pack A/B](../../user/getting-started-cli.md#6-compare-one-pack-variable-at-a-time)。
- プロバイダーとモデルを preset に入れて、コマンドラインを
  `commandagent --preset local_cli` まで短くする: [設定](configuration.md#preset)。
- GUI のウィザードで非公開の検証パックを作る:
  [GUI 拡張](../../user/gui-extensions.md#pack-creation-wizard)。
- `ingest` profile の 4 ゲート・ウォークスルー（日本語）を読む:
  [最初の 1 周](../../user/first-loop.md)。
