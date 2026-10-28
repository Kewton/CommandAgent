# Fixture provenance

This case backs Issue #900 (IC-44): a contract-mode editor that saves an
existing record. It keeps recorded data and synthetic scenarios apart.

## Recorded

`fixtures/recorded-markers.json` copies the marker sequences and HTTP mutation
observations of two recorded probe runs, with the source file and line of each
value. The recorded typed token is replaced by `{token}`; nothing else is
changed. The sources are read-only evidence; no generated app, raw request or
response body, secret, or absolute path was copied.

- L-137 content-q (campaign-G1-3a15, develop 2103047c): POST `/api/notes` 201
  before typing, PUT `/api/notes/<id>` 200 on the save press, and a reload that
  returns to the list. Typing and saving both change `content`, so the commit
  keys are empty and the recorded verdict is `not_evaluated` /
  `no_mutation_observed`.
- L-135 content-g (campaign-G1-3a14): PUT `/api/notes/new` 200, and the same
  editor and preview render the same content before and after the reload. Its
  recorded verdict is also `not_evaluated` / `no_mutation_observed`, and it
  must stay that way.

## Synthetic

`fixtures/fake-playwright.cjs` is a fake Playwright module that the real probe
script drives. It models a notes app and emits each request, response and
request failure through the page listeners when the app would send or receive
it. Its `recorded_content_q` and `l135_editor_after_reload` scenarios reproduce
the recorded marker sequences above; every other scenario in
`fixtures/scenarios.json` is synthetic: the positive app with one part changed,
and with the typed token kept by the server wherever the scenario allows, so
that only the changed part can stop the promotion.

The probe promotes a not-evaluated contract persistence verdict to `preserved`
only when all four parts of the commit witness hold: the fresh token was absent
before typing, the save press started a same-origin mutation that carried the
token and answered 2xx before the reload, after the reload no text entry is
shown and the token is visible outside one, and a read after the reload returned
the saved resource with the token, named by an identifier field (`id`, `_id`,
`uuid`, `slug` or `key`) of an array element when the read is the collection.
The save counts only when the probe's press actually happened, and a request the
browser started before the press never counts. The witness never reports
`reset`; anything short of all four parts leaves the verdict `not_evaluated`.
