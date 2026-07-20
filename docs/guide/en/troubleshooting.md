# Troubleshooting

[日本語](../ja/troubleshooting.md) | [Guide index](../README.md)

Start by confirming the active workspace, selected preset, provider roles, and
model IDs. In the TUI, `/status` shows effective configuration and readiness;
`/runs` shows recent run and recovery information. Runtime evidence is normally
written under `<workspace>/.anvil/runs/<run-id>/`.

## `GEMINI_API_KEY is not set`

This is the exact startup error when either selected provider role is Gemini
and `GEMINI_API_KEY` is absent or empty in both the process environment and the
active workspace's `.env`.

1. Obtain a key from [Google AI Studio](https://aistudio.google.com/app/apikey).
2. Set `GEMINI_API_KEY` in the shell that launches CommandAgent, or place
   `GEMINI_API_KEY=<secret>` in `<workspace>/.env`.
3. If using `.env`, confirm that `--cwd` points at the directory containing it.
4. Do not use `GOOGLE_API_KEY`; CommandAgent reads only `GEMINI_API_KEY`.
5. Restart CommandAgent. Do not print the key while checking it.

See [provider credential configuration](providers.md#configure-credentials).
The analogous OpenAI error is `OPENAI_API_KEY is not set` and has the same
resolution using `OPENAI_API_KEY`.

## `preflight: port N is busy`

The message contains the actual numeric port and, when `lsof` can identify it,
the owning PID and command. For example:

```text
preflight: port 3011 is busy: pid 1234 (node)
```

### Why the preflight runs

The check runs only for the effective `nextjs` profile on generate-and-run
actions: CLI or TUI `plan-run` and `ultra-plan-run`. It uses a port recognized
in the goal, or `3011` when the goal names no port. CommandAgent first attempts
to bind `127.0.0.1:<port>`; a failed bind is treated as busy. `lsof` lookup is
informational, so the owner can be `unknown owner`.

### Choices and non-interactive behavior

In an interactive terminal, the prompt is:

```text
Choose [k]ill / [a]bort:
```

- `k` or `kill` sends SIGTERM to the detected PID on Unix and continues. If no
  PID was detected, the kill choice fails safely.
- Any other answer aborts with `preflight aborted: port N is busy`.
- With `--yes`, CommandAgent fails with `--yes never auto-kills processes`.
- Without a TTY, it fails with `no TTY available for [k]ill/[a]bort`.

Prefer stopping or reconfiguring the known service yourself, or change the
requested port in the goal. Never kill an unknown PID without identifying it.

## `preflight: interaction probe unavailable`

For the same Next.js generate-and-run actions, CommandAgent checks whether
Playwright is available. If not, it prints the reason followed by this exact
remediation:

```text
run /setup-interaction-probe (or commandagent --setup-interaction-probe) to enable interaction release checks
```

The run continues with degraded interaction gates rather than pretending that
behavioral verification passed. The final status can therefore be partial. Run
the setup command from the workspace, allow its Playwright installation to
finish, then retry. The managed files remain under
`.anvil/tools/interaction-probe`. An explicit module search directory can be
set with `COMMANDAGENT_PLAYWRIGHT_DIR` or legacy `ANVIL_PLAYWRIGHT_DIR`.

## Footer rendering problems

If the fixed footer overlaps output, leaves cursor artifacts, or behaves badly
inside a terminal multiplexer, disable it for that invocation:

```bash
commandagent --footer off
```

Scrollback breadcrumbs remain. `--no-footer`, top-level/preset
`footer = "off"`, and non-empty `COMMANDAGENT_NO_FOOTER` or
`ANVIL_NO_FOOTER` are alternative controls. `NO_COLOR` removes color but does
not disable the footer. After an abnormal exit, resetting or reopening the
terminal can clear a scroll region left by the terminal emulator.

## Model ID does not exist

CommandAgent passes model IDs to the selected provider; it does not maintain a
cross-provider catalog or validate an ID before the request. A nonexistent,
unavailable, or unauthorized model therefore fails at provider-call time.

### What the failure looks like

- Gemini surfaces `Gemini streamGenerateContent API failed: <status>` or
  `Gemini interactions API failed: <status>`.
- OpenAI surfaces `OpenAI Responses API failed: <status>`.
- Ollama surfaces `Ollama /api/chat failed: <status>` after configured retries.
- A TUI command prints an error/failure block and returns to the REPL; a direct
  CLI action exits nonzero with `error: ...`.

Cloud-provider `provider_error` events include the HTTP status and a bounded
response-body snippet in the run's `events.jsonl`; the terminal error itself is
intentionally shorter. Exact status codes and provider wording can vary.

### Recovery

1. Confirm `/status` shows the intended provider for both executor and planner.
2. Copy an available model ID from that provider's current model list; IDs are
   provider-specific and can include versions or tags.
3. For Ollama, run `ollama list` or query `<ollama-host>/api/tags`, then
   `ollama pull <model-id>` if needed.
4. Correct `--model` and, separately, `--planner-model` or their preset fields.
5. If the ID exists but access is denied, check the account/project, key
   permissions, billing, region, and model availability at the provider.

Do not weaken verification or increase retries to hide a deterministic invalid
model error.

## Ollama is not running

A stopped or unreachable Ollama server normally appears as a request/connect
error after the initial attempt and configured retries, often with operating-
system text such as connection refused. Diagnose the configured host directly:

```bash
curl http://localhost:11434/api/tags
ollama serve
```

If Ollama is already managed as an application or service, start or restart it
through that manager instead of launching a second server. Then verify:

- `--ollama-host` is reachable from the environment running CommandAgent;
- the value is a base URL without `/api`;
- container-to-host networking uses an address visible from the container;
- firewalls or proxies allow the connection; and
- `/api/tags` lists the exact executor and planner model IDs.

See [Ollama host and models](providers.md#ollama-host-and-models). Raising
`--chat-timeout-secs` does not fix a server that is not listening.
