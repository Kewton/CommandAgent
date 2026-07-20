# Issue #23 Design

## Scope

Add the repository-level files needed to make the existing `license = "MIT"`
package declaration concrete and to document a maintainable contribution and
release-note workflow. This is a documentation-only change: Rust behavior,
event schemas, CI configuration, corpus fixtures, historical evidence, and the
live `.anvil/` namespace remain unchanged.

## Predecessor and Path Decisions

- Base this work on the completed Issue #19 commit `cffd8da`, which introduced
  parallel English and Japanese READMEs. Update both license sections together
  and preserve their section structure.
- The docs-reorganization successor work is not committed in its assigned
  branch, so keep the current canonical paths `docs/dev-guardrails.md` and
  `docs/mechanism-ledger.md`.
- Treat `Cargo.toml` and `.github/workflows/ci.yml` as source material rather
  than edit targets: they already declare Rust 1.88, MIT, Python 3.10, warning
  denial, and the required regression jobs.

## Files and Content

- Add `LICENSE` with the complete MIT License text and
  `Copyright (c) 2026 Kewton`.
- Add `CONTRIBUTING.md` in English. Cover the supported toolchain, normal and
  PTY test commands, Python evaluation requirements, CI regression suites,
  development and compatibility guardrails, bilingual-doc synchronization,
  warning-free PR expectations, and the requirement to update the
  changelog's `Unreleased` section.
- Add `CHANGELOG.md` following Keep a Changelog, beginning with an
  `Unreleased` section and an explicit pointer to Git history and the mechanism
  ledger for changes before the changelog began at 0.1.0 in July 2026.
- Replace the `Cargo.toml`-only license wording in `README.md` and
  `README.ja.md` with direct links to `LICENSE`.

## Verification

Use focused documentation checks to validate the canonical MIT text and
copyright, required contributor-guide phrases and commands, changelog shape,
README parity, local Markdown links, and whitespace. Run
`cargo test --all-targets` as the contributor-facing baseline command described
by this change. No PTY execution is required because the command is documented
from the existing test target rather than changed here.
