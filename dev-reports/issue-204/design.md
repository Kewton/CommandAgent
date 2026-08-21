# Issue #204 Design

## Problem

The built-in `python-cli` profile advertises and executes `python -m
compileall -q src`, and its non-virtualenv behavior probe also launches
`python`. A host that exposes Python only as `python3` therefore fails both
step verification and final profile acceptance in plan-run workflows.

## Design

- Make `python3 -m compileall -q src` the canonical non-virtualenv Python CLI
  verification command in profile expectations, runtime/generation guidance,
  final verification, the bounded pytest-timeout substitution, and the
  deterministic Python CLI setup fallback.
- Change only the behavior probe's non-virtualenv interpreter fallback to
  `python3`. Keep the existing `.venv/bin/python` (or Windows equivalent)
  branch first and unchanged.
- Keep both `python` and `python3` accepted by the build oracle so previously
  saved plans remain recognized; this is not an event or schema migration.
- Add a restricted-PATH plan-run regression test whose PATH contains `sh` and
  `python3` but no `python`, plus a focused assertion that an existing virtual
  environment interpreter still wins.

## Verification

Run the focused Python CLI profile tests first, then formatting, clippy, and
the full Rust test suite because planner guidance and shared plan-run behavior
are exercised.
