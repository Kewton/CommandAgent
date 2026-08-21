# Issue 208 Implementation Summary

## Implemented

- Replaced the generated Python CLI fallback package identity `anvil_app` with
  the brand-neutral `app`.
- Made Python CLI required paths goal-aware:
  - `greet.py` selects `src/greet/main.py`;
  - `wc.py` selects `src/wc/main.py`;
  - filename mentions remain detectable next to sentence punctuation or
    Japanese text;
  - an explicit `src/<package>/main.py` keeps that package;
  - otherwise an existing `[project].name` or source package is retained;
  - an identityless project falls back to `src/app/main.py`.
- Prevented `before_phase` from materializing a package directory when only a
  prospective/default identity is known. It may still fill in missing project
  metadata for an already-existing `src/<package>/main.py`, without creating a
  second source package.
- Required an explicitly named goal entrypoint during invariant checks instead
  of accepting an unrelated existing source package as a fallback.
- Kept scaffold completion to one entrypoint and made both generated
  `pyproject.toml` and fallback CLI output use the selected package identity.
- Updated the built-in python-cli manifest's generation prompt, ownership
  vocabulary, default artifacts, and concrete usage example to describe the
  goal-derived `src/<package>/main.py` contract. The existing `cli/main.py`
  C1-C4 bindings and evidence mappings remain unchanged for #205/#285
  compatibility.
- Updated Python CLI conformance and corpus path expectations from the legacy
  branded package to the neutral/default and explicit-name paths.

## Tests

- Added focused unit coverage for explicit `greet.py` and `wc.py` goals,
  punctuation and Japanese-bound filename mentions, explicit package
  entrypoint paths, existing project precedence, the neutral fallback, no
  eager or second-package creation, strict goal-entrypoint verification,
  existing-source metadata completion, and generated-content branding.
- Updated the conformance Python CLI scenario to use `src/app/main.py` and
  matching project metadata.
