# Providers

[日本語](../ja/providers.md) | [Guide index](../README.md)

CommandAgent supports separate executor and planner roles. `--provider` and
`--model` configure execution; `--planner-provider` and `--planner-model`
configure planning. If the provider roles match, the planner inherits the
executor model unless it is overridden.

## Provider matrix

| Provider | CLI value | Required key | Obtain/setup | CommandAgent endpoint | Configuration |
| --- | --- | --- | --- | --- | --- |
| Ollama | `ollama` | none for a local server | [Ollama quickstart](https://docs.ollama.com/quickstart) | `--ollama-host`, default `http://localhost:11434`; `/api/chat` is appended | `--provider ollama --model <model-id>` |
| LM Studio | `lm-studio` | none by default; `LM_STUDIO_API_TOKEN` when server authentication is enabled | [LM Studio local server](https://lmstudio.ai/docs/developer/core/server) | `--lm-studio-host`, default `http://localhost:1234`; OpenAI-compatible `/v1` routes are appended | `--provider lm-studio --model <model-id>` |
| OpenAI | `openai` | `OPENAI_API_KEY` | [Create an OpenAI API key](https://platform.openai.com/api-keys) | fixed `https://api.openai.com`; explicit `--api chat-completions` (default) or `--api responses` | process environment only |
| Gemini | `gemini` | `GEMINI_API_KEY` | [Create a Gemini API key in Google AI Studio](https://aistudio.google.com/app/apikey) | fixed Google Generative Language endpoints | process environment or workspace `.env` |

CommandAgent does not accept `GOOGLE_API_KEY` as a substitute for
`GEMINI_API_KEY`, even though some Google client libraries do. When planner and
executor use different cloud providers, configure both required keys and set an
explicit planner model.

## Configure credentials

`OPENAI_API_KEY` is read only from the process environment. It is intentionally
rejected from command arguments, presets, suite definitions, and workspace
`.env` files. `GEMINI_API_KEY` first checks the process environment and then
falls back to `<workspace>/.env`, where the workspace is the canonical `--cwd`
or current directory.

`LM_STUDIO_API_TOKEN` is optional and read only from the process environment.
Leave it unset for the default unauthenticated local server. Set it only when
LM Studio's Require Authentication setting is enabled.

### Shell environment

Export the key into the shell that launches CommandAgent:

```bash
export OPENAI_API_KEY="<secret>"
# or
export GEMINI_API_KEY="<secret>"

commandagent --provider openai --api responses --model <openai-model-id>
```

Do not run `echo $OPENAI_API_KEY`, `env`, `printenv`, or shell tracing to verify
the value on a shared screen or in captured logs. After startup, use `/status`
to confirm the effective provider and model settings; it does not print the key.

### Workspace `.env`

For Gemini only, you can instead create `.env` at the active workspace root:

```dotenv
GEMINI_API_KEY=<secret>
```

The small parser accepts one `KEY=value` per line, ignores blank lines and lines
starting with `#`, trims whitespace, and strips one pair of surrounding single
or double quotes. Do not add the shell-only `export` prefix. Values are not
expanded as shell expressions.

Restrict the file to its owner on Unix-like systems:

```bash
chmod 600 .env
```

The repository ignores `.env`, but verify ignore rules in any workspace before
committing. Never commit, paste, record, or show key values on screen. If a key
is exposed, revoke it at the provider and replace it.

## OpenAI model identity

Use an exact GPT-5.6 tier ID: `gpt-5.6-luna`, `gpt-5.6-terra`, or
`gpt-5.6-sol`. The ambiguous alias `gpt-5.6` is rejected because it routes to
Sol rather than preserving the requested tier. Terra also rejects invented
suffixes such as `-latest`; only the exact ID or a provider-published
`gpt-5.6-terra-YYYY-MM-DD` snapshot shape is accepted. Prefer a published
snapshot-qualified ID when repeatable comparisons matter. CommandAgent records
the requested and returned model IDs plus `system_fingerprint` in the provider
turn event so endpoint drift can be audited without exposing credentials.

OpenAI Chat Completions reasoning effort is opt-in. Set
`COMMANDAGENT_OPENAI_REASONING_EFFORT` in the process environment only when an
explicit effort value is required. If it is unset or empty, CommandAgent omits
the control from the request; it does not synthesize a model-specific default.
The same declaration is rendered as `reasoning.effort` for Responses.

API selection is also declaration-only. Omission means Chat Completions;
CommandAgent never infers an API from a model name. Responses native-tool turns
retain the provider's reasoning output items and replay them with subsequent
function outputs in the same run. Response IDs, service tier, cached input
tokens, and reasoning-token counts are recorded in provider turn events.

## LM Studio server and models

Start the server from LM Studio's Developer tab or with `lms`, then select an
exact model identifier returned by `/v1/models`:

```bash
lms server start
curl http://localhost:1234/v1/models
commandagent --provider lm-studio --model <model-id> \
  --lm-studio-host http://localhost:1234
```

The host flag accepts either `http://localhost:1234` or
`http://localhost:1234/v1`; CommandAgent normalizes the optional `/v1` suffix.
Chat Completions is the default. Select LM Studio's Responses endpoint with
`--api responses`. Native function tools are enabled by default; use
`--tool-protocol text` when measuring a model that does not reliably produce
structured tool calls.

LM Studio provider turns currently use the non-streaming client path. An
interactive REPL remains functional when `--stream on`, but output is rendered
after each provider turn completes.

## Ollama host and models

Ollama requires a running HTTP server and a locally available model. Local API
access at the default address does not require an API key. CommandAgent passes
the resolved `context_budget` as Ollama `options.num_ctx`, passes `num_predict`,
keeps a model loaded for 10 minutes, and appends API routes to the configured
host. `commandagent --doctor` displays both the context budget and the matching
Ollama `num_ctx` value. After the first provider turn loads the model, `ollama
ps` reports that value in its `CONTEXT` column.

### Ollama thinking

Use `--think` to send Ollama's top-level `think` request field for every role
whose resolved provider is Ollama. A bare flag sends boolean `true`; explicit
values use the unambiguous equals form:

```bash
commandagent --provider ollama --model qwen3 --think
commandagent --provider ollama --model gpt-oss:20b --think=high
commandagent --provider ollama --model qwen3 --think=false
```

Accepted values are `true`, `false`, `low`, `medium`, and `high`. Models such as
GPT-OSS require a level rather than a boolean. CommandAgent does not infer model
support: Ollama remains responsible for accepting or rejecting the value. When
neither the executor nor planner uses Ollama, `--think` is an error.

Ollama returns thinking-capable output separately in `message.thinking`.
CommandAgent intentionally excludes that text from the TUI, assistant content,
conversation history, and events; only the final `message.content` and tool
calls continue through the existing response path. See Ollama's official
[thinking documentation](https://docs.ollama.com/capabilities/thinking) for
model-specific behavior.

### Local setup

```bash
ollama serve
ollama pull <model-id>
curl http://localhost:11434/api/tags
commandagent --provider ollama --model <model-id>
```

`/api/tags` should list the exact model ID, including any tag. See the official
[Ollama API introduction](https://docs.ollama.com/api/introduction) and
[list-models endpoint](https://docs.ollama.com/api/tags).

### Remote or non-default host

Set the server's bind address in the Ollama process, then tell CommandAgent the
reachable base URL:

```bash
OLLAMA_HOST=0.0.0.0:11434 ollama serve
commandagent --ollama-host http://server.example:11434 \
  --provider ollama --model <model-id>
```

`OLLAMA_HOST` configures the Ollama server; CommandAgent itself reads
`--ollama-host`. Do not include `/api` in the flag value because CommandAgent
adds `/api/chat` and `/api/tags`. Exposing Ollama beyond localhost has network
security consequences; follow the official
[Ollama server configuration](https://docs.ollama.com/faq) and restrict access.

## Secret-handling checklist

- Store the OpenAI key only in the launching process environment. Gemini may
  also use workspace `.env`. Never put a cloud key in `config.toml`, a preset,
  a suite, a goal, or a command argument.
- Store an LM Studio token only in the launching process environment when
  server authentication is enabled; it is unnecessary for the default server.
- Set `.env` permissions to `600` where Unix permissions are available.
- Do not display a key value on screen, include it in screenshots, paste it into
  issues, or capture it in terminal transcripts.
- Keep `.env` out of version control and review staged files before committing.
- Use separate, least-privilege keys where the provider supports them, and
  rotate a key immediately after suspected exposure.
- Treat the Ollama host as a service endpoint. Do not expose an unauthenticated
  local server to an untrusted network.
- Apply the same restriction to LM Studio, especially when Serve on Local
  Network is enabled.
