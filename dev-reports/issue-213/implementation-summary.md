# Issue 213 implementation summary

## Outcome

Doctor now preserves the configuration error that prevented it from deciding
whether a selected preset exists. A typo in another preset is reported as the
actual unknown key and the selected preset is described as uninspectable,
rather than incorrectly reported as `not found`. Malformed TOML is reported as
an `invalid TOML syntax` failure before CommandAgent's supported-key validation
runs.

## Runtime changes

- Added a TOML syntax-validation pass in `src/config.rs` using the repository's
  existing `toml` dependency. The established line-oriented parser still owns
  CommandAgent section, key, value, and source validation after syntax passes.
- Extended the crate-private config inspection result with the exact errors
  from config files that could not be parsed or validated.
- Updated the `config.preset` doctor check to prefer those preserved causes over
  the `not found` branch when configuration resolution failed. The check's JSON
  details now include `inspection_errors`; existing IDs and schema version are
  unchanged.
- Kept a successfully resolved preset independent from errors in lower-priority
  files that normal preset resolution did not need.

## Tests

- Added a doctor CLI regression covering a valid selected preset plus an
  unknown key in another preset. It asserts the exact bad key is retained and
  the selected preset is not reported as missing.
- Added a doctor CLI regression covering malformed TOML that the old parser
  classified as an unknown key. It asserts syntax-error classification and the
  absence of the misleading unknown-key diagnosis.

No event/corpus contract, guarded runner chokepoint, historical evidence, or
live `.anvil/` namespace changed.
