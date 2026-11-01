# artifact-check-diagnostics (Issue #955)

Synthetic event contract for the accepted diagnostics of an
operator-registered `artifact_contract_check` adapter: they reach the existing
bounded final repair as untrusted data and the receipt event as one additive
field, `adapter_diagnostics`.

## What the cases pin

- A failed row's `check`/`target`/`code`/`message` reach the repair request the
  scripted provider receives, in both prompt layouts, inside one fixed
  `<artifact-check-diagnostics>` block; the scripted repair fixes the input
  and the same check reruns and passes. The control without diagnostics has
  the same receipt verdict and the same request minus the block.
- A run without diagnostics keeps the receipt event and the repair request
  observed at develop `20b549fc` (`base_events` of `plain-base-1`).
- Results the existing validation refuses (nonce, input digest, duplicate or
  missing required row, counts, unknown field, exit code, wire bounds,
  stream overflow, timeout, tooling changed mid-run) stay failed, and nothing
  they printed reaches the event or the request.
- Only `failed` rows and required rows that did not pass carry diagnostics;
  passed and optional unverified rows add none.
- Two receipts with the same row name keep their own groups; a later cycle's
  request holds only that cycle's receipts; a workspace report is never read.
- Injection text, the closing delimiter, line/fence/role breaks,
  ANSI/C0/C1/bidi characters, shell commands, and outside paths stay inside
  one well-formed JSON data line. This shows the framing, not that a model
  ignores injected text.
- Registered synthetic values and a secret-like token never reach the
  request, the events, or the summary.
- One event field and the combined repair section each hold at most 16
  items and 8,192 serialized UTF-8 bytes, measured just below, at, and just
  above each bound.
- In every case `sequence` equals `base_sequence`: diagnostics add no
  receipt, repair attempt, or provider call.

`fixtures/events.json` lists, per case, the test that runs it, the projected
`base_sequence` observed at the base, the expected `sequence`, and the receipt
and repair-attempt counts. `tests/artifact_check_diagnostics.rs` runs every
case through `/run-ultra-plan` and compares the observed sequence with this
file.

## Provenance

Hand-authored. The profiles (`diag-one`, `diag-two`, `diag-tamper`), the
adapters (`diag-gate-a`, `diag-gate-b`, `diag-gate-t`), the rows
(`rows/alpha`, `rows/beta`, `rows/gamma`), and every code and message are
generic stand-ins built at test time in temporary directories. The adapter is
a POSIX shell script run by the operator-pinned `/bin/sh`; the planner and
executor are scripted fake clients.

No external smoke task, grader input, expected value, model output, generated
code, host path, or credential is stored or copied here. The cases do not
measure a model's ability, and no live model, external provider, or real key
is used.
