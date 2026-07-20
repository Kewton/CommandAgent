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
| OpenAI | `openai` | `OPENAI_API_KEY` | [Create an OpenAI API key](https://platform.openai.com/api-keys) | fixed `https://api.openai.com/v1/responses` | process environment or workspace `.env` |
| Gemini | `gemini` | `GEMINI_API_KEY` | [Create a Gemini API key in Google AI Studio](https://aistudio.google.com/app/apikey) | fixed Google Generative Language endpoints | process environment or workspace `.env` |

CommandAgent does not accept `GOOGLE_API_KEY` as a substitute for
`GEMINI_API_KEY`, even though some Google client libraries do. When planner and
executor use different cloud providers, configure both required keys and set an
explicit planner model.

## Configure credentials

CommandAgent first checks the process environment for the exact key name. If it
is absent or empty, it reads `<workspace>/.env`, where the workspace is the
canonical `--cwd` or current directory. A non-empty process value wins over
`.env`.

### Shell environment

Export the key into the shell that launches CommandAgent:

```bash
export OPENAI_API_KEY="<secret>"
# or
export GEMINI_API_KEY="<secret>"

commandagent --provider openai --model <openai-model-id>
```

Do not run `echo $OPENAI_API_KEY`, `env`, `printenv`, or shell tracing to verify
the value on a shared screen or in captured logs. After startup, use `/status`
to confirm the effective provider and model settings; it does not print the key.

### Workspace `.env`

Alternatively, create `.env` at the active workspace root:

```dotenv
OPENAI_API_KEY=<secret>
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

## Ollama host and models

Ollama requires a running HTTP server and a locally available model. Local API
access at the default address does not require an API key. CommandAgent passes
`num_predict`, keeps a model loaded for 10 minutes, and appends API routes to the
configured host.

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

- Store cloud keys in the launching process environment or the workspace
  `.env`, never in `config.toml`, a preset, a goal, or a command argument.
- Set `.env` permissions to `600` where Unix permissions are available.
- Do not display a key value on screen, include it in screenshots, paste it into
  issues, or capture it in terminal transcripts.
- Keep `.env` out of version control and review staged files before committing.
- Use separate, least-privilege keys where the provider supports them, and
  rotate a key immediately after suspected exposure.
- Treat the Ollama host as a service endpoint. Do not expose an unauthenticated
  local server to an untrusted network.
