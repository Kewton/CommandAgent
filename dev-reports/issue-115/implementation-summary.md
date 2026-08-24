# Issue 115 implementation summary

## Outcome

Implemented a complete local-pack creation wizard on the GUI **拡張** screen.
An operator can now choose a profile/intent target, start from a minimal pack or
the complete `nextjs-acme` example, edit all bounded text members, recover from
field-specific verification failures, pin the server-returned exact-byte hash,
and select that local pack in Trial without leaving the GUI.

## Changes

- Fast-forwarded the worktree through the required Issue 122 predecessor so
  the wizard documentation targets the reader-oriented GUI guide structure.
- Added a five-step `PackWizard` leaf component with responsive styling:
  **対象セル → 出発点 → 編集 → 検証 → pin**.
- Added the exact `nextjs-acme@1.0.0` assist, eval, and two material files as a
  browser starting template, plus a minimal identity-bound assist scaffold for
  other registered profile/intent cells.
- Added a base-path-safe extension API client that delegates list, detail,
  stage, verify, pin, and retire operations to the existing authenticated Issue
  114 routes. It exposes no PUT, PATCH, DELETE, pin-overwrite, or unretire path.
- Retained the optional API diagnostic report on `GuiRequestError`. Client and
  server failures are rendered as itemized problems whose action returns to and
  focuses the responsible identity, token, YAML, or material control.
- Made every identity, target, starting-point, and member control immutable
  after pin. Retirement requires a separate irreversible acknowledgement,
  removes the Trial handoff, and leaves the wizard in a terminal read-only
  state.
- Documented the GUI workflow and added its launch action to the single-owner
  GUI help map.

## Verification coverage

- Added a focused Rust source guard for wizard wiring, actionable field focus,
  Issue 114 API delegation, lifecycle immutability, Trial handoff, and absence
  of forbidden HTTP methods.
- Added a provider-free `--wizard-only` browser smoke. On both `/` and
  `/proxy/commandagent/`, it injects an unknown `assist.yaml` field, observes
  the expected 422, follows the focus action, repairs the file, verifies the
  exact repository hash
  `sha256:6dab3671f1750a85830185486cf94f199b227cd4f3d4eccfe03a30742cee7ac0`,
  pins it, confirms Trial preselection, and retires it.
- The smoke uses a fresh owner-private extension root and removes its scratch
  runtime after success. Its JSON result is stored under
  `dev-reports/issue-115/smoke/`.

## Compatibility and safety

The existing `SupplyRoot` remains the only extension write boundary and the
server remains authoritative for strict conformance, credential scrub, exact
hashing, pin immutability, and terminal retirement. No Rust production logic,
pack/event schema, `.anvil/` runtime namespace, corpus contract, or historical
evidence was changed. A pinned local example is still explicitly labeled
**ローカル（未承認・帯域未計測）** and gains no admission or measured band.
