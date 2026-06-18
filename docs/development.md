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

## Commit Policy

- Keep commits small enough to explain in one sentence.
- Separate code changes from eval-report-only commits when practical.
- Commit focused behavior changes before running eval so `meta.json` records a
  clean commit and `dirty: false`.
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
| eval scripts | dry-run or recheck smoke plus script syntax check |

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
- headline result
- failure category
- interpretation and follow-up decision

Dirty evals may be useful for observation, but do not use them as adoption
evidence.

## Dependency And Environment Policy

- Local eval should not silently install dependencies unless the case/step
  explicitly makes dependency setup part of the task.
- Do not change build scripts to fake verifier success.
- Treat `dependency_missing` as an environment/setup boundary, not as a generic
  implementation failure.
- Provider API keys must come from the environment or the caller's env loader.
  CommandAgent does not load `.env` internally.

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
