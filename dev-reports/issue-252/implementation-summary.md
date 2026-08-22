# Issue #252 implementation summary

## Outcome

Added the read-only 'commandagent --extensions' action. It inventories one
explicit or configured extension root without starting providers or executing
packs, and renders either one-line tab-separated rows or a structured
'commandagent.extensions/v1' JSON report.

## Implementation

- Fast-forwarded this worktree to the completed Issue #230 predecessor before
  editing, preserving its '--allow' field and scoped policy installation at the
  top of the CLI run.
- Added 'src/extension_inventory.rs' as the leaf owner for extension discovery,
  validation, usability projection, bounded journal-tail reading, and text/JSON
  rendering.
- Added '--extensions' as an exclusive action in 'src/cli.rs'. '--json' now
  requires either '--doctor' or '--extensions'; using it alone is still rejected.
- Profile and overlay files are validated independently with the existing
  manifest boundaries, so a malformed manifest does not hide a valid overlay.
  Rows include the exact-byte hash when the file is safely readable, draft or
  invalid status, overlay base, and the validation reason.
- Pack directories retain the closed pack decoder and conformance gates. Rows
  distinguish staged/pinned/retired lifecycle, missing or mismatched pins,
  conformance, compatible profile and intent, and decode errors such as an
  unregistered source.
- The latest non-empty 'journal.jsonl' record is read from a bounded tail and
  projected without rewriting history. Missing, invalid, symlinked, and
  oversized journal states remain explicit.
- Updated the bilingual CLI references and public-flag counts.

## Tests and fixtures

- Added 'tests/corpus/apps/issue252-extension-inventory/' with:
  - one malformed profile manifest;
  - one valid additive overlay;
  - one conformant but unpinned pack;
  - one pack containing an unregistered assist source;
  - two journal records proving only the final record is projected.
- Added 'tests/issue252_extension_inventory.rs' for the text contract, JSON
  contract, and configured-root fallback.
- Added focused Clap coverage for help, JSON ownership, and action conflicts.

## Compatibility

- No event name or schema, historical evidence, '.anvil/' state, profile
  admission, pack static-assurance cap, plan-YAML action, declarative command
  check, or execution gate was changed.
- The original Issue proposal also mentioned adding doctor remediation copy.
  The approved row decision narrowed ownership to 'src/cli.rs' plus a leaf
  inventory/projection module, so this implementation leaves the shared doctor
  surface unchanged.
