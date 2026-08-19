# Issue #109 Implementation Summary

## Delivered

- Added public `--pack ID@VERSION`, `--pack-hash SHA256`, and
  `--extension-root DIR` flags.
- Added top-level `extension_root` and preset `pack` config keys, including
  documented precedence and the `nextjs_acme_cagentpack` preset shape.
- Added `src/cli_pack.rs` as the leaf owner for selector validation, extension
  root before repository lookup, exact-byte loading, `pack.sha256` verification,
  explicit hash verification, conformance, profile/intent compatibility, and
  scoped runtime environment binding.
- Classified pack selection failures as pre-run CLI usage errors with exit code
  2 while preserving exit code 1 for existing execution failures.
- Extended the headless summary additively with an optional `pack` object
  containing `id`, `version`, `hash`, and `source`.
- Added the stable `pack.selection` doctor check for selected, absent, invalid,
  and configuration-unavailable states.
- Updated English and Japanese CLI/configuration references together and
  documented pack projection in the headless summary reference.

## Tests

Added `tests/cli_pack.rs` coverage for:

- extension-root precedence over a same-identity repository pack;
- summary identity/hash/source projection;
- preset-only activation through `nextjs_acme_cagentpack`;
- unpinned selector, missing pin, stale pin, explicit hash mismatch, profile
  mismatch, and preset/flag contradiction exit status 2;
- doctor JSON pack-selection details.

The leaf module also tests exact selector grammar, traversal-shaped ID
rejection, and source precedence. Existing headless, doctor, pack runtime,
documentation drift, protection coverage, and growth guardrails remain green.

## Prerequisites

The verified Issue #107 and #108 commits were not present on the dispatched
branch. They were inspected and cherry-picked in dependency order before Issue
#109 implementation (`a7eee060`, then `39283935`). No pull request, merge,
release, or external issue mutation was performed.
