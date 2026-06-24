# Phase28 Source Alignment Matrix

Date: 2026-06-23 JST

| coverage id | Anvil source files | adopted behavior | intentionally omitted behavior | CommandAgent target modules | proof method |
| --- | --- | --- | --- | --- | --- |
| C33 | `contract_conflict_job.rs`, `spec_authority.rs`, `api_contract_expectation.rs` | Structured implementation-vs-test-vs-docs/API conflict decision; deterministic source-of-truth decision; separate authority side and repair target/action; ambiguous-authority safe stop. | Hidden confirmation, advisory memory, semantic sidecar judgment, unbounded continuation, provider/model-specific policy, profile-owned workflow. | New `contract_conflict.rs`; `recovery_orchestration.rs`; eval schema/report files; focused fixtures. Existing recovery task/evidence rendering consumes the added fields without a new executor. | `cargo test contract_conflict`; recovery orchestration tests; focused C33 root `eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521`; broad sign-off. |

## Adopted Contract Details

| source concept | CommandAgent form |
| --- | --- |
| contract conflict job | C33 contract layer that consumes existing deterministic evidence and returns authority/action/safe-stop data. |
| spec authority | Bounded authority inputs from user request, behavior obligation, profile contract, existing docs/API/schema, pre-existing tests, generated-test binding, verifier command, and preservation constraints. |
| API contract expectation | Docs/API/schema authority side in the conflict object, with binding/freshness evidence before it can override source or tests. |

## Omitted Behavior

Phase28 intentionally does not adopt:

- autonomous conflict resolution outside the bounded repair task;
- hidden confirmation questions;
- memory/case retrieval;
- retry expansion after conflict detection;
- verifier weakening;
- provider/model-specific repair policy.

## Review Result

Review findings applied:

- Kept C33 as a shared contract boundary instead of embedding it in profiles or
  verifier diagnostics.
- Required explicit authority inputs before any selected action can be
  admitted.
- Required the repair target/action to be distinct from the authoritative side.
- Required ambiguous conflict safe-stop as first-class proof.
