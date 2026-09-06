# Issue #425 Reopen Implementation

Generated Next.js create contracts register the product build verifier before
phase execution. Recovery handoffs for read-only stagnation, phase execution or
invariant failure, final-acceptance repair failure/exhaustion, and empty bounded
repair now retain that run authority. Existing nonempty bounded-repair command
lists remain intact, including the 4/1/1 cases from comment 5550459250.

This completes the mechanical WIP port at 3d222aae on develop 0296d779. It
supersedes the inherited PR #427 implementation summary.

## Authority and acceptance

- Register validated, success-expecting implement/verify commands from admitted
  plans in generated Next.js/generic contracts. Setup, inspection, reporting,
  and expected-failure probes retain their step scope.
- Preserve registrations across acceptance refresh within the same execution.
  In-process scoped provenance distinguishes product-generated step contracts
  from configured files, including the same filename and canonical aliases.
  Fresh runs cannot import an old registry merely by matching profile/goal.
- Keep the three exact Next.js package-script assertions at their existing
  step/profile gates. The profile leaf identifies them using the existing
  command generators; final profile verification still rejects invalid build,
  dev-port and start-port settings. No tokenizer or JSON-policy rewrite is used.
- Complete legacy empty generated Next.js handoffs from profile authority.
  Generic runs with no registered observation continue to fail honestly.
  Configured and data contracts remain closed; Recovery candidates bind only
  their registered commands. Existing typed business acceptance remains required.
- Preserve event schemas, machine stop codes, and the readable stop summary
  added by PR #427. New registration reuses the provenance event with the
  `admitted_step_plan` or `profile_runtime` source. No runtime namespace or
  on-disk provenance migration is introduced.

## Regression coverage

Dedicated leaf tests cover generation/refresh, empty/registered contracts,
step verify presence/absence, step/phase candidate competition, all confirmed
handoffs, invalid authority, configured/data confinement, explicit filename
aliases, stale prior runs, and profile configuration checks. An executable
corpus fixture models the reopened failures and the existing 4/1/1 handoffs.

The existing browser-probe failure fixture now supplies the existing mock
Next.js build toolchain so it reaches its intended HTTP 500 failure. Its
assertions are unchanged. The existing known-profile success regression passes
without changing its fixture or assertions. Temporary diagnostic edits were
removed. Guardrail baselines are unchanged; initialization lives in a leaf and
the phase chokepoint only wires it in.

Two independent temporary sessions exercise real preflight, candidate binding,
plan preparation and a deterministic local repair. The registered JavaScript
syntax check changes from fail to pass, while persistence acceptance
remains failed and no successful Recovery is reported. This is local test
evidence, not a measurement of a regenerated real Next.js build or business E2E.
Live providers, original external sessions, remote lifecycle operations and CommandMate service
operations are excluded by the approved worker scope.

Issue #428 (da4c6a81) and #430 (492073d1) were inspected but not imported or
reimplemented. Issue #425 test wiring avoids their integration insertion sites.
