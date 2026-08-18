# Issue 104 Implementation Summary

## Outcome

Issue 104 is complete as a design and institutional-contract revision. Pack
contract v0.1 now admits explicitly selected, unsigned operator-local supply
under `--extension-root` without treating that supply as reviewed, measured,
signed, or admitted. No runtime supply, catalog, CLI, GUI, renderer, or check
implementation was added; those remain assigned to Issues 109, 114, 115, and
116.

## Fixed contract

- Added direct UTF-8 `materials/*.md` members to the exact-byte composition
  hash in relative-path byte order, with a 65,536-byte file limit and
  262,144-byte aggregate limit. The existing domain separator and no-material
  encoding remain unchanged, so every existing pack hash is stable.
- Reserved `pack_material_document` with closed `file` and `max_bytes` params,
  a 16,384-byte render default, a 65,536-byte ceiling, and initial compatibility
  limited to the four Next.js create points named by Issue 116.
- Fixed `PackSource` and its serialized values to
  `admitted | repository | local`, including selection/mutation rules, exact
  Japanese disclosure text, extension-root precedence, and the local shadow
  warning.
- Fixed the extension-root owner-only write boundary, atomic stage, immutable
  pin, non-deleting `RETIRED` marker, and append-only `journal.jsonl` schema.
  The API names are `JournalEntry` and
  `planner::pack::supply::journal::append`.
- Split baseline pin conformance from admission: strict identity, vocabulary,
  compatibility, floor, path/bounds, scrub, and hash remain mandatory for
  local/repository selection; measured fixtures and rendering goldens remain
  mandatory for admission.
- Updated D-3c Gate 1 to allow only pinned, compatible, non-retired local or
  repository packs with unapproved/unmeasured disclosure. Merely present YAML
  remains unselectable.
- Narrowed Phase G's remaining supply scope to signatures, publisher identity,
  trust roots, revocation, and remote distribution. Unsigned local supply is
  explicitly not a Phase G substitute or admission path.

## Records and protection

- Recorded the design alternatives, prompt-injection/credential/hash threats,
  and decisions in `dev-reports/issue-104/design.md` before contract edits.
- Appended the decision to `docs/dev/mechanism-ledger.md` without rewriting
  historical records.
- Updated the canonical Phase E exit row in
  `docs/dev/integration-notes.md` to distinguish the fixed contract from queued
  implementation.
- Added a focused `tests/doc_drift.rs` guard covering the v0.1 hash/bounds,
  `PackSource`, Japanese displays, `pack_material_document`, journal schema,
  D-3c selection requirements, and Phase G disposition.

## Scope

No production Rust behavior, event name/schema, corpus contract, guide pair,
`.anvil/` namespace, or historical `workspace/management/runs/` evidence was
changed. No new runtime/write boundary exists until the downstream
implementation issues land.
