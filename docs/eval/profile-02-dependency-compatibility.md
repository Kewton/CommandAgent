# Profile 02 Dependency Compatibility

Date: 2026-06-20

## Problem

UAT `workspace/mvp/uat/test0620_004.md` moved past the previous plan-file
parser failure and reached actual Next.js setup/build verification. The run
stopped in phase 1, step `verify-project-build`:

```text
dependency_setup_failed: step verify-project-build setup command `npm install`
exited with status 1.

npm error ERESOLVE unable to resolve dependency tree
npm error Found: postcss@8.0.0
npm error peer postcss@"^8.0.2" from autoprefixer@10.0.0
```

The generated manifest included:

```json
"postcss": "8.0.0",
"autoprefixer": "10.0.0"
```

This is not a provider, native tool-call, execution, or plan YAML parse
failure. The relevant boundary is Next.js profile manifest compatibility plus
bounded dependency setup evidence.

## Design Decision

The existing structure is sufficient:

```text
Profile facts -> Profile compatibility rule -> Profile verification failure
Setup stderr -> bounded setup evidence -> explicit setup blocker
```

The implementation adds content to that structure rather than adding a new
control stack. It does not increase retry count, run hidden setup, continue
phases automatically, query package registries, or add provider-specific
behavior.

## Implementation Summary

- Next.js profile verification now detects the observed Tailwind/PostCSS peer
  dependency conflict:
  - `autoprefixer` exact major 10 or newer
  - `postcss` exact version below `8.0.2`
- The failure is reported as `nextjs_dependency_version_conflict` with
  `package.json` as the target.
- Tailwind dependency obligation wording now asks for compatible
  `tailwindcss`, `postcss`, and `autoprefixer` versions without turning plan
  lint into a version solver.
- Runtime setup failure rendering now classifies the observed npm `ERESOLVE`
  peer dependency pattern as `npm_eresolve_peer_dependency` when stderr
  contains the dependent package, required peer range, and observed package.

## Focused Tests

Focused tests added or updated and covered by `cargo test`:

```text
cargo test nextjs_verification_rejects_postcss_autoprefixer_peer_conflict
cargo test nextjs_verification_accepts_compatible_postcss_autoprefixer_versions
cargo test nextjs_verification_accepts_postcss_version_range_for_tailwind_stack
cargo test setup_failure_evidence
cargo test setup_failed_blocker_message_renders_peer_dependency_evidence
```

## Verification

Commit under test: `ce7f805` with a dirty worktree containing this change set.

Passed:

```text
cargo fmt --check
cargo test
python3 tests/test_eval_report.py
python3 -m py_compile scripts/eval_report.py
cargo clippy --all-targets -- -D warnings
cargo build --release
scripts/check_branding.sh
git diff --check
```

## Focused UAT

Focused UAT for this slice was kept deterministic rather than running a live
`npm install`, because the observed failure depends on package manager
resolution and can vary with registry/cache state.

The focused signal is:

- profile verification rejects the incompatible manifest pins before setup:
  `autoprefixer@10.0.0` with `postcss@8.0.0`
- profile verification accepts compatible exact pins:
  `autoprefixer@10.0.0` with `postcss@8.0.2`
- profile verification accepts a PostCSS version range instead of pretending
  to solve it locally
- setup evidence rendering classifies npm `ERESOLVE` peer dependency stderr as
  `npm_eresolve_peer_dependency` and names `package.json`, `autoprefixer`,
  `postcss`, and `^8.0.2`

## Interpretation

This change should make the `test0620_004` failure easier to classify and
repair. It does not claim that a generated Space Invaders game is complete,
visually polished, route-integrated, or playable. Those remain later profile,
planning, verifier, or app-quality concerns.
