# D-3c boundary-shell shakedown 001

- Executed: 2026-07-31 17:04:35 JST
- Product build provenance: `1c41a61489a1b1b6c93f4ceb99676ba273c65906`
  plus the uncommitted B5 wiring later committed with this report
- Workspace: `/private/tmp/d3c-shakedown.GkRHzj`
- Input: `workspace/management/bench/assets/ingest/list/data/snapshots/events-list.html`
- Input SHA-256:
  `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07`
- Planner: `ollama / qwen3.6:27b-coding-nvfp4`
- Executor: `ollama / gemma4:31b-cloud`
- Selected route: `ingest × create × list`
- Selected pack: `no pack`
- Confirmation card:
  `sha256:564ec8f762ef42048d0f4e22ae088ba201865490de2e1c2d3ef10df103b9f62c`
- Product run ID: `019fb733-f316-7b63-b6ed-4331c4b9ac5b`

## Four-gate result

Gate 1 displayed the exact request, deterministic route bases, N1-N5 contract
floor, Full meaning, the formal elevated Window B value tag `4/6 = 66.7%`,
planner/executor pins, and explicit `no pack`. Dispatch remained unavailable
until the exact card hash was confirmed and persisted. Gate 2 then invoked the
existing product execution path without dialogue. The run ended at Gate 3 with
`status=completed`, `assurance=full`, `runtime_acceptance=pass`,
`final_acceptance=full_success`, and `release_gate=pass`; Gate 4 was therefore
not entered.

The machine acceptance evidence reported N1-N5 all `pass`. Candidate accounting
was `10 = 9 accepted + 1 excluded`; the excluded candidate had the recorded
reason `missing required name or date`. N2 bound Japanese date normalization
and document-year context, including `8/3(月)` plus the document fragment
`2026年` to `2026-08-03`.

The durable boundary artifacts are:

- `boundary-transcript.md`: exact Gate 1, confirmation, and Gate 3 presentation
- `boundary-confirmations/564ec8f762ef42048d0f4e22ae088ba201865490de2e1c2d3ef10df103b9f62c.json`:
  persisted frozen identity
- `boundary-sheets/564ec8f762ef42048d0f4e22ae088ba201865490de2e1c2d3ef10df103b9f62c.md`:
  generated five-section acceptance sheet

## Pre-dispatch calibration

The transcript retains an earlier, unconfirmed Gate 1 card whose route basis
incorrectly treated the Japanese word `パイプライン` as evidence for the
`pipe` failure family. No dispatch occurred for that card. The lexical rule was
narrowed to explicit pipe-failure wording, and a regression guard now proves
that `パイプライン` does not create `pipe` evidence. The corrected card was
rendered and explicitly confirmed before the sole product dispatch.

## Measurement disposition and scrub

This is a D-3c interaction shakedown, not a bench campaign. It does not enter or
modify any capability-band denominator. The displayed `66.7%` is the tracked
pre-execution Window B value, not a result awarded by this conversation.

Credential-pattern scrub found no token, secret, password, authorization, or
API-key material in the committed transcript, confirmation record, or sheet.
REPL history and the temporary product workspace remain uncommitted runtime
state. Historical run evidence was not rewritten.
