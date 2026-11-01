# artifact-repair-recheck (Issue #949)

Synthetic event contract for a final-acceptance repair that changes the bytes
of an artifact an admitted `artifact_contract_check` inspected.

## What the cases pin

- A failed check, a repair that changes a declared contract JSON or YAML (or an
  input the admitted check bound), then the **same** check again inside the
  existing bounded repair. The last receipt is bound to the final bytes; only a
  passing rerun reaches the existing success path.
- A same-bytes rewrite, an mtime-only touch, and a report-only change are no
  change: those cases keep the develop `4edfc5a7` sequence and their repair
  events stay byte-identical to the events observed there.
- A rerun that still fails (invalid content, adapter exit without an envelope,
  adapter timeout, a required row `unresolved` or `not_run`) stays a native
  failure, and no case exceeds the existing two repair attempts.

`fixtures/events.json` lists, per case, the test that runs it, the projected
`base_sequence` observed at develop `4edfc5a7` (every case stopped with
`final_acceptance_repair_no_source_change` before a second receipt), the
expected `sequence`, and the receipt and repair-attempt counts.
`tests/artifact_repair_recheck.rs` runs every case through `/run-ultra-plan`
and compares the observed sequence with this file.

## Provenance

Hand-authored. The profiles (`recheck-json`, `recheck-yaml`, `recheck-split`),
the adapter `state-gate`, and the file contents (`{"state": "draft"}`,
`state: ready`, ...) are generic stand-ins built at test time in temporary
directories. The adapter is a POSIX shell script run by the operator-pinned
`/bin/sh`; the planner and executor are scripted fake clients.

No external smoke task, grader input, expected value, model output, generated
code, host path, or credential is stored or copied here. The cases do not
measure a model's ability, and no live model, external provider, or real key
is used.
