# Issue 256 design

## Scope

- Edit only `docs/dev/extension-catalog.md` for the delivered maintainer
  guidance, plus the required Issue 256 reports under `dev-reports/issue-256/`.
- Do not change runtime vocabulary, family/intent registries, classifiers, tool
  registries, tests, or historical band evidence in this row.

## Documentation design

- Add a family-extension procedure that names
  `src/tui/boundary_shell/family_catalog.rs` as the canonical catalog delivered
  by Issue 214. The procedure will begin with formal band evidence, require one
  closed typed identity and one catalog entry, preserve aliases at the parsing
  boundary, and enumerate the dependent band, routing, producer-vocabulary,
  profile, and corpus/conformance updates that a future implementation owns.
- Add an intent-extension procedure centered on the typed `IntentId` and its
  closed `IntentContract`. The procedure will require contract semantics,
  profile hooks, family/band combinations, parsing/routing, presentation and
  pack compatibility, evidence, corpus coverage, and honest-failure tests to be
  updated together.
- State the ordering and completion conditions explicitly so neither procedure
  can be read as permission to add a free-form token, fabricate a band row, or
  weaken admission and verification gates.

## Verification

- Review the rendered Markdown source and repository-relative links.
- Run `git diff --check` to catch whitespace errors.
- Confirm the commit changes only the approved guide and required Issue reports.
