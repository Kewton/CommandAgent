# Issue 122 design: reader-oriented bilingual documentation

## Context

The current end-user reference is bilingual under `docs/guide/{en,ja}`, but GUI
setup, Trial use, history, extensions, operations, and first-run help are mixed
in the 575-line `docs/user/gui.md`. Issue 122 is the only authorized structural
documentation change after the M2 GUI and pack work landed. The product and its
honest-failure boundaries do not change.

## Design

1. Replace `docs/user/gui.md` with a compatibility index. Keep every existing
   H2/H3 anchor at that stable path and make each anchor link to its new owner:
   GUI onboarding, setup, Trial, history, extensions, or operations.
2. Add the reader-oriented pages named in the Issue:
   `getting-started-cli.md`, `getting-started-gui.md`, `gui-trial.md`,
   `gui-history.md`, `gui-extensions.md`, `gui-setup.md`,
   `gui-operations.md`, and `docs/dev/extension-catalog.md`. Move the current
   GUI contract without weakening token, Origin, workspace, evidence, or
   read-only guarantees; add only the missing onboarding and maintainer
   connective guidance.
3. Add `docs/user/gui-help-map.md` as the short one-to-one map from stable
   Japanese in-app explanations, empty states, and actions to owned document
   sections. Extend `gui/scripts/smoke.mjs` to verify the mapped copy on both
   root and reverse-proxy base paths.
4. Update both root README Quickstarts and the documentation/profile/pack
   indexes so CLI, GUI, and extension layers are directly reachable. Append an
   Issue 122 entry to the changelog and mechanism ledger; do not edit immutable
   migration or run evidence.
5. Extend focused Rust documentation guards to require the deliverable files,
   validate local Markdown links and retained legacy GUI anchors, require the
   three Quickstart routes in both READMEs, and point the existing workspace
   recovery assertion at its new page. Existing CLI flag, config key, slash
   command, and EN/JA file/heading parity checks remain unchanged.

## Verification

Run the documentation drift and GUI read-only guard tests first, then the GUI
type/lint/build and smoke checks because browser-smoke code is touched. Finish
with the repository-required formatting, Clippy, default test suite, and GUI
feature test suite. Record every required command honestly in
`verification.md`; use `blocked` if any required check cannot pass.
