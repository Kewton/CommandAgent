# Issue 208 Design

## Problem

The Python CLI domain profile ignores the goal when it computes required
scaffold paths. On an empty workspace it therefore chooses the branded
`anvil_app` fallback, and `before_phase` writes that package before a planner
can honor an explicit name such as `greet.py` or `wc.py`. An ultra plan that
correctly chooses `src/wc/main.py` consequently receives a second,
unrequested `src/anvil_app/main.py` tree.

## Design

- Derive a package name from the first explicit Python filename in the goal.
  Use the filename stem (`greet.py` -> `greet`, `wc.py` -> `wc`), or the parent
  package when the explicit path ends in `main.py`. Normalize the result to a
  safe Python package identifier.
- When the goal has no explicit Python filename, retain the existing workspace
  precedence: `[project].name` from `pyproject.toml`, then an existing
  `src/<package>/main.py`. Use the brand-neutral package name `app` only when
  none of those signals exists.
- Keep the package scaffold contract as exactly `pyproject.toml` plus one
  `src/<package>/main.py` entrypoint. Do not eagerly write a fallback package
  during `before_phase` when an empty workspace has no naming signal; the
  goal-aware expected paths remain authoritative for later planning and
  scaffold completion.
- Render generated `pyproject.toml` and the fallback entrypoint with the chosen
  package identity rather than embedding `anvil_app` in their contents.
- Align the built-in python-cli generation guidance and artifact vocabulary
  with the goal-derived `src/<package>/main.py` convention. Keep the existing
  `cli/main.py` C1-C4 check bindings and evidence targets intact so Issue #205
  and #285 manifest-shaped workspaces remain backward compatible.
- Update the Python CLI conformance corpus and the existing setup-fallback
  corpus expectation to use the neutral `src/app/main.py` fallback.

## Tests and verification

- Add focused profile tests for `greet.py`, `wc.py`, `src/<package>/main.py`,
  existing `pyproject.toml`, the neutral fallback, and absence of eager
  `anvil_app` creation.
- Run the focused Python CLI profile and conformance tests first, then
  formatting, Clippy with warnings denied, and the full Rust suite because the
  profile's shared planner contract changes.
