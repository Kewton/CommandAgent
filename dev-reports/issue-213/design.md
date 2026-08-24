# Issue 213 design: preserve preset diagnostic causes

## Goal

Make `--doctor` preserve the configuration failure that prevented preset
inspection. A validation error in another preset must not turn into a false
selected-preset `not found` result, and malformed TOML must be identified as a
syntax error before supported-key validation runs.

## Constraints and inspected contracts

- Keep the existing config search order, preset merge precedence, supported-key
  allowlists, doctor check IDs, and JSON schema version unchanged.
- Keep invalid files fatal during normal configuration resolution; this Issue
  improves diagnosis rather than allowing a selected preset to bypass errors
  elsewhere in the same file.
- Preserve unknown-key validation for syntactically valid TOML.
- Limit production changes to `src/config.rs` and `src/doctor.rs`; no event,
  corpus, guarded runner, or live `.anvil/` contract changes are needed.

## Design

1. Validate each present config file with the existing `toml` dependency before
   the line-oriented schema parser runs. Add file context and the explicit
   phrase `invalid TOML syntax` to syntax failures. Once syntax is known valid,
   the existing parser continues to enforce CommandAgent's supported sections,
   keys, and values.
2. Extend the crate-private config inspection result with the exact errors from
   files that could not be parsed or validated. Preset discovery remains based
   only on successfully parsed files, but the inspection result distinguishes
   “absent from all valid inspected files” from “inspection was incomplete.”
3. In the doctor preset check, give inspection errors precedence over the
   `found` and completeness branches. Emit a failed `config.preset` check that
   says the preset could not be inspected and includes the preserved causes;
   retain the existing `not found` result only when every searched file was
   inspected successfully.

## Tests and verification

- Add a focused doctor test with a valid selected preset and an unknown key in
  a different preset; assert that the other preset's key is reported and the
  selected preset is not described as `not found`.
- Add a focused doctor test with malformed TOML that the old line parser could
  classify as an unknown key; assert that doctor reports `invalid TOML syntax`
  and not `unknown config key`.
- Run focused config/doctor tests first, then formatting, Clippy, and the full
  Rust test suite because config parsing is shared CLI behavior.
