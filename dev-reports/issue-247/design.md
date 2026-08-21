# Issue 247 / 248 design

## Context

Epic #260 assigns Lane I as the combined implementation lane for Issue #247's
duplicated external-manifest diagnostics and Issue #248's v2 external manifest
backend. The required Issue #217 predecessor is incorporated exactly: this
branch and
`feature/issue-217-help-man-minimal-loop-yaml-plan-runner-mvp` both point to
`8bb9879e4cafcfdff327e446cd5e1d48b8a64ed4` before implementation edits.

External v1 manifests currently deserialize through nested error wrappers that
both render and expose the same source. The binary's alternate error display
then walks those sources again, so one TOML parser cause appears three times.
The v1 external fixture also needs 91 lines because it must fill inert
Next.js-era `step_templates` fields. Issue #217 added the
`--validate-manifest` and `--init-profile` arguments but deliberately left
their execution to this lane.

## Design

1. Flatten external TOML decode failures at the file boundary. Convert a TOML
   span to one one-based line and column and retain only the parser's reason in
   a non-chained external error. All external manifest diagnostics therefore
   have one `path:line:column: reason` occurrence, while embedded v1 parsing
   keeps its existing API.
2. Add a closed external v2 schema in a leaf module. V2 keeps common
   `metadata`, `plan`, `artifacts`, `guidance`, and `checks`, omits external
   admission status by default, and expands validated declarations into the
   existing `ManifestV1` runtime representation with neutral shared defaults.
   Existing v1 documents continue through the original parser and runtime.
3. Add leaf backend functions for validating one `manifest.toml` or
   `overlay.toml` and for initializing
   `<extension-root>/profiles/<id>/manifest.toml`. Validation performs the
   normal syntax, identity, vocabulary, capability, and overlay-base checks
   without registering or running a profile. Initialization uses create-new
   semantics, emits a neutral valid v2 template no longer than 20 lines, and
   never overwrites an existing file.
4. Wire the two already-parsed direct actions at the start of `run`; do not
   change `src/cli.rs`, built-in manifests, runner chokepoints, event schemas,
   or `.anvil/` state.
5. Add an external v2 corpus fixture and focused binary tests covering v1
   compatibility, v2 doctor loading, one-cause doctor diagnostics with exact
   location fields, standalone validation, overlay-base rejection, template
   creation, and overwrite refusal.

## Verification

Run the new integration target and manifest-focused existing targets first.
Because shared Rust manifest loading and binary startup behavior change, then
run `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo test`.

## Independent review correction

Independent review identified that suppressing `ManifestError::Parse` from
the shared error source chain was broader than the external-diagnostic design.
The external loader already converts every parse error to the terminal
`ExtensionManifestError::Located` variant, so duplicate alternate rendering
is prevented without changing the direct/embedded v1 API. Restore the original
TOML source from `ManifestError::source`, keep `Located` non-chained, and pin
both sides with focused regressions.
