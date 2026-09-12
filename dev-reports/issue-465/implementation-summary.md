# Issue #465 implementation

Create-origin Recovery no longer completes an API repair merely because an Edit
failed, cat succeeded and required files already existed. Completion consults an
independent host repair obligation with the original failure, related sources,
registered contract, responsible steps and confirmation boundary.

## Product changes

- `src/planner/recovery_repair_obligation.rs` and its leaf modules capture and
  validate the obligation, choose finite dependency boundaries, preserve the
  relevant source/check surfaces, and run fresh host confirmation.
- Recovery inspection binding captures the obligation before model execution.
  Continuations retain its original sources and diagnostics, append the new
  failure evidence and receive a fresh attempt identity.
- `RunSessionOptions` carries the bound obligation. The existing completion helper
  checks it for assistant final, after-tool completion and iteration short circuit.
  Runner checks it before precheck completion and after initial/local-repair
  verification. `src/model_probe.rs` initializes the new optional field to None.
- Pending dependent work records owners, remaining steps, the confirmation boundary
  and the existing iteration cap. A later registered verifier producer can finish
  before group confirmation. Pending does not mean repaired.
- Verification is never reused from a prior source hash. Existing path checks,
  model assertions and old passes cannot discharge the obligation. Registered
  checks must pass; missing dependencies and different early failures remain failures.
- Preserve frozen existing verifiers/configuration, target public exports and
  relevant imported/diagnosed calls. Reject added diagnostic suppression and unsafe
  assertions; allow const assertions, import aliases and unused-helper cleanup.
- Re-scan the compiler/build/test configuration inventory before and after each
  confirmation. A new configuration file changes the original conditions even
  when the registered build succeeds. Ignore rules do not exclude configuration
  inputs, and symlinked files must resolve inside the candidate. Missing registered
  smoke scripts may still be produced; their absence is not a configuration lock.
- Add `recovery_repair_obligation_observed` with pending/unresolved/resolved status
  and additive completion-blocking evidence. Existing event names and fields remain.

## Host-record integrity

`write_read_only_bytes` is only a filesystem precaution. A run-scoped host-memory
binding holds the expected attempt, contract and obligation independently of disk.
Both Runner binding and completion confirmation compare the record to that binding.
Confirmation also rechecks it after executing the verifier. Deleting the record,
changing its contents or substituting a record from another attempt fails closed.
Removing both the record and inspection context cannot clear the host binding.

The real Write-tool test proves the existing private-path policy rejects direct
replacement. The real Bash-tool test actually removed the file (`record_exists=false`)
and proved that neither the loop nor the next Runner boundary completed. Separate
Runner tests cover content substitution and a foreign attempt with the same contract.
Legacy contexts without this new obligation remain readable; a new obligation file
without its live host provenance is rejected rather than trusted as resumable evidence.

## Tests and corpus

The new `tests/corpus/apps/issue465-repair-obligation/` contains an executable API /
store / policy analogue and a strict TypeScript variant. The frozen JavaScript check
invokes the actual API with accepted and rejected inputs. TypeScript replay captures
the actual `Expected 2 arguments, but got 1` failure before running repair.

Focused tests cover the unbound old cat-completion control, all requested non-repairs,
repair after feedback, related store edits, Bash edits, fresh no-edit confirmation,
stale generated-JSON inputs, suppression/removal/configuration bypasses, finite
producer dependencies, owner spelling/omission, every completion exit, continuation
provenance and runtime-record tampering. The explicit compiler test additionally
checks six real Runner variants, including related-store `as const` and import aliases.

The new-configuration regression first reproduced an incorrect `plan-run complete`
with an executable configuration-sensitive checker. After the fix, that same
registered check succeeds but Runner rejects the new configuration. An additional
explicit test runs real Next.js 15.5.20: without `next.config.js`, `next build`
reports the original arity error; adding `typescript.ignoreBuildErrors=true` makes
the build pass with identical API bytes, while Runner still rejects completion.
Restoring the original absent configuration and repairing the related store then
passes through Runner. The compiler tests use installed dependencies read-only,
temporary candidate roots and disabled Next telemetry.

The #456 negative replay now expects the earlier unresolved-repair rejection and
retained failing command, while retaining its candidate-rejection/control-preservation
assertions. It no longer requires spending the remaining candidate before rejection.

## Scope and practical limits

`requires_write`, fix safety, repair budgets, final verification, acceptance and
promotion gates are preserved. #466 producer formation and #467 output recognition
are not reimplemented. Existing output policy is used for source-mutation accounting.
No historical evidence, live runtime, original develop edits, README/CHANGELOG or
plugin cache was modified. No external lifecycle action was performed.

Source preservation is a bounded conservative check, combined with the original
registered verifier; it is not a proof of arbitrary program equivalence or business
correctness. The measured cases cover call deletion, commented calls, type suppression,
configuration exclusion and early failures. Final acceptance remains necessary.
