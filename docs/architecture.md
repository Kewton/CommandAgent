# Architecture

CommandAgent is built around a small set of modules with explicit boundaries.
The boundary is more important than the module name: when a feature crosses a
boundary, it should be split before more behavior is added.

## Runtime

- `cli`: parses command-line options and starts one-shot or REPL mode.
- `config`: merges CLI, environment, and `.commandagent/config`.
- `runtime_client`: builds the configured executor and planner provider clients.
- `providers`: hides model transport differences behind a thin chat contract.
- `agent/minimal_loop`: runs one tool-call execution session.
- `agent/repl`: provides interactive use when no prompt is passed.
- `agent/slash_command`: parses interactive planning commands.
- `agent/step_runner`: implements plan and ultra-plan execution.
- `tools`: built-in deterministic capabilities.
- `session`: stores messages, llm-io logs, and resumable state.
- `safety`: path confinement and host validation.
- `util`: shared workspace path and file classification helpers.
- `tui`: terminal rendering.

## Boundary Summary

| Layer | Owns | Must Not Own |
| --- | --- | --- |
| Provider | HTTP/API transport, provider-specific payload shapes | Planning, repair policy, profile behavior |
| Minimal loop | Tool-call execution, observations, bounded completion guards | Multi-step plans, domain profiles, unbounded retry |
| Profile | Small domain facts, verifier hints, protected prefixes | Workflow control, hidden task-specific agents |
| Step runner | Plan schema, lint, verifier, repair packet, ultra phase order | Provider transport, low-level tool implementation |
| Tools | Deterministic workspace actions | Task interpretation or planning |
| Eval | Run roots, summaries, recheck, reports | Runtime behavior changes |

This separation is the main defense against rebuilding the removed legacy stack
under new names.

## REPL

When no prompt argument is supplied and stdin is a TTY, `cli` starts the minimal
REPL. The REPL owns only line input, `/exit` and `/quit`, empty-line skipping,
and per-turn session saving. Actual work is delegated to a `ReplTurnRunner`;
the production runner calls the minimal loop, while tests use a mock runner.

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

The provider layer does not own planning, repair, profiles, or evaluation. A
provider-specific bug fix belongs in the provider module; a behavioral policy
belongs in the minimal loop or step runner only if it is provider-independent.

XML fallback is a shared tool-call format, not provider-specific behavior.
Gemini and OpenAI provider modules may parse XML fallback blocks from provider
response text and return them as `ChatResponse.tool_calls`, while also removing
the XML block from assistant content. The minimal loop still keeps XML
extraction as a safety net so the execution contract remains provider
independent. When XML fallback tool calls are parsed into `tool_calls`, the
minimal loop renders those calls back into canonical XML in assistant history so
API providers can see the prior tool call on the next turn. A single XML block
must not result in duplicate tool execution.

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

Search tools walk the workspace deterministically and skip hidden paths by
default. Search output is bounded so a tool result cannot flood the next model
turn.

## State and Logs

State lives under `.commandagent/`, including plans, repairs, and sessions.
Sessions are stored at `.commandagent/sessions/<id>/session.json`. The MVP
supports save/load and `--resume` plumbing, but does not migrate historical
state directories.

LLM request/response observations are stored as JSON Lines at
`.commandagent/logs/llm-io.jsonl`. The logger records provider, model, planner
metadata, tool-call mode, and payload. Secret-bearing keys such as API keys,
authorization headers, and tokens are redacted before writing.

## Step Runner Boundary

The step runner owns planning, linting, verification, and bounded repair. The
minimal loop owns single-step execution. Profiles add small contracts and
verifiers, not full domain-specific agents.

Step plans use a small CommandAgent-owned YAML schema: goal, profile, style,
intent, required final artifacts, and ordered steps with kind, instruction,
expected result, expected paths, and verifier commands. The YAML reader/writer
intentionally supports only this schema so planning remains a bounded contract
instead of an open-ended document format. Missing fields in older plan files are
defaulted on read and normalized on save.

Plan linting is a separate pass. It rejects obvious schema-contract mistakes:
non-file `expected_paths`, JSON/property selectors, alternative paths, glob
patterns, version strings, path escape, and steps that clearly mix
file-changing setup with final verification. Workspace-aware lint may check
whether named paths already exist, but it is limited to shallow existence checks;
it does not read file contents or force a framework-specific project structure.

Ultra plans are one level higher: goal, profile, style, intent, required final
artifacts, and ordered phases. Each phase is later turned into a step plan.
Ultra planning does not run tools by itself; it only creates bounded phase
contracts under `.commandagent/plans/ultra-plan-*.yaml`.

Ultra execution is phase-oriented. For each phase, CommandAgent builds a
phase-local step-planning prompt with a bounded workspace snapshot and profile
contract, then delegates to a step-plan executor. A phase failure stops the run
and returns a readable phase report instead of continuing with stale context.

Verification is deterministic. It runs only commands accepted by the local Bash
policy, detects dependency-missing cases before fake success is possible, and
compresses failures into bounded diagnostics plus nearby source excerpts when a
file/line reference is present.

Repair is bounded and evidence-driven. The default budget allows two
file-changing attempts. When repair is exhausted, CommandAgent writes a short
replan packet under `.commandagent/repairs` and suggests an explicit
`/ultra-plan-run --profile <profile> "$(cat ...)"` command instead of hiding an
unbounded retry loop.

## Profile Boundary

Profiles are intentionally small. They provide profile text, optional verifier
commands, and optional protected path prefixes. They do not own planning logic
or run domain-specific agents.

The current profile set is MVP-sized: `generic`, `nextjs`, `python`, `rust`,
`investigation`, `docs`, `data-analysis`, and `data-pipeline`. A new profile
must justify why the generic contract plus explicit user instructions are not
enough.

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

## Removed Legacy Surface

CommandAgent has no legacy engine, sidecar route, case memory, anti-pattern
retrieval, Photon/PAM advisory layer, or old repair job system. If one of those
ideas becomes necessary, it must be reintroduced through the admission rule in
`docs/philosophy.md`, with a narrow trigger and an eval plan.
