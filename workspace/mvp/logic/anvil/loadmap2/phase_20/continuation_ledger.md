# Phase 20 Continuation Ledger

Date: 2026-06-23 JST

## Purpose

This ledger records why Phase20 cannot declare migration completion. The rows
below are grouped continuation blockers derived from
`workspace/mvp/logic/anvil/loadmap2/phase_20/coverage_closure.md`.

The ledger is intentionally not a new implementation roadmap. A future phase
must split any group into narrower implementation tasks with proof commands
before changing status.

## Blocking Groups

| id | coverage rows | owner layer | blocker | required action | proof gate | status |
| --- | --- | --- | --- | --- | --- | --- |
| P20-COV-001 | C01-C12 | planning, task contract, artifact graph, ownership, recovery orchestration | Core contract and ownership responsibilities remain `Partial`. | Define row-specific parity criteria and prove each with focused fixtures plus broad sign-off. | Updated coverage rows with proof report and sign-off pass. | open |
| P20-COV-002 | C13-C20 | recovery task, setup, profile, semantic repair, action envelope | Recovery task and repair-action responsibilities remain `Partial`. | Split by recovery owner/action family and prove safe-stop, allowed action, and target behavior. | Focused recovery matrix plus broad sign-off. | open |
| P20-COV-003 | C21-C32 | target admission, repair state, verifier, completion evidence, patch validation | Target/repair/verifier responsibilities remain `Partial`. | Add proof for target ranking, no-progress, verifier orchestration, patch admission, and rollback/exclusion semantics. | Focused fixtures for each branch plus broad sign-off. | open |
| P20-COV-004 | C33 | contract conflict | Contract conflict job is `Missing` while adoption is `Adopt`. | Implement conflict object, source-of-truth decision, spec authority, and ambiguous-authority safe stop. | Unit tests, focused conflict fixture, broad sign-off. | open |
| P20-COV-005 | C34-C44 | language adapters, tool policy, workspace walk, job events, scaffold/data/docs, lifecycle, provider plumbing | Cross-profile and runtime-support responsibilities remain `Partial`. | Define accepted MVP parity for each row or explicitly exclude with design rationale. | Coverage update with proof source plus broad sign-off. | open |
| P20-COV-006 | C49-C50 | quality gates, slash/plan UI | Rows are not in the accepted migration surface yet but remain unresolved priority decisions. | Decide whether to adopt, partially adopt, or explicitly exclude. | Coverage table decision update. | open |
| P20-LEDGER-001 | P17-L001 | provider transport / eval boundary | Large timeout rows are owned and evidence-bound but remain `blocked_external`. | Either run a non-timeboxed proof to convert to `closed_proven`, or keep as explicit limitation in a complete-with-exclusions decision. | Non-timeboxed successful proof or final exclusion rationale. | open |

## Next Phase Admission Rule

A future phase should not start from this grouped ledger directly. It should
first select one group, split it into row-level tasks, assign an owner layer,
and define a deterministic proof command for each selected row.
