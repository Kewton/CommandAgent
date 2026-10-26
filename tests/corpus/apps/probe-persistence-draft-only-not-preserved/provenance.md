# Fixture provenance

This fixture is synthetic. It is derived from the S-119a investigation and the
probe at develop 46d99b14 (IC-31, Issue #875); no generated app or recorded run
was copied.

`src/app/page.tsx` is a minimal contract-mode app: a search input whose term is
kept in `localStorage`, a `data-anvil-action="primary"` control that commits
nothing, and a results heading that echoes the term. The exported state is
`{ search, visibleCount }`, and both keys change while typing, so they are the
draft.

`browser-interaction.json` is the evidence the probe writes for that app after
IC-31. The typed token is echoed outside the input and is still on the page after
the reload, but the keys a commit changed (the state keys that moved between the
start of text entry and the reload, minus the keys typing alone moved) are empty.
Persistence is therefore `not_evaluated` with reason `no_mutation_observed`, the
run fails with `persistence_not_evaluated:no_mutation_observed`, and
`informational_failure_kinds` records `persistence_draft_only`. Before IC-31 the
typed-token branch reported `preserved` with `persistence_changed_dimensions` of
`["typed_token"]`, so a draft copy satisfied the persistence evidence.

The behavioral status for `persistence_evidence` is
`unverified:not_evaluated:no_mutation_observed`.
