# スラッシュコマンド

[English](../en/slash-commands.md) | [ガイド目次](../README.md)

対話型の `commandagent>` プロンプトでスラッシュコマンドを入力します。`/help` は dispatch と
同じコマンドレジストリから描画されるため、インストール済みバイナリの実行時の正となります。

## コマンド一覧

レジストリには主コマンドが 15 件あります。`/quit` は `/exit` の別名として独立して受け付ける
コマンド名なので、受け付ける名前は合計 16 件です。

| コマンド名 | `/help` に表示される使用法 | 動作 |
| --- | --- | --- |
| `/help` | `/help` | コマンド一覧、footer のヒント、入力キュー上限、複数行の継続入力、interrupt 動作を表示します。 |
| `/status` | `/status` | 実効設定とプロバイダの readiness を表示します。 |
| `/doctor` | `/doctor` | ネットワーク要求を行わず、設定ファイル、プロバイダ readiness、interaction probe、ローカル環境を診断します。 |
| `/runs` | `/runs` | ワークスペースの最近の run と recovery 可否を一覧表示します。 |
| `/resume` | `/resume [run-id\|yaml-path]` | recovery UltraPlan を準備し、確認後に再開します。引数が空の場合、利用可能なら最新の再開可能 run を選びます。 |
| `/plan` | `/plan` | アクティブな plan と現在の activity を表示します。 |
| `/plan-steps` | `/plan-steps <goal>` | step plan を生成・保存し、そのパスを表示します。 |
| `/plan-run` | `/plan-run <goal>` | step plan を生成して実行します。 |
| `/run-plan` | `/run-plan <path>` | 既存の step plan YAML ファイルを実行します。 |
| `/ultra-plan` | `/ultra-plan <goal>` | UltraPlan を生成・保存し、そのパスを表示します。 |
| `/ultra-plan-run` | `/ultra-plan-run <goal>` | UltraPlan を生成して実行します。 |
| `/run-ultra-plan` | `/run-ultra-plan <path>` | 既存の UltraPlan YAML ファイルを実行します。 |
| `/setup-interaction-probe` | `/setup-interaction-probe` | 管理対象の Playwright interaction readiness probe をインストールまたは検証します。 |
| `/model-probe` | `/model-probe` | 限定的なモデル動作プローブ一式を実行します。[モデル動作プローブ](../../model-probe.md)も参照してください。 |
| `/exit` | `/exit or /quit` | TUI を終了します。 |
| `/quit` | `/exit or /quit` | `/exit` の別名として TUI を終了します。 |

未知のスラッシュコマンドは入力エラーとして扱われ、候補がある場合は最も近いコマンドを案内します。
タスクの開始や run summary の生成は行いません。スラッシュなしの平文も実行せず、
`/ultra-plan-run <goal>` と `/plan-run <goal>` を案内します。plan コマンドの失敗は REPL を
終了せずに報告されます。

## インラインフラグ

スラッシュ入力では、残りの単語をゴールに結合する前に次の上書きを認識します。

| インラインフラグ | 値 | 効果 |
| --- | --- | --- |
| `--profile <PROFILE>` | profile 名 | このコマンドの設定済み profile を上書きし、自動 profile 推論を無効にします。 |
| `--style <STYLE>` | style 名 | このコマンドの設定済み style を上書きします。 |
| `--prompt-layout <stable\|legacy>` | `stable` または `legacy` | このコマンドのプロンプトセクション順序を上書きします。 |

plan を実行または生成するコマンドで有用です。すべてのスラッシュ入力で構文解析されますが、
`/help` や `/status` などの discovery コマンドは適用前に返ります。

```text
/plan-run --profile nextjs --style compact "3011ポートでアプリを作成する"
/ultra-plan-run --prompt-layout stable 現在のプロジェクトを改善する
```

## ゴールのテキストと引用符

パーサーはダブルクォート内を除いて空白とタブで分割し、ダブルクォートの区切り文字を削除して、
残った単語を 1 個の空白で結合します。シングルクォートとシェルのエスケープ構文に特別な意味は
ありません。これは小さなコマンドパーサーであり、シェルではありません。

アクションコマンドでは、インラインフラグをゴールの単語より前または途中に置けます。後続値がない
フラグトークンは上書きとして扱われず、ゴールに残ります。語句のまとまりを明確に保ちたい場合は
ダブルクォートで囲んでください。

## 複数行入力

待機中の `commandagent>` プロンプトでは、行末を `\` にするか、ダブルクォートを閉じずに Enter を
押すと入力を継続できます。エディターは継続プロンプト `... ` を表示し、次の行を待ちます。
すべてのダブルクォートを閉じ、現在行の末尾が `\` でない状態で Enter を押すとコマンドを送信します。

```text
commandagent> /ultra-plan-run Build a dashboard \
... with accessible navigation
commandagent> /plan-run "Create a CLI that
... validates configuration"
```

既存の単語パーサーへ渡す前に、エディターは継続用の末尾 `\` を削除し、各行を 1 個の空白で
結合します。そのため、継続用バックスラッシュと改行は送信されるコマンドに含まれません。
これはエディターの継続入力であり、シェルのエスケープではありません。

## `$(cat <path>)` 展開

インラインフラグの構文解析後、ゴール内のリテラルな `$(cat <path>)` 形式を、参照先ファイルの
UTF-8 内容で繰り返し置換します。パスはワークスペースの path guard で解決されます。相対パスは
アクティブなワークスペースを起点とし、外へ脱出するパスは拒否されます。これは内部展開であり、
シェルを起動せず、ほかのコマンド置換もサポートしません。

```text
/ultra-plan-run --profile nextjs "$(cat prompts/site-goal.txt)"
```

複数の形式は左から順に展開されます。開始文字列 `$(cat ` に閉じる `)` がなければ、通常の
ゴールテキストとして残ります。ファイルがない、許可されない、UTF-8 でない場合はスラッシュ
コマンドが失敗します。

## プロファイル推論

CLI と選択した preset のどちらでも profile を明示していない場合、各スラッシュコマンドは展開後の
ゴールとワークスペースから profile を推論できます。

1. Next.js のゴールトークンがあれば `nextjs` を選びます。
2. それ以外で Python CLI のゴールトークンがあれば `python-cli` を選びます。
3. それ以外で `package.json` の dependency または devDependency に `next` があれば
   `nextjs` を選びます。
4. それ以外で `pyproject.toml` があれば `python-cli` を選びます。
5. それ以外は設定済み fallback（通常は `generic`）を維持します。

ゴールの根拠はワークスペースの根拠より優先されます。インライン `--profile`、CLI `--profile`、
preset の `profile` のいずれかがあれば、明示値が `generic` でも推論を無効にします。
この処理は `data` profile を推論しないため、明示的に選択する必要があります。

## TUI の注意点

- コマンド実行中に Enter を押した入力は、最大 10 行、各行最大 4096 バイトまでキューに入ります。
  Backspace で保留中の入力を編集できます。
- Ctrl-C は interrupt します。Esc は空でない保留入力を消去し、それ以外では interrupt します。
  interrupt を繰り返すと強制 finalize します。
- 実効 profile が `nextjs` の場合、`/plan-run` と `/ultra-plan-run` は Next.js の使用中ポートと
  interaction-probe preflight を実行します。
- `/resume` はワークスペースの drift を検査し、TUI 起動時に `--yes` を指定していなければ確認を求めます。
- `/exit` と `/quit` は、trim 後の入力全体と一致した場合だけ終了コマンドとして認識されます。
