# プロバイダ

[English](../en/providers.md) | [ガイド目次](../README.md)

CommandAgent は実行役割と planner 役割を分けられます。`--provider` と `--model` は実行を設定し、
`--planner-provider` と `--planner-model` は planning を設定します。プロバイダが同じ場合、
上書きしなければ planner は実行モデルを継承します。

## プロバイダ対応表

| プロバイダ | CLI 値 | 必要なキー | 取得／セットアップ先 | CommandAgent の endpoint | 設定方法 |
| --- | --- | --- | --- | --- | --- |
| Ollama | `ollama` | ローカルサーバーでは不要 | [Ollama quickstart](https://docs.ollama.com/quickstart) | `--ollama-host`、既定値 `http://localhost:11434`。`/api/chat` を付加 | `--provider ollama --model <model-id>` |
| LM Studio | `lm-studio` | 既定では不要。サーバー認証時は `LM_STUDIO_API_TOKEN` | [LM Studio local server](https://lmstudio.ai/docs/developer/core/server) | `--lm-studio-host`、既定値 `http://localhost:1234`。OpenAI 互換の `/v1` ルートを付加 | `--provider lm-studio --model <model-id>` |
| OpenAI | `openai` | `OPENAI_API_KEY` | [OpenAI API キーの作成](https://platform.openai.com/api-keys) | 固定の `https://api.openai.com`。明示 `--api chat-completions`（既定）または `--api responses` | プロセス環境のみ |
| Gemini | `gemini` | `GEMINI_API_KEY` | [Google AI Studio で Gemini API キーを作成](https://aistudio.google.com/app/apikey) | 固定の Google Generative Language endpoint | プロセス環境またはワークスペース `.env` |

Google の一部 client library と異なり、CommandAgent は `GOOGLE_API_KEY` を
`GEMINI_API_KEY` の代わりに受け付けません。planner と実行で異なるクラウドプロバイダを使う場合、
両方の必要キーを設定し、planner model を明示してください。

## 認証情報の設定

`OPENAI_API_KEY` はプロセス環境だけから読みます。コマンド引数、preset、suite 定義、
workspace の `.env` からの指定は意図的に拒否します。`GEMINI_API_KEY` は最初にプロセス環境を
確認し、存在しない場合は `<workspace>/.env` へフォールバックします。workspace は正規化した
`--cwd` または現在のディレクトリです。

`LM_STUDIO_API_TOKEN` は任意で、プロセス環境だけから読みます。既定の認証なしローカル
サーバーでは未設定のままにし、LM Studio の Require Authentication を有効にした場合だけ
設定してください。

### シェル環境

CommandAgent を起動するシェルへキーを export します。

```bash
export OPENAI_API_KEY="<secret>"
# または
export GEMINI_API_KEY="<secret>"

commandagent --provider openai --api responses --model <openai-model-id>
```

共有画面や採取されるログで値を確認するために、`echo $OPENAI_API_KEY`、`env`、`printenv`、
shell trace を実行しないでください。起動後は `/status` で実効 provider と model の設定を確認できます。
キーは表示されません。

### ワークスペースの `.env`

Gemini に限り、代わりにアクティブなワークスペース直下へ `.env` を作成できます。

```dotenv
GEMINI_API_KEY=<secret>
```

小さなパーサーは 1 行 1 個の `KEY=value` を受け付け、空行と `#` で始まる行を無視し、空白を
trim して、前後を囲むシングルクォートまたはダブルクォートを 1 組取り除きます。シェル専用の
`export` prefix は追加しないでください。値をシェル式として展開することもありません。

Unix 系システムでは所有者だけにファイルを許可してください。

```bash
chmod 600 .env
```

このリポジトリでは `.env` を ignore していますが、commit 前に各ワークスペースの ignore rule を
確認してください。キー値を commit、貼り付け、記録、画面表示しないでください。露出した場合は
プロバイダ側で revoke して交換してください。

## OpenAI モデルID

実行モデルには厳密ID `gpt-5.6-luna` を使います。別の Sol モデルへ解決され得る曖昧な
alias `gpt-5.6` は拒否されます。反復可能な比較が必要な場合は、提供されているなら
provider 公開の日付suffix付きLuna snapshot IDを優先してください。CommandAgent は返却された
model IDと`system_fingerprint`をprovider turn eventへ記録し、資格情報を露出せずendpoint driftを
監査可能にします。

OpenAI Chat Completions の reasoning effort は明示指定時だけ有効です。必要な場合だけ
`COMMANDAGENT_OPENAI_REASONING_EFFORT` をプロセス環境に設定してください。未設定または空文字なら
CommandAgent は request に制御値を含めず、model 別の既定値も合成しません。Responses では同じ宣言を
`reasoning.effort` として送ります。

API 選択も宣言だけで決まります。省略時は Chat Completions で、モデル名から API を推測しません。
Responses の native-tool turnでは、providerが返したreasoning output itemを保持し、同一runの後続
function outputとともに再送します。response ID、service tier、cached input token、reasoning token数は
provider turn eventへ記録します。

## LM Studio のサーバーとモデル

LM Studio の Developer タブまたは `lms` でサーバーを起動し、`/v1/models` が返す正確な
モデルIDを指定します。

```bash
lms server start
curl http://localhost:1234/v1/models
commandagent --provider lm-studio --model <model-id> \
  --lm-studio-host http://localhost:1234
```

host には `http://localhost:1234` と `http://localhost:1234/v1` のどちらも指定でき、
任意の `/v1` 接尾辞は CommandAgent が正規化します。既定は Chat Completions です。
LM Studio の Responses endpoint を使う場合は `--api responses` を指定します。native function
tool は既定で有効です。構造化 tool call が安定しないモデルを測定する場合は
`--tool-protocol text` を使用してください。

現在、LM Studio provider turn は非ストリーミング経路を使います。REPL で `--stream on` でも
機能は維持されますが、出力は provider turn 完了後に描画されます。

## Ollama のホストとモデル

Ollama には実行中の HTTP サーバーとローカルで利用可能なモデルが必要です。既定アドレスへの
ローカル API アクセスには API キーが不要です。CommandAgent は `num_predict` を渡し、モデルを
10 分間ロードしたままにし、設定した host に API route を追加します。

### ローカルセットアップ

```bash
ollama serve
ollama pull <model-id>
curl http://localhost:11434/api/tags
commandagent --provider ollama --model <model-id>
```

`/api/tags` に tag を含む正確な model ID が表示されることを確認してください。公式の
[Ollama API introduction](https://docs.ollama.com/api/introduction) と
[モデル一覧 endpoint](https://docs.ollama.com/api/tags)も参照してください。

### リモートまたは既定値以外のホスト

Ollama process でサーバーの bind address を設定し、CommandAgent に到達可能な base URL を
指定します。

```bash
OLLAMA_HOST=0.0.0.0:11434 ollama serve
commandagent --ollama-host http://server.example:11434 \
  --provider ollama --model <model-id>
```

`OLLAMA_HOST` は Ollama サーバーを設定し、CommandAgent 自体は `--ollama-host` を読みます。
CommandAgent が `/api/chat` と `/api/tags` を追加するため、フラグ値に `/api` を含めないでください。
localhost の外へ Ollama を公開するとネットワーク上のセキュリティに影響します。公式の
[Ollama サーバー設定](https://docs.ollama.com/faq)に従い、アクセスを制限してください。

## 秘密情報の取り扱いチェックリスト

- OpenAI キーは起動プロセスの環境だけに保存します。Gemini は workspace `.env` も利用できます。
  クラウドキーを `config.toml`、preset、suite、ゴール、コマンド引数には保存しません。
- LM Studio tokenはサーバー認証を有効にした場合だけ、起動プロセスの環境へ保存します。
- Unix permission が使える環境では `.env` を `600` に設定します。
- キー値を画面表示、screenshot、issue への貼り付け、端末 transcript の採取対象にしません。
- `.env` を version control から除外し、commit 前に stage 済みファイルを確認します。
- プロバイダが対応する場合は用途別の最小権限キーを使い、露出が疑われたら直ちに rotate します。
- Ollama host をサービス endpoint として扱い、認証のないローカルサーバーを信頼できない
  ネットワークへ公開しません。
- LM StudioでServe on Local Networkを有効にする場合も同じ制限を適用します。
