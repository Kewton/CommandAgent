# Loadmap2 Phase 5: Active Job Dispatch

Date: 2026-06-22

## Scope

This slice implements the Phase 5 recovery-dispatch boundary from
`workspace/mvp/logic/anvil/loadmap2/phase_5`.

The goal is to make active-job arbitration the common gate between deterministic
failure evidence and recovery prompt construction.

## Implemented

- `ActiveJobCandidate` now carries owner, job, action, source layer, source of
  truth, target hint, artifact role, rerun authority, tool-policy projection,
  loop-control action, and deterministic reason.
- The dispatch gate sorts candidates by deterministic priority, source of
  truth, and source layer before choosing a top candidate.
- Compatible same-owner/same-action candidates may merge deterministic rerun
  authority instead of becoming false conflicts.
- Competing owners or incompatible actions at the same top rank stop with
  `contract_conflict`/`ambiguous_tie`.
- Profile recovery policy can adapt its failure-specific decision into a
  canonical active-job candidate, but final owner/action selection remains in
  recovery orchestration.
- Candidate lines now expose source layer, source of truth, selected tool
  policy, loop-control action, target hint, and artifact role for eval/debugging.

## Design Checks

- No retry budgets were increased.
- No hidden repair loop was added.
- Provider/model behavior was not special-cased.
- Profiles remain fact and candidate producers; they do not become workflow
  engines.
- The minimal loop remains the executor for one selected bounded task.

## Verification Plan

Required local checks:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

Focused eval should confirm:

- setup and manifest failures select one setup/manifest owner;
- route/source/test/docs/tool-protocol/evidence-binding failures expose one
  selected owner/action;
- compatible same-owner candidates merge;
- competing owners stop with `contract_conflict`;
- no-owner cases stop explicitly.

## Local Verification

Executed on 2026-06-22:

- `cargo fmt --check`: passed
- `cargo test`: passed, 636 unit tests plus integration/doc-test suites
- `cargo clippy --all-targets -- -D warnings`: passed
- `cargo build --release`: passed
- `python3 tests/test_eval_report.py`: passed, 19 tests
- `scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery --out eval/runs/loadmap2-phase5-dry-run --binary target/release/commandagent --dry-run`: passed

Dry-run focused eval root:

- `eval/runs/loadmap2-phase5-dry-run/20260622T110245`

The dry-run confirms case discovery, expected assertion parsing, and report
field plumbing. It does not execute a live local-LLM repair turn.
