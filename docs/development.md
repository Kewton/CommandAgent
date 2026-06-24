# Development

This document defines day-to-day development workflow for CommandAgent.
Architectural boundaries live in `AGENTS.md`, `docs/philosophy.md`, and
`docs/architecture.md`. Evaluation details live in `docs/evaluation.md`.

## Branch Strategy

- Keep `main` releasable.
- Do feature work on short-lived branches.
- Prefer branch names that describe the change type:
  - `feat/<topic>` for user-facing capability
  - `fix/<topic>` for bugs
  - `docs/<topic>` for documentation
  - `eval/<topic>` for evaluation scripts, cases, and reports
  - `refactor/<topic>` for internal structure-preserving changes
- Keep each branch scoped to one behavioral idea or one documentation/eval
  artifact set.
- Do not mix unrelated runtime, profile, provider, and eval changes in one
  branch unless the dependency is explicit and documented.

## Worktree And CommandMate Operations

- The existing `ci/phase1-default-ci` checkout may remain in place for the CI
  setup work already in progress.
- For any later work on a different branch, create or use a separate
  `git worktree`; do not repurpose the current checkout by switching branches.
- When Codex operates against another branch or worktree, coordinate through the
  CommandMate CLI available as `commandmatedev` in this environment. Treat it as
  the commandmatecli entrypoint referenced in task notes.
- Check usage with `commandmatedev --help`. If worktree coordination is needed
  and the server is stopped, start it with `commandmatedev start --daemon`.
- Use `commandmatedev ls`, `commandmatedev send`, `commandmatedev wait`,
  `commandmatedev capture`, and `commandmatedev respond` for registered
  worktree agent operations instead of silently manipulating another branch
  from the current checkout.

## Repo-Local Codex Harness

Repository-local Codex skills live under `.codex/skills`. They guide operator
workflows such as issue work, PR creation, UAT, release checks, worktree
cleanup, and dry-run issue orchestration. They do not change CommandAgent
runtime behavior.

Long reusable prompt bodies live under `.codex/prompts` and are loaded only when
a skill needs them. Do not duplicate the same command body across many skills.

Migrate historical source-command workflows as Codex skills, not as
CommandAgent REPL slash commands. The runtime slash commands remain the
commands documented in `docs/usage.md`.

Generated harness artifacts should go under ignored workspace state, normally
`workspace/management/runs/<run_id>/`, unless a summary is intentionally
promoted into docs or eval evidence.

When editing harness files:

- keep `SKILL.md` frontmatter to `name` and `description`
- check for stale source-repository references before finishing
- keep mutating worktree, CommandMate, PR, merge, and UAT operations explicit
  and off by default
- run script compile and fixture/dry-run checks for Python harness changes

## Branch Dependencies

Stack dependent branches from most deterministic to most behavioral:

1. schema/parser/data model change
2. runtime/tool/provider implementation
3. tests
4. focused eval and triage report
5. docs update

When one branch depends on another, state the dependency in the PR body or task
notes. Do not evaluate a behavior branch on a binary that does not contain its
dependencies.

For expensive evals, merge deterministic prerequisites first, then run the
combined eval once. Avoid running a full matrix after each tiny prerequisite
when the result would immediately become stale.
When large failures may come from planner quality, run the Gold Plan slice
(`scripts/eval_large_gold_tasks.sh`) before broad matrix comparisons. Gold Plan
cases use checked-in `/run-plan` inputs, so they isolate the existing minimal
loop worker, tool policy, verifier, and bounded repair behavior from plan
generation.

## Commit Policy

- Keep commits small enough to explain in one sentence.
- Separate code changes from eval-report-only commits when practical.
- Commit focused behavior changes before running eval so `meta.json` records a
  clean commit and `dirty: false`.
- When a change affects an adopted control-recovery path, add or update the
  focused `matrix_row` and its `expected_*` assertions. A case name alone is
  not evidence that the control path is covered.
- Do not commit raw eval workspaces by default. Commit summaries and triage
  reports under `docs/eval/` instead.
- Do not include API keys, `.env`, local model caches, or generated dependency
  directories.

Acceptable commit shapes:

```text
Clarify missing expected path step contract
Record rust missing path contract eval
Tighten provider error mapping
Document development workflow
```

## PR Policy

Every PR should make its intent and evidence clear:

- what changed
- why the change is aligned with CommandAgent's minimal design
- what tests ran
- whether focused eval ran
- what docs changed
- whether follow-up work remains

Behavioral PRs should include a focused eval result or explain why eval was not
run. Eval-only PRs should identify the exact commit and binary used.

## Test Strategy

Minimum checks for code changes:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Run a release build when the change affects CLI wiring, providers, tools,
runtime behavior, or eval execution:

```bash
cargo build --release
```

Use targeted tests where possible:

```bash
cargo test step_runner::repair
cargo test providers::openai
cargo test --test runtime_flow
```

Integration tests under `tests/` cover public runtime flows such as the minimal
loop, slash-command execution, and provider-free CLI smoke. Keep them offline
and deterministic; use mock chat clients and temp workspaces rather than live
providers.

Testing expectations by change type:

| Change type | Expected checks |
| --- | --- |
| docs only | no Rust test required unless examples changed |
| parser/schema | unit tests plus `cargo test` |
| provider transport | provider unit tests plus `cargo test` |
| tool behavior | tool unit tests plus relevant safety/path tests |
| minimal loop guard | unit tests plus focused eval when behavior changes |
| step runner / repair | unit tests plus focused eval for the target failure |
| step tool policy | executor/runtime unit tests plus a focused slash-runtime case |
| profile verification | profile unit tests plus a focused phase-boundary case |
| eval scripts | dry-run or recheck smoke plus script syntax check |
| Codex harness skills/prompts | stale reference check plus manual frontmatter review |
| Codex orchestration script | `python3 -m py_compile`, unit tests or fixture dry-run |

## Evaluation Strategy

Evaluation is evidence, not a tuning loop. See `docs/evaluation.md` for command
details and failure categories.

Use this escalation path:

1. `scripts/eval_smoke.sh` for basic wiring.
2. `scripts/eval_large_tasks.sh --dry-run` for case wiring.
3. focused one-case or small-slice eval for a targeted behavior change.
4. `runs=1` large eval for MVP sign-off smoke.
5. `runs=3` or higher only when judging stability.

Record eval results with:

- commit hash
- dirty flag
- binary path
- provider/model
- eval root
- event JSONL path when `COMMANDAGENT_EVENT_JSONL` is used
- headline result
- failure category
- interpretation and follow-up decision

Dirty evals may be useful for observation, but do not use them as adoption
evidence.

## Dependency And Environment Policy

- Local eval should not silently install dependencies by default.
- Approved online runs may perform one runtime-owned dependency setup recovery
  when verifier evidence is exactly `dependency_missing`.
- `--yes` or `COMMANDAGENT_YES=true` is approval for that one setup attempt,
  including package lifecycle scripts in the current workspace.
- `--offline` is a hard block for dependency setup, even with `--yes`.
- Normal model-issued `Bash(npm install)`, `Bash(npm ci)`, or
  `Bash(pnpm install)` remains blocked; dependency setup is triggered by the
  step runner after verifier evidence, not by planner/model choice.
- In create/edit/repair worker turns, model-issued build/test or script-run
  `Bash` is not executed by the worker. It may become a verifier transition
  only when the command exactly matches the step verifier and the step's
  expected paths already exist; otherwise it stays a structured blocker or
  repair input. Compound read checks such as `test -f ... && ...` are not
  verifier transitions.
- Do not commit generated dependency directories or scratch lockfiles unless
  the task explicitly asks for them.
- Do not change build scripts to fake verifier success.
- Treat `dependency_missing` as an environment/setup boundary, not as a generic
  implementation failure.
- Provider API keys must come from the environment or the caller's env loader.
  CommandAgent does not load `.env` internally.
- Step tool policy is part of the execution contract. Inspect/report steps are
  read-only, verify steps are no-mutation, setup steps are setup/config-only,
  and repair turns are explicit bounded repair sessions.
- Profile verification must stay read-only and deterministic. If it fails, it
  should produce visible diagnostics and stop the phase rather than editing
  files or silently continuing.

## Documentation Policy

Update docs when changing:

- architecture or design philosophy
- provider behavior
- tool/safety policy
- profile contracts
- plan schema, lint, verification, or repair behavior
- eval semantics or case definitions
- known limitations or release readiness

Prefer one of these locations:

- `AGENTS.md` for agent-facing rules and architectural boundaries
- `docs/development.md` for branch, PR, test, and workflow rules
- `docs/evaluation.md` for eval mechanics and interpretation
- `docs/eval/triage/*.md` for run-specific evidence and decisions
- `docs/adr/*.md` for accepted design decisions

## Development Checklist

Before finishing a task, check:

- The change is in the smallest responsible layer.
- No removed legacy subsystem was reintroduced under a new name.
- Provider behavior remains transport-only unless the change is provider-specific
  transport mapping.
- Profiles remain small contracts, not hidden workflows.
- Repair and guards remain bounded and observable.
- Tests appropriate to the change ran.
- Eval ran when behavior changed, or the reason it did not run is recorded.
- Docs were updated when users or future agents need the decision.

## Event Protocol And Budget Changes

Changes to versioned Job/Event protocol, evidence envelopes, usage records, or
budget behavior require compatibility tests. At minimum, verify:

- schema version is present on persisted/external records;
- unknown event or evidence variants are ignored or reported as unsupported
  without panicking;
- replay projection derives the expected job state from ordered events;
- budget exceeded behavior is explicit and finite;
- provider token usage is carried from `ChatResponse` into runtime events when
  present;
- usage unavailable is recorded as unavailable instead of treated as failure
  when provider metadata is absent.

One-shot runs can record external events with:

```bash
COMMANDAGENT_EVENT_JSONL=/tmp/commandagent-events.jsonl commandagent "..."
```

Use this for focused eval evidence when the change affects event, usage,
budget, or CommandMate-facing behavior.
