# Issue #415 implementation summary

## Result

Gate 4 now offers an independent, confirmation-gated Recovery Run. The first
action freezes the resolved Recovery Plan's exact bytes and returns a card with
the source/frozen paths, SHA-256, phase IDs, permission policy, automatic-run
budget, and Gate 1 identity hash. The GUI cannot send the confirmation until
the dedicated acknowledgement is checked, and the server independently
requires `acknowledged: true`.

The required Issue #414 predecessor commit (`9303570f`) was incorporated before
implementation so plan selection uses the shared resolved control/promoted
Recovery lineage.

## Runtime boundary

- Added `tui::boundary_shell::recovery_run` as a leaf module. It parses the
  resolved Ultra Plan, stores byte-identical frozen YAML, hashes the proposal,
  and persists a one-shot confirmation record.
- Proposal confirmation revalidates the session identity, policy and budget,
  latest proposal round, treatment state, resolved source path and bytes, and
  frozen bytes. Frozen plan symlinks and paths outside the workspace contract
  are rejected.
- Rejected treatment, unresolved treatment, drift, stale/used hashes, pending
  additional requests, and workspace lease conflicts remain reasoned denials.

## GUI server and execution

- Added proposal and confirmation POST routes below
  `/api/sessions/{id}/recovery-runs`.
- Confirmation acquires the existing workspace lease and launches
  `--run-ultra-plan <frozen-plan>` through the existing identity-bound command
  builder, retaining providers, models, profile, intent, pack, permission
  policy, and automatic-Recovery budget.
- Recovery runs register the normal process generation, so the existing status
  page and generation-bound stop endpoint monitor and cancel them.
- Current-interval projection now recognizes a new CLI command after a terminal
  result without introducing a new event name. Lease completion requires new
  terminal evidence from that recovery process.

## GUI and compatibility

- Added the Gate 4 `Recovery Plan を実行する` action and confirmation card,
  including explicit acknowledgement and disabled-until-checked confirmation.
- Added typed API contracts and actionable error guidance for every Recovery
  Run denial.
- Kept the additional-request flow separate and retained the existing
  no-automatic-recovery safety copy.
- Did not change Gate 1 confirmation hash construction, existing event schemas,
  or existing corpus fixtures. Added a new Issue #415 corpus case and updated
  the GUI user guide, help map, contract version, and changelog.

## Tests

- Added leaf tests for exact-byte freezing, source/frozen drift, stale proposal
  hashes, rejected/unresolved treatment, one-shot confirmation, and frozen-plan
  symlink rejection.
- Added GUI server integration tests covering no start before acknowledgement,
  same-identity arguments, displayed/executed SHA-256 equality, monitoring,
  cancellation, lease conflict, pending follow-up, drift, stale hash, and
  treatment rejection.
- Strengthened the GUI guard to pin the confirmation card fields and checkbox
  gate.
