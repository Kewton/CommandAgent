# Phase30 Implementation Tasks

Date: 2026-06-23 JST

Status: completed / closed_excluded

## Phase Admission

- [x] Confirm `P20-COV-006` / KI-009 is still open and assigned to Phase30.
- [x] Confirm C49 and C50 are the only selected coverage rows.
- [x] Record unrelated dirty files before implementation and keep them out of
  Phase30 commits.
- [x] Confirm Phase30 is not responsible for Phase31 external timeout proof or
  Phase32 final migration declaration.

## Source Alignment

- [x] Reconcile C49 against the Anvil source set:
  `quality.rs`, `quality_confirm.rs`, `feedback_kind_confirm.rs`, and
  `task_classification.rs`.
- [x] Reconcile C50 against the Anvil source set:
  `slash_commands.rs`, `plan_sections.rs`, `plan_mode_helpers.rs`,
  `commands.rs`, `tool_display.rs`, `message_push.rs`, `footer.rs`.
- [x] For each row, record adopted behavior, intentionally omitted behavior,
  CommandAgent target module, proof method, and split/exclusion condition.
- [x] Update `source_alignment_matrix.md` if inspection discovers a narrower
  responsibility that must be split forward.

## C49 Quality Classification Decision

- [x] Inventory current CommandAgent quality-related surfaces:
  eval classification, verifier failure kinds, profile verification failures,
  setup failure kinds, implementation-quality labels, known limitations, and
  final-answer/prose-only guards.
- [x] Decide whether any deterministic recovery/eval gap requires a new C49
  contract.
- [x] If no deterministic gap is found, mark C49
  `excluded_with_rationale` and explain why semantic quality confirmation
  remains out of scope.
- [x] If a gap is found, choose one of:
  - `closed_proven` with a small deterministic classifier and tests;
  - `split_forward` with the exact narrower quality row, owner, failed proof,
    and downstream phase.
- [x] Do not use model-quality as a blanket explanation for missing contract
  evidence.

## C50 Slash/Plan/Command UI Helper Decision

- [x] Inventory current CommandAgent CLI/REPL/slash surfaces:
  `src/agent/slash_command.rs`, `src/agent/repl.rs`, command docs,
  `/plan-run`, `/ultra-plan-run`, help behavior, and session output.
- [x] Decide whether any recovery-parity or eval gap requires adopting Anvil
  UI helper behavior.
- [x] If no deterministic gap is found, mark C50
  `excluded_with_rationale` and explain why Anvil UI helpers remain outside
  migration scope.
- [x] If a gap is found, choose one of:
  - `closed_proven` with CommandAgent-native parser/help/report tests;
  - `split_forward` with a narrower CLI/REPL/slash responsibility, owner,
    failed proof, and downstream phase.
- [x] Do not import Anvil slash commands or plan-mode UI helpers wholesale.

## Coverage And Roadmap Updates

- [x] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only after
  C49 and C50 have final Phase30 dispositions.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
  KI-009 after the row dispositions are proven.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md` Phase30
  exit gate after proof.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/README.md` Phase30 status
  after proof.
- [x] Add `implementation_report.md` at closure time.

## Tests And Proof

- [x] If both rows are excluded, run documentation/report checks only:
  `git diff --check` and the smallest relevant docs/eval report tests.
- [x] If C49 is adopted, add targeted tests for the deterministic quality
  classifier or focused fixtures for the new eval fields.
- [x] If C50 is adopted, add targeted CLI/slash parser/help tests.
- [x] If either row is split forward, include failed proof evidence and the
  downstream phase in `blocking_ledger.md` and `row_closure_matrix.md`.
- [x] Run broad sign-off only as a regression check; do not use it as the sole
  row proof.

## Review Gate

- [x] Verify the final decision does not conflict with `docs/philosophy.md`,
  `docs/architecture.md`, or `docs/adr/0002-contract-recovery.md`.
- [x] Verify provider/model-specific behavior was not introduced.
- [x] Verify no hidden retry loop, semantic advisory loop, or workflow engine
  was added.
- [x] Verify the row status and issue map do not leave C49/C50 as unresolved
  `Missing`.

## Implementation Result

- C49: `excluded_with_rationale`. CommandAgent keeps deterministic eval/report
  quality attribution but excludes Anvil semantic quality confirmation and
  advisory feedback classification.
- C50: `excluded_with_rationale`. CommandAgent keeps native CLI/REPL slash
  command behavior but excludes Anvil UI helper/rendering compatibility.
- No runtime code changes, focused fixtures, or downstream splits were needed.

## Plan Review Result

Review updates applied:

- Added explicit adoption thresholds for both rows to prevent accidental
  feature creep.
- Added split-forward rules so real gaps can move forward without blocking the
  decision phase.
- Added proof rules that distinguish row proof from broad sign-off.
