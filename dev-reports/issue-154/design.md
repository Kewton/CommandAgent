# Issue 154 design

## Problem

The documentation has three related drift classes. New readers can open an
entry page, a tutorial, or a reference, but the intended progression between
those layers is not explicit. Introductory surfaces also use different sample
goals even though the CLI recording and GUI first-run preset share one real
`python-cli` goal. Finally, the existing doc-drift test validates links in only
a small hand-picked set, approximates anchors without duplicate-heading
behavior, and binds only the English flag and command tables to runtime.

The predecessor tips were inspected before this design. Issue 147 adds the
REPL Gate 1 `/confirm` flow and raises the slash registry to 18 primary entries
and 19 accepted names. Issues 160/161 refine GUI static/catalog behavior, and
Issues 245/246 refine co-located extension discovery and tolerant listing.
Issue 154 must describe the combined behavior without weakening any runtime or
verification contract.

## Change

- Make the bilingual README navigation name the three learning layers and keep
  the complete first-run Gate 1 flow introduced by Issue 147.
- Route the CLI entry page through the detailed EN/JA tutorials and from there
  to the language-matched references, so each layer is reachable from a root
  README in at most three clicks.
- Use `Create a CLI --pattern filter command`, the existing CLI recording and
  GUI preset goal, on introductory CLI surfaces. Keep the separate ingest
  walkthrough as an intentionally profile-specific advanced example.
- Repair current local link and fragment typos in active root, guide, demo, and
  developer documents. Do not edit immutable migration files or run evidence.
- Extend `tests/doc_drift.rs` to scan the maintained documentation set, derive
  GitHub-style heading slugs including duplicate suffixes, and report all
  broken local links together. Extend bilingual drift coverage to compare the
  table row shape of every EN/JA guide pair and bind both language flag and
  slash-command tables plus their advertised counts to the Rust registries.

## Compatibility and verification

This is a documentation and test-contract change. It does not alter CLI/event
schemas, verification semantics, `.anvil/`, or historical evidence. Run the
focused `doc_drift` target first. Because the test compiles the shared CLI and
slash registries and predecessor production commits are part of the verified
baseline, also run formatting, Clippy for all targets, and the full Rust suite.
