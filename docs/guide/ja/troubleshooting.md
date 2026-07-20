# トラブルシューティング

[English](../en/troubleshooting.md) | [ガイド目次](../README.md)

最初に、アクティブなワークスペース、選択した preset、プロバイダの役割、model ID を確認します。
TUI の `/status` は実効設定と readiness、`/runs` は最近の run と recovery 情報を表示します。
runtime evidence は通常 `<workspace>/.anvil/runs/<run-id>/` に書き込まれます。

## `GEMINI_API_KEY is not set`

選択したいずれかのプロバイダ役割が Gemini で、プロセス環境とアクティブなワークスペースの
`.env` の両方で `GEMINI_API_KEY` が存在しないか空の場合に出る正確な起動エラーです。

1. [Google AI Studio](https://aistudio.google.com/app/apikey) でキーを取得します。
2. CommandAgent を起動するシェルで `GEMINI_API_KEY` を設定するか、
   `<workspace>/.env` に `GEMINI_API_KEY=<secret>` を置きます。
3. `.env` を使う場合、`--cwd` がそのファイルを含むディレクトリを指すことを確認します。
4. `GOOGLE_API_KEY` は使わないでください。CommandAgent が読むのは `GEMINI_API_KEY` だけです。
5. キーを表示せずに CommandAgent を再起動します。

[プロバイダ認証情報の設定](providers.md#認証情報の設定)も参照してください。OpenAI で対応する
エラーは `OPENAI_API_KEY is not set` であり、`OPENAI_API_KEY` を使って同様に解決します。

## `preflight: port N is busy`

メッセージには実際の数値ポートが入り、`lsof` で特定できれば所有する PID とコマンドも入ります。
次に例を示します。

```text
preflight: port 3011 is busy: pid 1234 (node)
```

### Preflight を実行する理由

この検査は、実効 profile が `nextjs` の generate-and-run アクションだけで実行されます。対象は
CLI または TUI の `plan-run` と `ultra-plan-run` です。ゴールから認識したポートを使い、ゴールに
ポートがなければ `3011` を使います。最初に `127.0.0.1:<port>` への bind を試し、失敗すると
使用中とみなします。`lsof` の検索は情報取得用なので、所有者が `unknown owner` の場合もあります。

### 選択肢と非対話時の動作

対話型端末では次の prompt を表示します。

```text
Choose [k]ill / [a]bort:
```

- `k` または `kill` は Unix で検出した PID に SIGTERM を送り、処理を続けます。PID を検出できなければ
  kill の選択は安全に失敗します。
- それ以外の回答では `preflight aborted: port N is busy` で中止します。
- `--yes` を使うと `--yes never auto-kills processes` で失敗します。
- TTY がない場合は `no TTY available for [k]ill/[a]bort` で失敗します。

既知のサービスを自分で停止または再設定するか、ゴール中の要求ポートを変更する方法を推奨します。
不明な PID を特定せずに終了しないでください。

## `preflight: interaction probe unavailable`

同じ Next.js の generate-and-run アクションで、CommandAgent は Playwright が利用可能か検査します。
利用できない場合、理由に続けて次の正確な remediation を表示します。

```text
run /setup-interaction-probe (or commandagent --setup-interaction-probe) to enable interaction release checks
```

動作検証に合格したと装うのではなく、interaction gate を縮退して run を続けます。このため最終 status が
partial になることがあります。ワークスペースから setup コマンドを実行し、Playwright のインストールが
完了してから再試行してください。管理対象ファイルは `.anvil/tools/interaction-probe` に残ります。
明示的な module 探索ディレクトリは `COMMANDAGENT_PLAYWRIGHT_DIR` または旧名の
`ANVIL_PLAYWRIGHT_DIR` で指定できます。

## フッター描画の問題

固定 footer が出力に重なる、cursor の跡が残る、terminal multiplexer 内で乱れる場合は、今回の
呼び出しで無効にします。

```bash
commandagent --footer off
```

scrollback の breadcrumb は残ります。`--no-footer`、トップレベル／preset の `footer = "off"`、
空でない `COMMANDAGENT_NO_FOOTER` または `ANVIL_NO_FOOTER` も代替手段です。
`NO_COLOR` は color を除きますが footer は無効にしません。異常終了後は、端末を reset または
開き直すと terminal emulator に残った scroll region を消せる場合があります。

## Model ID が存在しない

CommandAgent は model ID を選択したプロバイダへ渡します。プロバイダ横断の catalog を保持せず、
request 前に ID を検証しません。そのため、存在しない、利用できない、権限がない model は
プロバイダ呼び出し時に失敗します。

### 失敗の見え方

- Gemini は `Gemini streamGenerateContent API failed: <status>` または
  `Gemini interactions API failed: <status>` を表示します。
- OpenAI は `OpenAI Responses API failed: <status>` を表示します。
- Ollama は設定済み再試行後に `Ollama /api/chat failed: <status>` を表示します。
- TUI コマンドは error/failure block を表示して REPL に戻り、直接 CLI アクションは
  `error: ...` とともに非ゼロで終了します。

クラウドプロバイダの `provider_error` event には HTTP status と、run の `events.jsonl` 内に限定長の
response body snippet が入ります。端末エラー自体は意図的に短くなっています。正確な status code と
プロバイダの文言は変わる場合があります。

### 復旧

1. `/status` で実行と planner の両方が意図したプロバイダになっていることを確認します。
2. そのプロバイダの現在の model 一覧から利用可能な model ID をコピーします。ID はプロバイダ固有で、
   version や tag を含む場合があります。
3. Ollama では `ollama list` または `<ollama-host>/api/tags` を確認し、必要なら
   `ollama pull <model-id>` を実行します。
4. `--model` と、それとは別の `--planner-model`、または対応する preset フィールドを修正します。
5. ID が存在しても access denied なら、プロバイダ側の account/project、キー権限、billing、region、
   model availability を確認します。

決定的な無効 model エラーを隠すために検証を弱めたり、再試行回数を増やしたりしないでください。

## Ollama が起動していない

Ollama サーバーが停止または到達不能の場合、通常は最初の試行と設定済み再試行後の request/connect
error になり、connection refused など OS の文言を含むことがあります。設定 host を直接診断します。

```bash
curl http://localhost:11434/api/tags
ollama serve
```

Ollama がすでに application または service で管理されている場合、2 台目のサーバーを起動せず、
その manager から起動または再起動してください。その後、次を確認します。

- `--ollama-host` が CommandAgent を実行する環境から到達可能である。
- 値が `/api` を含まない base URL である。
- container から host への通信では container から見えるアドレスを使っている。
- firewall または proxy が接続を許可している。
- `/api/tags` に実行と planner の正確な model ID が表示される。

[Ollama のホストとモデル](providers.md#ollama-のホストとモデル)も参照してください。listen していない
サーバーは `--chat-timeout-secs` を増やしても直りません。
