# Phase24 Source Alignment Matrix

Date: 2026-06-23 JST

| coverage id | Anvil source files | adopted behavior | intentionally omitted behavior | CommandAgent target modules | proof method |
| --- | --- | --- | --- | --- | --- |
| C07 | `artifact_ledger.rs`, `artifact_ledger_state.rs`, `repo_edit_observation.rs`, `post_tool_reconciliation.rs` | Record deterministic artifact observations from graph nodes, tool records, reads, writes, edits, verifier mentions, workspace observations, setup deltas, scaffold deltas, and pass-side completion authority inputs. | Hidden post-tool jobs, unbounded repo scanning, case memory, or semantic guessing about unobserved files. | `artifact_ledger.rs`, `artifact_graph.rs`, `artifact_ownership.rs`, `workspace_scope.rs`, `evidence_authority.rs`, eval scripts | Unit tests for every ledger source family; eval report tests; focused ledger fixture; broad sign-off regression. |
| C08 | `completion_evidence.rs`, `success.rs`, `completion_probe_gate.rs`, `objective_evidence.rs`, `evidence_observation.rs` | Convert already observed verifier, command, file-layout, docs, data, report, and profile-wide facts into typed completion evidence consumed by completion authority and eval. | Hidden completion probes, implicit build/test/schema execution, success-until-passing checks, or provider-specific completion policy. | `completion_evidence.rs`, `evidence_producer.rs`, `evidence_authority.rs`, `artifact_completion.rs`, eval scripts | Completion evidence/evidence producer/evidence authority tests; focused completion fixture; broad sign-off regression. |
| C09 | `evidence_binding.rs`, `evidence_runner.rs`, evidence-binding adapters | Produce deterministic binding facts for manifest identity, import/route symbol, executable handle, test script, docs section, schema column, citation, and file layout. Missing/failed bindings become structured contract evidence. | Hidden evidence runner orchestration, new repair dispatch logic, or profile-specific workflow engines. | `evidence_binding.rs`, `evidence_producer.rs`, `evidence_authority.rs`, `recovery_contract.rs`, `recovery_orchestration.rs`, eval scripts | Evidence binding/evidence authority tests; focused binding fixture; broad sign-off regression. |
| C10 | `deliverable_obligation_audit.rs`, `task_contract_deliverable_projection.rs`, `task_contract_deliverable_lifecycle.rs`, `deliverable_freshness.rs` | Project required deliverables, evidence requirements, and freshness rules into task/profile/eval facts; prevent stale read-only observations from satisfying fresh deliverable obligations. | Hidden lifecycle manager, broad semantic freshness inference, or forcing docs/data/report tasks into source edit workflows. | `deliverable_obligation.rs`, `task_contract.rs`, `plan_lint/mod.rs`, `evidence_authority.rs`, `artifact_completion.rs`, eval scripts | Deliverable obligation/task contract/plan lint/evidence authority tests; focused freshness fixture; broad sign-off regression. |

## Review Result

Review findings applied:

- Mapped every Phase24 coverage row to explicit Anvil source files and
  CommandAgent target modules.
- Marked omitted Anvil behavior so Phase24 does not import hidden runners,
  workflow dispatch, or unbounded evidence probes.
- Required producer and consumer proof per row, not only type existence.

## Implementation Result

The adopted behavior for C07-C10 is implemented in the CommandAgent target
modules listed above. Intentionally omitted Anvil behavior remains omitted:
hidden post-tool jobs, hidden evidence probes, workflow dispatch, and unbounded
repair are still outside Phase24 and outside the current runtime boundary.
