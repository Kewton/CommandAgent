# Issue 264 implementation summary

## Implemented harness closure

- Ported the seven UAT-proven strict GUI smoke repairs from
  `cb2a5c8423876bca638b6a5600cd14accc37176e` into the merged harness:
  - match the visible `GATE 1 / 見積り` heading;
  - map raw terminal gates to their localized final labels once;
  - inspect session rows through visible `innerText`;
  - require the reconnect action to be a visible native button with the exact
    session-specific accessible name;
  - match the localized incompatible-pack warning;
  - prove a running lease disables contract checking before proposal and
    permits neither a proposal nor dispatch POST; and
  - wait for visible `実行中` while retaining the raw polling fixtures and
    ten-minute cadence assertions.
- Preserved all surrounding lease, reconnect, lifecycle, localization,
  request-body, ETag, polling-reduction, GET-only, and honest-failure checks.
  The newer merged Trial and extension-catalog wording outside those seven
  repairs was retained.
- Under the parent integration scope amendment, repaired the remaining Issue 75
  expected-option drift with a strict JavaScript equivalent of
  `repositoryRunStatusLabel(state, status_text)`. The comparison still requires
  the exact localized date, semantic status label, and run ID for every option.
- Updated only the directly corresponding source-contract assertions in
  `tests/gui_read_only_guard.rs`, including negative guards against the stale
  raw-label, raw run-status, anchor, lease-launch, and polling assumptions.

## Real GUI walkthrough

- Built the current release `commandagent` and GUI server, then ran
  `scripts/demo/record_gui_demo.mjs` against a real server using the release
  binary, local model `qwen3.8:27b-mlx`, and the throwaway execution root
  `/private/tmp/commandagent-issue-264-gui-recording.SqkFSy/execution`.
- The real session `01a02dbf-2c1b-7cb0-ae24-d3c79137f2f0` completed through
  Gate 3. The recorder produced 13 storyboard frames and the installed asset
  is a 1,000 x 625, 245-frame, 30.63-second GIF with SHA-256
  `807641ac30b5aab8ebbe2ff52f5d4d12211769b8fbaf9d4a5d2cbd5ed14f1427`.
- Inspected the overview, Gate 1, terminal result, history, and final GIF. No
  mock asset, token, or secret was used or captured. The recording proved no
  README-copy or tutorial-screenshot mismatch, so those files remain
  unchanged.

## Scope and disposition

- Production GUI/Rust behavior, event schemas, historical evidence, the live
  `.anvil/` namespace, README copy, and tutorial screenshots were not edited.
- The fresh required full smoke completed root and proxy with aggregate
  `ok: true`, including the amended run-selector contract and all seven prior
  repairs. Every required worker check and the real recording passed, so the
  local-commit condition was met.
