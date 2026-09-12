# Independent follow-up: executable hook error attribution

Owner handoff: the parent CommandAgent orchestrator/maintainer should schedule a
separate diagnostic Issue. No GitHub Issue has been created or changed by this
worker. Suggested title: `[cli][acceptance] Preserve executable Node failure
predicates through bounded summaries and contract-attribute diagnosis`.

## Deterministic reproduction after the #474 classification fix

Run `cargo test --lib issue474_diagnostic_handoff -- --nocapture`.
Inputs are the preserved original command, synthetic page, and
`tests/corpus/apps/issue474-compound-node-hooks/fixtures/diagnostic-handoff.json`.
The test executes the actual command with captured stderr, then calls the real
final acceptance boundary and existing attribute detector. No model is involved.

| Missing predicate | Raw stderr | Final CommandFailure.reason | Attribute diagnosis | Selected source |
| --- | --- | --- | --- | --- |
| primary | `Error: missing primary` | Actual error line absent | `data-anvil-state` | `src/app/page.tsx` |
| input | `Error: missing input` | Actual error line absent | `data-anvil-state` | `src/app/page.tsx` |

The state control also fails with `Error: missing state` in the three-predicate
execution test. All these checks remain StaticSyntax (`weak_evidence=[]`), and
final acceptance fails on the executable command. Unrelated API changes cannot
unlock acceptance. Correcting the declared hook and rerunning does progress.

The reproduction intentionally asserts the observed defect for handoff. It is
not a desired-behavior assertion and must be updated by the follow-up fix to
require primary/input/state attribution to the actual failed predicate.

## Responsibility and independence

- `src/tools/bash.rs::build_summary` extracts lines containing error-related
  words. Node's echoed one-line eval source contains all three Error calls and
  precedes the actual exception.
- `src/minimal_loop/build_verifier.rs::FullCommandOutput::excerpt` calls
  `eval_events::body_snippet`; the failed observation uses that excerpt as its
  `primary_reason`. The final `CommandFailure.reason` therefore loses the actual
  error line. Full output and display shortening are distinct channels; the
  follow-up must trace them separately.
- `src/planner/contract_attribute_repair.rs::missing_attribute_name` chooses
  state if its literal occurs anywhere in command/reason text, before considering
  action attributes. Even retaining the full error does not by itself fix that
  lexical-priority assumption.

The recognition fix cannot resolve either behavior: it changes only the command's
evidence class. This is an independently reproduced failure-reason-to-attribute
contract, not evidence that final acceptance still selects the unrelated API
file. The existing `contract_attribute` path mapping selects the page correctly.
Budgets, failure outcomes, and outer Recovery are unchanged.

## Follow-up acceptance proposal

Preserve bounded/redacted causal error information separately from display
snippets. Attribute primary/input/state only from an executed failing predicate
and its exact checked path; retain a generic failure when attribution is unknown.
Include multiple missing predicates, misleading command/comment/other-path
literals, swallowed errors, genuine Test regressions, and final repair prompt
content. Do not increase unbounded output retention or weaken command failure.

Codex2's second review agreed with this split and ownership tracing. It did not
run tests or a live campaign and did not certify the future diagnostic fix.
