# Issue #73 Design

## Scope

- Keep `ConfirmationIdentity`, its serialized bytes and card hash, the Trial
  API routes/payload fields, explicit checkbox, and exact-hash dispatch check
  unchanged.
- Rewrite only the Gate 1 presentation markdown in Japanese so it explains the
  requested work, each required contract check, comparable-run result, write
  boundary, model pins, and confirmation ID in reader-facing language. Python
  CLI C1-C4 descriptions stay semantically aligned with
  `docs/cli-profile-contract.md`; other known create-profile checks receive
  descriptions as well.
- Render the returned `card_markdown` in the Trial page through a small safe
  leaf component with `data-testid="gate-one-card-markdown"`. Keep time/cost,
  the explicit write boundary, and confirmation controls beside it, but remove
  duplicated route/check and internal measurement labels.
- Make the Terminal heading a Gate 3/Gate 4 result summary. Show assurance only
  as a human explanation below it, and replace D-3d/scrub/persist labels with
  plain follow-up wording without changing directive APIs or behavior.

## Compatibility

Required predecessor commits #64 `7fcb0dbe`, #67 `f51c20b5`, #68 `73f57e8d`,
#69 `3ddda7ac`, #70 `52dd26ef`, #71 `c312eb75`, #76 `23c6f2ab`, and #80
`b84034b6` were inspected as committed sibling changes, not ancestors. This
patch does not merge or duplicate their lease, option, phase, feedback,
artifact, session-index, localization/runtime-status, or polling behavior. The
new markdown renderer is a leaf component, and smoke/page edits stay narrow so
those independently verified changes can be integrated normally.

Issue #76 fixes GUI-owned language to Japanese without i18n. Although this
worktree still has the pre-#76 English page, every reader-facing string added or
replaced by this Issue is Japanese now; translation is not deferred to merge
time. Opaque profile/provider values, hashes, filesystem paths, contract/API
references, and internal event/status identifiers remain unchanged.

## Tests and verification

- Extend Gate 1 presentation tests for described C1-C4 wording and preservation
  of the confirmation ID/instruction.
- Extend the GUI source guard and two-base-path Playwright smoke to require the
  markdown test ID, Japanese check descriptions and visible copy, and a
  Terminal heading that cannot equal assurance.
- Run focused Rust/GUI checks first, then formatting, Clippy, and the full Rust
  suite because shared TUI presentation and browser smoke contracts change.
