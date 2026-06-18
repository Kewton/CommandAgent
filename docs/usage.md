# Usage

CommandAgent is a minimal coding-agent CLI. It has one execution engine: the
minimal loop. Large task helpers build explicit plans around that loop instead
of switching engines.

## Basic Commands

```bash
commandagent --help
commandagent --version
commandagent --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4
commandagent --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 "Create README.md"
```

When started from a terminal without a prompt argument, CommandAgent opens the
minimal interactive REPL:

```text
commandagent>
```

Blank lines are ignored. `/exit` and `/quit` end the REPL. Each successful turn
runs the minimal loop and saves a session under `.commandagent/sessions`.

The interactive prompt is an application prompt, not a shell command. Slash
commands must be typed after `commandagent>`:

```text
commandagent> /ultra-plan-run --profile nextjs Create a Next.js app on port 3011
```

## Terminal Output

When stderr is a TTY, CommandAgent prints progress lines for long-running work:
planner generation, saved plan paths, compact plan previews, ultra phases,
step starts/finishes, tool summaries, verifier status, artifact status, bounded
repair attempts, repair packet paths, and a standalone `next command:` block
when a repair packet is saved.

While a blocking planner, model, verifier, repair, or tool call is still
running, CommandAgent emits bounded elapsed wait lines such as:

```text
waiting: ultra plan generating profile=nextjs 2s
waiting: model iter 1: gemini:gemini-3.1-flash-lite (native) 4s
```

Progress is presentation-only. It is emitted to stderr and does not change
planning, verification, repair budgets, provider behavior, or tool policy.
Non-TTY stdout remains plain for scripts.

When stdout is a TTY, final answers may be rendered with a narrow Markdown
subset. Disable terminal decorations with:

```bash
NO_COLOR=1
COMMANDAGENT_NO_SPINNER=1
COMMANDAGENT_NO_MARKDOWN=1
COMMANDAGENT_NO_EMOJI=1
```

`COMMANDAGENT_NO_SPINNER=1` also suppresses transient progress rendering.
TUI output does not parse XML tool-call blocks; XML fallback parsing remains in
provider/minimal-loop code.

## Configuration

CommandAgent merges configuration in this order:

1. built-in defaults
2. `.commandagent/config`
3. environment variables
4. CLI flags

Supported bootstrap keys include:

- `provider`
- `planner_provider`
- `model`
- `planner_model`
- `context_budget`
- `max_iterations`
- `timeout_secs`
- `retries`
- `yes`
- `offline`
- `resume`
- `state_dir`

The formal environment prefix is `COMMANDAGENT_*`, for example:

```bash
COMMANDAGENT_PROVIDER=ollama
COMMANDAGENT_MODEL=qwen3.6:35b-a3b-coding-nvfp4
COMMANDAGENT_PLANNER_PROVIDER=gemini
COMMANDAGENT_PLANNER_MODEL=gemini-3.5-flash
```

API keys use provider-standard names:

```bash
GEMINI_API_KEY=...
OPENAI_API_KEY=...
```

`.env` files are not loaded by CommandAgent itself in the MVP. Export variables
in the shell or use an external env loader.

## Providers

Provider examples:

```bash
commandagent --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4
commandagent --provider gemini --model gemini-3.1-flash-lite
commandagent --provider openai --model gpt-5.4-mini
```

Planner and executor models can differ:

```bash
commandagent \
  --provider ollama \
  --model qwen3.6:35b-a3b-coding-nvfp4 \
  --planner-provider gemini \
  --planner-model gemini-3.5-flash
```

`--provider` selects the execution model. `--planner-provider` selects the
model used for plan generation. If planner options are omitted, planning uses
the executor provider/model.

CommandAgent has only one execution engine: the minimal loop. `--engine` is not
a supported option. To start the interactive REPL, run `commandagent` from a
terminal without a prompt argument.

Live Gemini/OpenAI checks that use API keys are manual opt-in checks. They are
not part of `cargo test`, default CI, or default eval/smoke scripts.

## Slash Commands

Interactive slash commands use the same parser as the step runner. The parser
recognizes:

- `/plan-steps`
- `/plan-run`
- `/run-plan`
- `/ultra-plan`
- `/ultra-plan-run`
- `/run-ultra-plan`

The distinction:

- `/plan-steps`: generate and save a step plan.
- `/plan-run`: generate a step plan and run it.
- `/run-plan`: run an existing step plan file.
- `/ultra-plan`: generate and save a phase plan for a larger task.
- `/ultra-plan-run`: generate phases, then run each phase through step plans.
- `/run-ultra-plan`: run an existing ultra plan file.

The parser also accepts leading `--profile` and `--style` options for plan
commands, plus bounded file references such as:

```text
/ultra-plan-run --profile nextjs "$(cat .commandagent/repairs/repair.md)"
```

File references are resolved inside the current workspace and cannot escape it.
The REPL itself handles `/exit` and `/quit`.

## Profile vs Style

`--profile` chooses the domain contract. It changes facts and checks that should
be true for the task, such as `nextjs`, `python`, `rust`, `docs`,
`investigation`, `data-analysis`, or `data-pipeline`.

`--style` changes how the work should be approached inside the same domain.
Current styles are intentionally small:

- `default`: implement with practical checks.
- `tdd`: prefer tests or failing checks before implementation when reasonable.
- `test-hardening`: focus on improving verification and regression coverage.

Use `--profile` for the kind of project. Use `--style` for the development
method.

## Intent and Artifact Contracts

`/plan-run` and `/ultra-plan-run` accept optional contract flags:

```text
/ultra-plan-run --profile nextjs --intent modify --artifact app/page.tsx "Update the dashboard"
```

`--intent` tells the planner what kind of work is being requested. Current
values are `new`, `modify`, `investigate`, `document`, `data`, and `unknown`.
When omitted, CommandAgent uses a small deterministic detector and falls back to
`unknown` when the goal is ambiguous.

`--artifact` declares a final user-requested output path. It can be repeated.
Artifacts are not hidden benchmark hints; they are part of the task contract and
are preserved in saved plan files. If no artifact is specified, normal generic
behavior is unchanged.

Artifact status in terminal progress distinguishes two scopes:

- step `expected_paths`: step-local outputs that the minimal loop may enforce
  before accepting a step completion
- ultra `required_artifacts`: final user-requested outputs that are checked at
  the final ultra-plan boundary

## Repair Suggested Command

When bounded repair fails, CommandAgent saves a short replan packet:

```text
.commandagent/repairs/repair-verify-build-1234567890.md
```

It also prints a suggested command:

```text
/ultra-plan-run --profile nextjs "$(cat .commandagent/repairs/repair-verify-build-1234567890.md)"
```

Run that command from the interactive `commandagent>` prompt. The saved repair
packet is intentionally shorter than the full failure log so it can be reused as
a new explicit task without hitting slash-command length limits.

## Plan File Shape

Step plans are stored under `.commandagent/plans` as CommandAgent-owned YAML.
The current schema is:

```yaml
goal: "..."
profile: "generic"
style: "default"
intent: "unknown"
required_artifacts:
  - "relative/final-output.md"
steps:
  - id: "short-slug"
    kind: "create"
    instruction: "one concrete action"
    expected_result: "pass"
    expected_paths:
      - "relative/file/path"
    verify:
      - "local verification command"
```

Ultra plans are saved in the same directory as `ultra-plan-*.yaml` and contain
phase goals. A later execution step turns each phase into a step plan.
