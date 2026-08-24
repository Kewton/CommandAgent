# Issue #367 implementation summary

## Implemented

- Added an optional, typed `intent` field to the GUI Trial session proposal and
  creation inputs. Omitted or `null` intent values retain the previous automatic
  inference behavior, while invalid strings and non-string values are rejected
  by request decoding.
- Bound explicit `create`, `fix`, and `investigate` values into Gate 1 and
  prevented request-word intent observations from overriding that binding. The
  confirmed identity is passed unchanged to the delegated CLI as `--intent` and
  remains the source of truth for reconnect rendering.
- Added the four-choice Japanese intent selector to Trial compose, including
  post-launch locking and frozen-identity display. Profile and intent edits
  clear the proposal, confirmation, and selected pack.
- Enabled compatible `fix` and `investigate` packs in the Trial catalog and
  pack-wizard handoff. Pack deep links adopt both the pack's profile and intent,
  and compatibility filtering prevents stale packs from remaining in request
  payloads.
- Updated Trial user documentation and added focused Rust, GUI source-contract,
  and Playwright regression coverage for typed validation, automatic-mode
  compatibility, conflicting goal vocabulary, pack resets, delegation, locking,
  reconnect, and both supported base paths.

## Compatibility and scope

- Existing clients that omit `intent` keep the historical request inference
  path.
- No event names or schemas, persisted runtime namespace, or historical evidence
  were changed.
