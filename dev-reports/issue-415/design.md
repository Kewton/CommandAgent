# Issue #415 design

## Problem

Gate 4 currently exposes the resolved Recovery Plan only as a readable document
or as source material for an additional-request continuation. The GUI has no
separate, confirmation-gated operation that executes the recorded plan exactly
as shown. Reusing the additional-request path would change that feature's
artifacts and semantics, while launching the live source path would allow its
bytes to change between review and execution.

Issue #414 is a required predecessor and is not present in this branch's
starting commit. Its committed recovery-lineage resolver must be incorporated
before #415 so the GUI freezes the control/promoted plan selected by the same
rules as terminal projection, `/resume`, and directive continuation.

## Decision

Add an independent Recovery Run boundary artifact in a new
`tui::boundary_shell::recovery_run` leaf module. Proposal creation will:

- require the current terminal to be a failed Gate 4 result;
- reject a rejected or unresolved automatic-Recovery treatment;
- resolve the current plan through the Issue #414 terminal projection;
- read and parse the workspace-confined plan;
- copy its exact bytes to a deterministic frozen Recovery Run path;
- bind the source path, frozen path, exact-byte SHA-256, phase IDs, original
  Gate 1 identity hash, permission policy, automatic-Recovery budget, session,
  and round into a persisted confirmation proposal.

The confirmation API will require an explicit boolean acknowledgement in
addition to the proposal hash. It will revalidate the proposal, original Gate 1
identity, current resolved plan path and bytes, current terminal/treatment
state, absence of a pending additional request, and workspace lease before it
persists confirmation or starts a process. A changed source/frozen plan is
drift; an unknown, superseded, or already-used proposal hash is stale. Every
denial returns a machine-readable reason without starting the CLI.

After confirmation, the GUI server will launch
`--run-ultra-plan <frozen-plan>` through the existing identity-bound command
builder. That preserves the confirmed providers, models, profile, intent,
pack, permission policy, and automatic-Recovery budget. The existing process
generation registry remains the source for monitoring and cancellation.

## GUI and compatibility

Gate 4 will add a **Recovery Plan を実行する** action that first obtains the
proposal and renders its path/hash, frozen path, phases, permission policy, and
automatic-run budget. The confirm button remains disabled until the dedicated
checkbox is selected. The card is cleared when the terminal interval changes.

Additional-request state and endpoints remain independent. Gate 1 identity and
confirmation hashes are not modified. Existing event names and payloads are
not changed; recovery execution uses a small boundary-run state record plus the
existing CLI event stream. Existing corpus fixtures remain byte-identical.

## Verification

Add leaf-module tests for exact-byte freezing, drift, stale hashes, rejected and
unresolved treatments, and one-shot confirmation. Add GUI-server integration
coverage for no-launch-before-confirmation, the displayed/executed hash,
same-identity arguments, monitoring/cancellation generation, pending-directive
and lease conflicts, treatment rejection, drift, and stale confirmation. Run
the focused Rust tests, GUI checks, formatting, Clippy, and the full Rust suite
because shared GUI execution behavior is touched.
