# Issue 205 Design

## Problem

The admitted CLI manifest and its C1-C4 runtime already write
`evidence/cli-assurance.json`, but final acceptance dispatches that runtime only
for the legacy `cli` profile ID. The canonical `python-cli` profile instead
inherits `DomainProfile::behavior_probe`, which writes
`.anvil/evidence/python-cli-behavior.json`. Completion metadata then cannot find
the C1-C4 summary and honestly projects `static (cli_probe_not_run)` even when a
manifest-shaped Python CLI reaches final acceptance.

## Change

- Override `ProfileRuntime::run_behavior_probe` for `PythonCliProfile` and
  dispatch the existing manifest-bound `runtime::run_manifest_checks` adapter
  when the admitted manifest entry `cli/main.py` exists.
- Retain the pre-existing Python package behavior probe for legacy
  `src/<package>/main.py` workspaces that do not expose the manifest entry. A
  manifest-shaped workspace never falls back after a C-check failure.
- Preserve the existing check IDs, thresholds, evidence schema, event name, and
  status mapping; the change adds only the missing canonical-profile wiring.
- Keep the legacy `cli` dispatch intact for backward compatibility.

## Regression coverage

- Exercise final acceptance with `profile = "python-cli"` and a workspace that
  satisfies both the Python package invariant and the admitted CLI manifest.
  Assert that all C1-C4 checks pass, `evidence/cli-assurance.json` exists, and
  emitted completion metadata reports full assurance rather than static.
- Add a corpus fixture for the expected full C1-C4 assurance document and bind
  its contract fields in the corpus expectations.

## Verification

Run the focused runner integration test and corpus regression first, followed by
formatting, Clippy with warnings denied, and the full Rust test suite because the
shared profile runtime boundary is touched.
