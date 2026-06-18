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
- `agent/events`: shared passive runtime events for UI and tests.
- `tui`: terminal rendering for interactive progress and final-answer
  formatting.

## Boundary Summary

| Layer | Owns | Must Not Own |
| --- | --- | --- |
| Provider | HTTP/API transport, provider-specific payload shapes | Planning, repair policy, profile behavior |
| Minimal loop | Tool-call execution, observations, bounded completion guards | Multi-step plans, domain profiles, unbounded retry |
| Profile | Small domain facts, verifier hints, protected prefixes | Workflow control, hidden task-specific agents |
| Step runner | Plan schema, lint, verifier, repair packet, ultra phase order | Provider transport, low-level tool implementation |
| TUI | TTY-aware rendering of runtime events and final answers | Planning, repair, retry, provider parsing, filesystem policy |
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

## Terminal UI

The TUI is a passive observer. The runtime emits bounded events through
`agent/events`, and `src/tui` renders those events to stderr only when stderr is
a TTY. Non-TTY stdout remains script-friendly and does not receive progress
text.

Terminal progress can show a startup logo, plan generation, saved plan paths,
plan previews, ultra phases, step starts/finishes, tool summaries, verifier
status, dependency setup start/finish, profile verification failures, artifact
status, bounded repair attempts, repair packet paths, and a standalone
suggested next command. These lines are evidence from existing runtime state;
they do not change planning, verification, repair budgets, provider behavior,
or tool policy.

For blocking planner, model, verifier, repair, and tool waits, the TUI can emit
an in-place elapsed spinner until the runtime emits completion, failure, or the
next event. Disabling the spinner affects only the active wait animation, not
the ordinary append-only progress evidence.

Assistant final answers are Markdown-formatted only when stdout is a TTY and
Markdown rendering is enabled. The renderer supports a narrow subset and emits
SGR-only ANSI escapes.

XML fallback parsing remains in provider/minimal-loop code. TUI displays
tool-call mode and parser feedback events but does not parse
`<commandagent_tool_call>` blocks or infer tool behavior from assistant text.

Artifact status uses the runtime's path-confined missing-path helpers. Step
`expected_paths` are step-local gates. Ultra-plan `required_artifacts` are final
user-requested outputs and are reported at the final ultra boundary, not as
phase-local failures.

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

Dependency setup commands are a narrow exception class, not ordinary Bash
capability. `npm install`, `npm ci`, and `pnpm install` classify as `EnvSetup`.
They remain blocked for normal model-issued `Bash` tool calls, even when
`--yes` is set. The step runner may run one `EnvSetup` command only after a
verifier returns `dependency_missing`, expected source paths are present,
setup is approved, and offline mode is disabled.

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

Step execution also carries a step tool policy from the step runner into the
minimal-loop executor. Inspect and report steps are read-only. Verify steps are
no-mutation checks. Setup steps may change setup/config files such as
`package.json`, `tsconfig.json`, `next.config.*`, `tailwind.config.*`, and
`postcss.config.*`, but source route/component edits belong to create, edit, or
repair steps. Repair turns are explicit bounded repair sessions and may mutate
files within the normal file-tool and path-confinement rules.

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
phase-local step-planning prompt with a freshly collected bounded workspace
snapshot, a data-only phase workspace contract, and the selected profile
contract, then delegates to a step-plan executor. The phase contract contains
generic facts such as visible root entries, lockfiles, package scripts, final
required artifacts, and profile-projected summary lines. It does not choose a
framework-specific workflow or mutate files. A phase failure stops the run and
returns a readable phase report instead of continuing with stale context.

Verification is deterministic. It runs only commands accepted by the local Bash
policy, detects dependency-missing cases before fake success is possible, and
compresses failures into bounded diagnostics plus nearby source excerpts when a
file/line reference is present.

When every verifier failure is `dependency_missing` and expected paths are
present, the step runner checks the setup policy before repair. With `--yes`
and online mode, it selects one deterministic setup command from lockfiles,
runs it once with a bounded timeout, writes setup logs under
`.commandagent/setup/`, reruns the original verifier once, and continues only
if the evidence improves. Without approval, in offline mode, or after exhausted
setup recovery, it stops with a clear blocker instead of creating a repair
packet.

Repair is bounded and evidence-driven. The default budget allows two
file-changing attempts. When repair is exhausted, CommandAgent writes a short
replan packet under `.commandagent/repairs` and suggests an explicit
`/ultra-plan-run --profile <profile> "$(cat ...)"` command instead of hiding an
unbounded retry loop. That suggested command starts a standalone repair plan;
it is not reported as completion of the original ultra plan unless the original
plan is explicitly resumed or replanned and finishes.

## Profile Boundary

Profiles are intentionally small. They provide profile text, optional verifier
commands, optional protected path prefixes, read-only fact summaries, and
read-only profile verification. They do not own planning logic, edit files, or
run domain-specific agents. Profile verification can fail a phase with explicit
diagnostics, but it does not auto-repair.

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
