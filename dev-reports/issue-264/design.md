# Issue 264 design

## Objective

Close the post-merge W6 integration gap on current `origin/develop` without
changing product behavior. Port the seven strict GUI smoke-contract repairs
that were proven by the Issue 174 overlay at
`cb2a5c8423876bca638b6a5600cd14accc37176e`, pin those repairs in the existing
read-only source guard, and replace the README GUI walkthrough GIF with a real
recording from the current release binary.

## Starting point and ownership

- Worktree `HEAD` and freshly fetched `origin/develop` both resolve to
  `3eb1cca177daf968336fc53add86b789bcf06c4f`.
- The working tree is clean before this design note.
- Owned paths are limited to `gui/scripts/smoke.mjs`,
  `tests/gui_read_only_guard.rs`, `docs/assets/demo/gui-demo.gif`, and
  `dev-reports/issue-264/*`.
- Product GUI/Rust sources, README copy, tutorial screenshots, historical run
  evidence, live `.anvil/` state, and user-owned integration changes remain
  untouched.

## Contract repairs

Apply the Issue 174 repairs without weakening their surrounding assertions:

1. Match the visible Gate 1 panel label (`GATE 1 / 見積り`) while retaining the
   complete Japanese contract-card checks and the internal-copy exclusion.
2. Map the raw terminal gate value to the localized final gate label once and
   reuse that label for session-index checks.
3. Read session-index row visibility through `innerText`, preserving the CSS
   transformed label that a user sees instead of comparing raw DOM text.
4. Validate the conflict reconnect control as a visible native `BUTTON` with
   `type="button"` and the exact accessible session-specific name; retain the
   guidance, session-query, dispatch-count, and GET-only checks.
5. Match the approved localized incompatible-pack warning while retaining the
   selected-value, profile, request-body, and response-status checks.
6. When a running workspace lease exists, assert that contract checking is
   disabled before proposal, the inline notice names the session and explains
   the block, and both proposal and dispatch POST counts remain zero.
7. Wait for the localized visible polling status `実行中` while preserving raw
   API fixtures, ETag behavior, cadence, request-count bounds, and the full
   ten-minute simulated duration.

Update only the directly corresponding string/source assertions in
`tests/gui_read_only_guard.rs` so future merges cannot silently restore the
obsolete harness assumptions.

## Recording

Build `target/release/commandagent`, verify its version, create a throwaway
execution root outside the repository, and run
`scripts/demo/record_gui_demo.mjs` against that binary and root. The script must
produce the actual `docs/assets/demo/gui-demo.gif`; no mock recording or
historical asset reuse is acceptable. Inspect the recording command and the
resulting GIF metadata/frames. README text and tutorial screenshots change only
if the real command proves they are incompatible.

## Verification

Run the narrowest checks first, then the full integration and repository suite:

- `node --check gui/scripts/smoke.mjs`
- focused `gui_read_only_guard` tests for the repaired contracts, followed by
  the complete `gui_read_only_guard` target
- GUI lint, typecheck, and build checks
- release build and `target/release/commandagent --version`
- full root/proxy GUI smoke with the release binary and isolated output
- the real GUI demo recording command against a throwaway execution root
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`

Any unavailable prerequisite, failed smoke case, failed recording, or failed
required verification leaves `verification.md` at `blocked` and prevents a
commit. A commit is created only after every required check and the real GIF
recording succeed.

## Parent integration scope amendment: UAT repair loop 1

The completed full root/proxy smoke isolated one additional merged-harness
drift in the Issue 75 run selector. Product code correctly formats every run
option through `repositoryRunStatusLabel(state, status_text)`, while the smoke
still compares the visible option to raw `status_text`.

The amended repair is limited to a strict JavaScript equivalent of that product
formatter inside the smoke's expected-option construction. Preserve the exact
date and run-ID comparison, and add only the directly corresponding source
guard. Re-run syntax validation, the focused read-only guard, and a fresh full
root/proxy smoke. If all required checks pass, revise the implementation and
verification reports and commit only the owned paths; otherwise stop at the
new honest failure.
