# Loadmap2 Phase30 Plan

Date: 2026-06-23 JST

Status: completed / closed_excluded

## Scope

Phase30 closes `P20-COV-006` / KI-009 by making an explicit adoption
decision for the two remaining priority-decision rows.

| row | responsibility | Phase30 decision target |
| --- | --- | --- |
| C49 | Quality classification and confirmation. | Decide `adopt`, `partial-adopt with downstream phase`, or `exclude_with_rationale`. |
| C50 | Slash, plan, command UI helpers. | Decide `adopt`, `partial-adopt with downstream phase`, or `exclude_with_rationale`. |

This phase is a decision phase, not a broad implementation phase. Runtime code
changes are allowed only if the decision is `adopt` and the required proof is
small, deterministic, and inside the row boundary.

Implementation result: both rows are `excluded_with_rationale`. No runtime
code changes were required.

## Problem Statement

Phase21 through Phase29 converted most Anvil control-stack responsibilities
into CommandAgent contracts with row-level proof. C49 and C50 remain different:
they are not proven implementation gaps yet. They are priority decisions that
could either:

- represent useful deterministic CommandAgent contracts; or
- represent legacy advisory or UI surfaces that should stay outside the MVP.

Leaving them as `Missing` makes the migration ledger incomplete. Adopting them
without evidence would add unstable semantic classification or Anvil-specific UI
behavior that conflicts with CommandAgent's design.

## Decision Principles

Phase30 uses these rules:

- Adopt only behavior with a deterministic trigger, bounded effect, stable
  owner layer, and observable proof.
- Prefer exclusion for semantic quality scoring, secondary model
  confirmation, advisory feedback loops, or Anvil-specific UI rendering.
- Prefer CommandAgent-native CLI, REPL, and slash-command contracts over
  importing Anvil command helpers.
- If evidence shows a real recovery or eval gap, use `partial-adopt` and
  assign a narrower downstream row rather than folding a hidden subsystem into
  Phase30.
- Do not treat broad sign-off alone as proof for either row.

## Non-goals

- Do not add a semantic app-quality scorer.
- Do not add a model-powered quality confirmation loop.
- Do not import Anvil slash commands, plan-mode UI helpers, footer/spinner
  behavior, or message rendering helpers into the REPL by default.
- Do not add provider/model-specific behavior for quality classification.
- Do not rewrite eval expectations to hide model quality or UI limitations.
- Do not declare final migration completion; Phase32 owns that.

## Design Alignment

Phase30 follows the current recovery architecture:

```text
observed row responsibility
  -> source alignment
  -> deterministic adoption criteria
  -> decision record
  -> coverage row update
  -> proof or explicit exclusion rationale
```

The minimal loop remains the executor. Eval remains evidence collection and
reporting. Profiles remain domain facts. Slash commands remain CommandAgent
entrypoints, not a legacy UI compatibility layer.

## Row Strategy

| row | initial stance | adoption threshold | likely target |
| --- | --- | --- | --- |
| C49 | Default exclusion. | Adopt only if an existing deterministic guard needs a quality taxonomy that cannot be represented by current `verifier`, `profile_verification`, `setup`, `tool_protocol`, or `implementation_quality` categories. | Documentation and eval taxonomy first; runtime only if a deterministic failure classifier is missing. |
| C50 | Default exclusion. | Adopt only if CommandAgent has a concrete slash/plan command UX or eval gap that is not covered by existing parser/help/session behavior. | CommandAgent-native slash command docs/tests; no Anvil UI helper import. |

## Final Row Decisions

| row | disposition | rationale | proof |
| --- | --- | --- | --- |
| C49 | `excluded_with_rationale` | Existing CommandAgent eval/recovery taxonomy already gives deterministic attribution for verifier, profile, setup, tool protocol, and implementation-quality stops. Anvil's secondary quality confirmation and advisory feedback classification would add semantic scoring/model confirmation outside the minimal-loop design. | Coverage update; `git diff --check`; `python3 tests/test_eval_report.py`. |
| C50 | `excluded_with_rationale` | CommandAgent has native CLI/REPL slash parsing and command docs/tests. Anvil plan-mode UI helpers, footer/spinner/message rendering, and slash-command compatibility are UI compatibility surfaces, not recovery-parity requirements. | Coverage update; `git diff --check`; `cargo test slash_command --lib`. |

## Phase Outputs

Phase30 must produce:

- A row-level decision for C49 and C50.
- A source alignment matrix that separates adopted, omitted, and excluded
  behavior.
- A row closure matrix that records disposition, owner, and proof.
- A blocking ledger that shows why the row is closed or split forward.
- A reconciliation document mapping KI-009 and `P20-COV-006` to final row
  outcomes.
- Coverage and roadmap updates only after the decision is proven.

## Horizontal Expansion

The decision should avoid Next.js-only or local-model-only reasoning:

- C49 must be checked against current eval taxonomy for Rust, Python, Next.js,
  tool protocol, setup, and verifier failures.
- C50 must be checked against current CLI/REPL/slash command behavior and
  docs, not only `/ultra-plan-run`.
- Any adopted behavior must be provider-independent and profile-independent
  unless the row is split forward with a specific owner.

## Documentation Updates

If C49 or C50 is excluded:

- Update `docs/eval/legacy-control-stack-coverage-20260621.md` with
  `Excluded` plus design rationale.
- Update Phase30, `recovery_plan.md`, `current_issue_phase_map.md`, and the
  roadmap README after proof.
- Update `docs/known-limitations.md` only if the exclusion changes a documented
  product limit.

If either row is adopted or partial-adopted:

- Update the responsible architecture doc before runtime code changes.
- Add targeted tests or focused fixtures for the adopted deterministic surface.
- Assign any larger work to a narrower downstream phase before Phase32.

## Exit Gate

Phase30 is complete only when both C49 and C50 are one of:

- `excluded_with_rationale` with explicit design reason and coverage update;
- `closed_proven` with deterministic implementation proof; or
- `split_forward` with a narrower same-surface blocker, downstream phase,
  failed proof evidence, and owner.

Phase30 exit result: satisfied by `excluded_with_rationale` for C49 and C50.

## Plan Review Result

Self-review findings incorporated into this package:

- The phase is framed as a decision phase so it does not accidentally expand
  into a quality engine or slash-command compatibility port.
- The default exclusion stance is explicit, but adoption remains possible when
  deterministic evidence proves a real CommandAgent gap.
- The proof path separates coverage decision records from broad sign-off, so
  CI success or eval pass cannot close the rows by itself.
