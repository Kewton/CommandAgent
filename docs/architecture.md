# Architecture

CommandAgent is built around a small set of modules with explicit boundaries.

## Runtime

- `cli`: parses command-line options and starts the selected mode.
- `config`: merges CLI, environment, and `.commandagent/config`.
- `providers`: hides model transport differences behind a thin chat contract.
- `agent/minimal_loop`: runs the tool-call loop.
- `agent/repl`: provides interactive use when no prompt is passed.
- `agent/step_runner`: implements plan and ultra-plan execution.
- `tools`: built-in deterministic capabilities.
- `session`: stores messages, llm-io logs, and resumable state.
- `safety`: path confinement and host validation.
- `util`: shared workspace path and file classification helpers.
- `tui`: terminal rendering.

## Provider Boundary

Provider abstraction is intentionally thin. Providers send chat turns and return
assistant content plus optional native tool calls. Ollama may use native tools.
Gemini and OpenAI use XML fallback tool calls unless a provider-specific native
tool surface is added deliberately.

Planner and executor can use different providers and models.

Current provider capability contract:

- `ollama`: native tool calls by default
- `gemini`: XML fallback tool calls by default
- `openai`: XML fallback tool calls by default

The provider layer does not own planning, repair, or profile behavior.

## Tool Contract

File creation is done with `Write`; parent directories are created
automatically. `Bash` is for local inspection, tests, and build verification,
not for creating directories before `Write`.

`Bash` must keep offline policy consistent with the prompt: local read-only,
script-run, and build-test commands are allowed when they remain inside the
workspace; dangerous or network actions are blocked.

Directory creation through `Bash` is blocked with guidance to use `Write`
instead. The `Write` tool creates parent directories automatically, so `mkdir`
is not part of the normal file creation path.

The only compound command form intentionally recognized by `Bash` policy is:

```text
cd <workspace path> && <local read/script/build command>
```

The tail command is reclassified on its own. Extra chaining or dangerous tails
remain blocked.

All file tools and session writes must go through path confinement. Relative
paths are resolved under the workspace root. Parent traversal and symlink escape
are rejected before a tool reads or writes data.

State lives under `.commandagent/`, including plans, repairs, and sessions.
Sessions are stored at `.commandagent/sessions/<id>/session.json`. The MVP
supports save/load and `--resume` plumbing, but does not migrate historical
state directories.

LLM request/response observations are stored as JSON Lines at
`.commandagent/logs/llm-io.jsonl`. The logger records provider, model, planner
metadata, tool-call mode, and payload. Secret-bearing keys such as API keys,
authorization headers, and tokens are redacted before writing.

Search tools walk the workspace deterministically and skip hidden paths by
default. Search output is bounded so a tool result cannot flood the next model
turn.

## Step Runner Boundary

The step runner owns planning, linting, verification, and bounded repair. The
minimal loop owns single-turn execution. Profiles add small contracts and
verifiers, not full domain-specific agents.

## Minimal Loop

The minimal loop owns one coding-agent session:

- build the system/user/tool context
- call the active chat provider
- execute tool calls
- append tool observations
- finish only when the assistant returns a completed final answer

Provider transport is injected through a small `ChatClient` trait. This keeps
Ollama, Gemini, and OpenAI transport details outside the loop. Native tool calls
and XML fallback are both represented through the shared `ToolCall` type.

Malformed tool-call parsing in native mode downgrades the session to XML
fallback mode. The parser feedback shows the XML format example only after this
mode transition.
