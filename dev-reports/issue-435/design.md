# Issue #435 design

## Inputs and baseline

Read the dispatched Issue body, AGENTS.md, development guardrails, worker skill,
and the complete read-only `20260907-0052-orchestrate-inputs/reference.md`.
The supplied `issue-435.json` has an empty comments array; it contains no reopen
comment or approved decision. No required predecessors were listed. The clean
branch starts at `5dec1494c05f8ff395a4c00edd0f838ce3f64a84`, already containing
#425 via PR #431 (`eef68969`, merged as `6f4d0cfd`). Its authority registration,
handoff implementation, design and focused tests were inspected before editing.

## Smallest coherent change

- Share the evidence classifier's artifact-only predicate with generated
  contract registration. Exclude `test -f`, `test -e`, and `cat` from admitted
  final-success observations, including the empty-contract failure-handoff
  fallback. Keep build/test commands and the existing exact Next.js script
  assertion exclusion. If a Next.js handoff contains only inspections, retain
  the product-owned build fallback. Step checks continue to execute unchanged.
  Normalize compound commands and validate every segment before filtering, so
  `test -f package.json && npm test` retains its substantive test. Keep the
  shared predicate in a leaf module within the existing evidence growth budget.
- Preserve configured/data contract authority, existing command validation,
  source/evidence rejection and promotion gates. Do not silently exempt weak
  evidence in existing contracts; prevention at registration avoids the new-run
  regression without claiming inspections prove application behavior.
- Prevent typed evidence diagnostics and repair guidance from matching
  configuration or entrypoint heuristics just because they mention a path.
  Route scaffold-only and unobserved mutation failures to implementation,
  while retaining actual package/framework diagnostic classification.
- Add focused registration, acceptance and repair-target tests using faithful
  R0/S3/E3 corpus inputs. Update #425 tests whose synthetic success checks used
  file existence to use substantive commands, preserving their assertions on
  authority, preflight failure, Recovery execution and zero bind failures.

## Verification and limits

Run focused Rust tests, corpus regression and growth guards, then
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo test`. Record exact outcomes in the verification report before committing.
No Python harness edits are planned. No live campaign, historical evidence
rewrites, event schema changes, runtime namespace changes, guardrail baseline
increases, pushes or PRs are in scope. Deterministic fixtures demonstrate the
regression boundary, not full business success for the historical campaign.
