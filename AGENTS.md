# AGENTS.md

## Repo Intent

CommandAgent is a minimal local-first coding agent built around one execution
loop, explicit planning boundaries, deterministic tools, and bounded repair.
It is not a compatibility port of the historical legacy engine.

The repository tests a narrow product bet: useful coding tasks can be handled
with a small minimal loop plus visible step contracts, without reintroducing a
large hidden control stack.

Historical legacy surfaces are intentionally out of scope:

- legacy engine selection
- sidecar routing in the MVP
- case memory or anti-pattern corpora
- advisory layers such as Photon/PAM
- unbounded or hidden repair jobs
- provider/model-specific behavioral policy

## Core Concept

CommandAgent has one execution engine: the minimal loop.

`/plan-run` and `/ultra-plan-run` are not separate engines. They are outer
planning and execution helpers that decompose work into explicit step or phase
contracts, then delegate each executable step back to the same minimal loop.

The expected direction is:

```text
small task       -> direct prompt or /plan-run
large task       -> /ultra-plan-run
failed repair    -> bounded repair packet, then explicit user-visible replan
```

Do not hide large autonomous behavior behind the runtime.

## Design Principles

Follow these principles when changing the repository:

- Prefer deterministic evidence over semantic guessing.
- Remove ambiguity before adding a new mechanism.
- Keep runtime guards narrow, bounded, and observable.
- Preserve predictable behavior even when a benchmark result is tempting.
- Keep planning, execution, verification, and repair as separate contracts.
- Prefer smaller phase/step contracts over larger repair mechanisms.
- Treat eval scripts and docs as product code.
- Prefer explicit failure reports over hidden continuation.
- Prefer common contracts before profile-specific fixes.

Avoid changes that make behavior harder to attribute:

- retrying until success
- adding hidden repair loops
- broadening no-tool guards into ordinary conversation/report tasks
- rewriting verifier commands or build scripts to fake success
- running network/dependency setup implicitly in normal eval
- putting behavioral policy into provider transports
- turning profiles into workflow engines
- adding provider/model-specific prompt branches for shared behavior

## Architecture Map

Primary modules:

- `src/cli.rs`: command-line parsing and one-shot/REPL entry.
- `src/config.rs`: CLI, environment, and `.commandagent/config` merge.
- `src/providers/*`: thin chat transport for Ollama, Gemini, and OpenAI.
- `src/providers/xml_fallback.rs`: XML tool-call extraction shared by API providers.
- `src/agent/minimal_loop/*`: one execution session, tool calls, observations,
  completion guards, and final-answer validation.
- `src/agent/repl.rs`: interactive line loop and per-turn session saving.
- `src/agent/slash_command.rs`: `/plan-run`, `/ultra-plan-run`, and related
  command parsing.
- `src/agent/step_runner/*`: plan schema, ultra plans, lint, profiles,
  deterministic verification, bounded repair, and repair packets.
- `src/tools/*`: deterministic built-in tools.
- `src/session/*`: sessions and compaction.
- `src/safety/*`: path confinement and host validation.
- `scripts/*` and `eval/*`: evaluation harnesses and benchmark cases.

Boundary rules:

- Provider owns transport only. It must not own planning, repair, profiles, or
  behavioral policy.
- Minimal loop owns one tool-call execution session. It must not become a
  planner or multi-step workflow engine.
- Step runner owns plan schema, lint, deterministic verification, bounded
  repair, and ultra phase ordering. It must not own provider transport or tool
  internals.
- Profile owns small domain facts and verifier hints. It must not become a
  hidden domain-specific agent.
- Tools own deterministic workspace actions. They must not interpret tasks or
  plan next steps.
- Eval owns evidence collection and recheck/reporting. It must not tune runtime
  behavior during a run.

## Tool And Safety Contract

- File creation is done with `Write`; it creates parent directories
  automatically.
- Do not make `Bash` look like the normal file creation path.
- `Bash` is for local inspection, local script runs, and build/test checks.
- Offline policy must stay consistent with the tool catalog and prompts.
- Dangerous, network, mutating shell setup, and unsupported compound commands
  should stay blocked unless a narrow policy change is justified by triage.
- All file access must respect path confinement. Parent traversal and symlink
  escape must be rejected before reading or writing.

## Mechanism Admission

Add a new guard, repair behavior, profile contract, or lint rule only from an
observed failure with evidence.

A candidate mechanism should have:

- deterministic trigger: based on observable state such as a missing expected
  path, verifier result, or specific tool error
- bounded effect: hard cap; no hidden retry loop
- stable scope: one clear layer such as minimal loop, repair context, profile,
  verifier, eval, or provider transport
- observable outcome: logs, repair packets, eval reports, or explicit errors
  show why it fired
- provider-independent behavior unless isolated inside provider transport

If a mechanism starts to look like a removed legacy subsystem, stop and write a
design note before implementing it.

## Development Rules

For branch strategy, branch dependencies, commit policy, PR expectations, and
test strategy, follow `docs/development.md`. This file defines architectural and
behavioral boundaries for agents; detailed development workflow belongs in the
development document.

When working on code:

- Read the relevant docs before changing behavior.
- Keep changes scoped to the responsible layer.
- Do not reintroduce a legacy engine switch.
- Do not add sidecar routing unless a measured failure justifies it and a new
  design decision is recorded.
- Do not add unbounded retry, hidden repair loops, or success-until-passing
  behavior.
- Do not special-case one provider/model for shared runtime behavior.
- Do not fix benchmark failures by weakening honest verifier behavior.
- If a change affects runtime behavior, update docs and add or update tests.
- If a change affects eval interpretation, update eval docs/reports.

Preferred implementation shape:

```text
triage evidence -> narrow contract change -> unit test -> focused eval -> docs
```

## Testing And Evaluation

Development workflow and test selection are described in `docs/development.md`.
Evaluation mechanics, failure categories, and reporting expectations are
described in `docs/evaluation.md`.

Minimum local checks for code changes:

```bash
cargo fmt --check
cargo test
cargo build --release
```

Use focused eval for behavioral changes before broad eval:

- smoke and dry-run scripts for wiring
- single-case focused runs for a targeted failure class
- `runs=1` for MVP sign-off smoke
- `runs=3` or more only when judging stability

Evaluation results should record:

- commit hash
- dirty flag
- binary path
- provider/model
- eval root
- success/failure class
- what changed compared with the prior root

Keep raw run roots as evidence, but summarize decisions under `docs/eval/`.

## Documentation Expectations

Update docs when changing design, behavior, profile contracts, verifier policy,
repair behavior, provider behavior, or evaluation interpretation.

Key docs:

- `docs/philosophy.md`
- `docs/architecture.md`
- `docs/development.md`
- `docs/ultra-plan-run.md`
- `docs/evaluation.md`
- `docs/known-limitations.md`
- `docs/providers.md`
- `docs/profiles.md`
- `docs/adr/0001-minimal-only.md`
- `docs/adr/0002-contract-recovery.md`

Prefer adding a short eval/triage report over burying conclusions in a commit
message.

## Current Known Limits

CommandAgent is an MVP.

- Complex `/ultra-plan-run` workflows depend on model quality and local
  toolchains.
- Large Rust modify tasks can still fail from implementation-quality or phase
  decomposition issues after required artifacts are created.
- Next.js build-quality eval needs dependency setup/preinstall policy to be
  explicit; `dependency_missing` is not the same as implementation failure.
- Smaller planner models may fail strict YAML planning even after bounded
  correction.
- Repair is intentionally bounded; failure packets are expected behavior, not
  necessarily bugs.

If improving these limits, keep the fix in the smallest responsible layer and
avoid rebuilding removed legacy orchestration.
