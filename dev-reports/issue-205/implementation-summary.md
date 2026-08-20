# Issue 205 Implementation Summary

## Outcome

Canonical `python-cli` final acceptance now executes the admitted manifest's
C1-C4 CLI checks when the workspace exposes `cli/main.py`. Passing checks write
`evidence/cli-assurance.json`, emit the existing profile behavior event with
that evidence path, and project `Assurance: full` instead of
`static (cli_probe_not_run)`.

## Implementation

- Added the missing `PythonCliProfile` runtime dispatch to
  `runtime::run_manifest_checks`.
- Kept the existing Python package probe for legacy
  `src/<package>/main.py`-only workspaces. Once `cli/main.py` is present, C-check
  errors remain failures and cannot fall back to the legacy probe.
- Left the legacy `cli` profile path unchanged.
- Changed no C1-C4 check identifiers, classification thresholds, evidence
  fields, or event schemas.

## Coverage

- Updated the final-acceptance integration test to use canonical
  `profile = "python-cli"` and assert full assurance with no
  `cli_probe_not_run` reason.
- Retained focused coverage for failure evidence and the older Python package
  run path.
- Added a corpus fixture that freezes the full C1-C4 assurance document and
  bound all relevant fields in `expectations.toml`.
