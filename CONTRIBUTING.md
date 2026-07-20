# Contributing to CommandAgent

Thank you for helping improve CommandAgent. Keep each change focused, preserve
existing compatibility contracts, and add tests that demonstrate behavior
changes.

## Development environment

Install these tools before working on the repository:

- Rust 1.88 or later, as declared in `Cargo.toml`
- Python 3.10 for the evaluation golden tests, matching
  `.github/workflows/ci.yml`
- A Unix-like pseudo-terminal environment for the opt-in PTY tests

Optionally install `just` to use the repository's task shortcuts:

```bash
cargo install just --locked
```

Run `just --list` to see the available tasks and `just ci` for the complete
local CI sequence. `just` is optional; without it, use the raw commands in
`.github/workflows/ci.yml`, which remains the authoritative CI definition.

Run the complete Rust test suite from the repository root:

```bash
cargo test --all-targets
```

The terminal PTY suite is opt-in because it needs a real pseudo-terminal:

```bash
ANVIL_PTY_TESTS=1 cargo test --test tui_pty
```

Python 3.10 runs the evaluation golden tests used in CI:

```bash
python3 -m unittest tests/eval/test_acceptance_contract.py
python3 -m unittest tests/eval/test_completion_contract_snapshots.py
python3 -m unittest tests/eval/test_false_positive_regression.py
```

Start with the narrowest relevant test while iterating, then run the broader
checks required by the affected surface.

## Engineering guardrails

Read [`docs/dev-guardrails.md`](docs/dev-guardrails.md) before changing
production code. CI enforces line-count budgets on the runner chokepoints and
their extracted leaf modules. A guarded file that grows beyond its recorded
baseline plus 2% fails CI. Put a new subsystem in a new module and keep wiring
changes in the chokepoints minimal; do not raise a baseline merely to admit
growth.

The compatibility policy is recorded in
[`docs/mechanism-ledger.md`](docs/mechanism-ledger.md). Existing event names,
JSON keys, and schemas are frozen. Preserve those interfaces unless a task
explicitly authorizes a migration. A change that touches a frozen contract,
invariant, or vocabulary must include the corresponding ledger entry and
compatibility review in the same pull request.

Do not rewrite historical evidence under `workspace/management/runs/` or
`docs/migration/`. Do not change the live `.anvil/` runtime namespace without
an explicitly authorized state migration.

## CI regression suites

In addition to `cargo test --all-targets`, CI runs these permanent regression
and compatibility checks:

- Corpus regression: `cargo test --test corpus_regression`, backed by
  `tests/corpus_regression.rs` and fixtures under `tests/corpus/apps/`
- Conformance matrix: `cargo test --test conformance`
- Generality guardrails: `cargo test --test generality_guardrails`
- Python evaluation golden tests and Codex harness validation

When an event, recovery flow, or corpus contract changes, add or update the
focused fixture under `tests/corpus/apps/` as part of the same change.

## Documentation translations

Update translated documentation in the same pull request:

- `README.md` and `README.ja.md`
- Corresponding pages under `docs/guide/en/` and `docs/guide/ja/`

Keep paired headings, code examples, links, and command names structurally
aligned. The CI doc-drift guard validates this structure; a translation may
differ naturally in prose, but it must describe the same current behavior.

## Changelog

Add a concise, user-facing entry to the appropriate category under
`## [Unreleased]` in [`CHANGELOG.md`](CHANGELOG.md) in every pull request. Do
not rewrite historical entries. Maintainers move accumulated entries into a
versioned section when preparing a release.

## Pull request checklist

Before requesting review:

- Keep the pull request to one coherent change and explain its user-visible or
  maintainer-visible impact.
- Add focused tests for behavior changes and update corpus fixtures when a
  corpus contract changes.
- Run `cargo test --all-targets` and any relevant opt-in or Python checks.
- Run the Rust checks with warnings denied, matching CI, for example:

  ```bash
  RUSTFLAGS="-D warnings" cargo test --all-targets
  ```

- Update both sides of every affected documentation translation pair.
- Update `CHANGELOG.md` under `Unreleased`.
- Record any authorized contract change in `docs/mechanism-ledger.md`.
