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

## REPL

When no prompt argument is supplied and stdin is a TTY, `cli` starts the minimal
REPL. The REPL owns only line input, `/exit` and `/quit`, empty-line skipping,
and per-turn session saving. Actual work is delegated to a `ReplTurnRunner`;
the production runner calls the minimal loop, while tests use a mock runner.

This keeps interactive UX separate from provider transport and from future
slash-command planning features.

Slash command parsing is a separate module. It recognizes plan/ultra-plan
commands, `--profile`, `--style`, and bounded `$(cat ...)` repair prompt
references. File references are resolved through path confinement before their
contents are expanded.

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

Step plans use a small CommandAgent-owned YAML schema: goal, profile, style, and
ordered steps with instruction, expected paths, and verifier commands. The YAML
reader/writer intentionally supports only this schema so planning remains a
bounded contract instead of an open-ended document format.

Ultra plans are one level higher: goal, profile, style, intent, and ordered
phases. Each phase is later turned into a step plan. Ultra planning does not run
tools by itself; it only creates bounded phase contracts under
`.commandagent/plans/ultra-plan-*.yaml`.

Ultra execution is phase-oriented. For each phase, CommandAgent builds a
phase-local step-planning prompt with a bounded workspace snapshot and profile
contract, then delegates to a step-plan executor. A phase failure stops the run
and returns a readable phase report instead of continuing with stale context.

Profiles are intentionally small. They provide profile text, optional verifier
commands, and optional protected path prefixes. They do not own planning logic
or run domain-specific agents.

Plan linting is a separate pass. It rejects obvious schema-contract mistakes:
non-file `expected_paths`, JSON/property selectors, version strings, path
escape, and steps that clearly mix file-changing setup with final verification.
It does not force a framework-specific project structure.

Verification is deterministic. It runs only commands accepted by the local Bash
policy, detects dependency-missing cases before fake success is possible, and
compresses failures into bounded diagnostics plus nearby source excerpts when a
file/line reference is present.

Repair is bounded and evidence-driven. The default budget allows two
file-changing attempts. When repair is exhausted, CommandAgent writes a short
replan packet under `.commandagent/repairs` and suggests an explicit
`/ultra-plan-run --profile <profile> "$(cat ...)"` command instead of hiding an
unbounded retry loop.

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

The loop has three narrow completion guards:

- future-action feedback: a no-tool response that says it will create, edit,
  read, run, or verify something is not accepted as a final answer on the first
  occurrence
- completion-without-write feedback: a no-tool completion before any Write/Edit
  receives one neutral reminder that file-changing tasks require tools
- requested-artifact feedback: configured expected paths are checked before
  completion, and missing paths receive one direct reminder

These guards do not inspect task semantics. They only react to observable
session facts and are capped to avoid unbounded repair behavior.
