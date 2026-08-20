# Issue 166 implementation summary

## Outcome

The GUI pack wizard no longer dead-ends after pin or retirement. Both terminal
states now expose **新しい version を作る**, which opens an editable copy at
the next patch version without reloading the page. The operator can immediately
use the existing **保存して検証** action to stage and verify that version.

## Changes

- Added one local wizard transition that increments the semantic patch
  version, preserves the displayed pack ID and members, and updates the
  embedded version identity in assist/eval YAML.
- Clears the prior version's verification report, issues, and retirement
  acknowledgement and enters `draft` at the editor step. No server mutation
  occurs until the existing bounded stage API is called.
- Exposed the same action after both pin and retirement while retaining all
  read-only controls for the old immutable version.
- Updated the GUI extension guide to distinguish the local draft copy from the
  subsequent server-side stage operation.
- Extended the source guard and provider-free browser smoke for the new state
  transition.

## Predecessor and compatibility

Fast-forwarded to required predecessor Issue 165 commit `38b7bc17` before the
Issue 166 implementation. The new version is therefore copied from the exact
members reconciled by Issue 165's saved-byte read-back behavior.

No lifecycle endpoint, pack schema, event name or bytes, corpus contract,
historical evidence, or live `.anvil/` runtime namespace changed. Pinned and
retired server versions remain immutable; the change does not add overwrite,
unretire, delete, or direct browser filesystem capabilities.

## Evidence

The wizard smoke passed at both supported base paths. Starting from pinned
`nextjs-acme@1.0.0`, each case created editable `1.0.1`, copied and re-identified
all displayed members, and confirmed the server reported it as `staged` without
a page reload. After pinning and retiring `1.0.1`, each case created editable
`1.0.2` with the same member-copy contract. The predecessor's exact-byte
reconciliation and pinned-byte equality assertions remained green.
