# Phase30 Concrete Work Plan

Date: 2026-06-23 JST

Status: completed / closed_excluded

## Step 0: Preflight

1. Run `git status --short --untracked-files=all`.
2. Record unrelated dirty files and keep them out of Phase30 changes.
3. Re-read:
   - `workspace/mvp/logic/anvil/loadmap2/README.md`
   - `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
   - `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
   - `docs/eval/legacy-control-stack-coverage-20260621.md`
4. Confirm C49 and C50 are the only selected rows.

Exit criteria:

- Phase30 scope is limited to C49/C50.
- Phase31 and Phase32 responsibilities remain untouched.

Result: completed. The only pre-existing unrelated dirty file was
`workspace/mvp/logic/anvil/loadmap2/phase_21/implementation_report.md`, and it
was left untouched.

## Step 1: Establish Decision Criteria

1. Create a row decision template with:
   - source row;
   - Anvil source responsibility;
   - CommandAgent equivalent or absence;
   - deterministic trigger;
   - owner layer;
   - allowed action;
   - omitted behavior;
   - disposition;
   - proof command or proof artifact.
2. Apply the template to C49 and C50 in `row_closure_matrix.md`.
3. Confirm every possible disposition has a closure condition:
   `closed_proven`, `excluded_with_rationale`, or `split_forward`.

Exit criteria:

- The decision does not depend on broad sign-off alone.
- Any adoption must name an owner layer and deterministic proof.

Result: completed. Both rows use `excluded_with_rationale`; no adopted owner
layer or split-forward blocker was needed.

## Step 2: Evaluate C49 Quality Classification

1. Inspect current CommandAgent surfaces that already classify failures:
   - eval report categories;
   - `ContractEvidence` and recovery evidence fields;
   - verifier/profile/setup/tool protocol failures;
   - known limitations for app quality and model quality.
2. Compare them to Anvil C49 sources:
   `quality.rs`, `quality_confirm.rs`, `feedback_kind_confirm.rs`,
   `task_classification.rs`.
3. Determine whether CommandAgent lacks a deterministic row-level contract or
   merely excludes semantic quality confirmation by design.
4. Choose the row disposition:
   - Prefer `excluded_with_rationale` for semantic quality scoring and
     secondary confirmation.
   - Use `split_forward` only if a failed proof shows a specific deterministic
     classifier is needed.
   - Use `closed_proven` only if a small deterministic implementation and test
     can be completed inside Phase30.

Exit criteria:

- C49 has a row decision with cited CommandAgent surfaces and Anvil behavior.
- The decision does not hide missing contract evidence under model quality.

Result: completed. C49 is excluded because CommandAgent already reports
deterministic quality-related attribution through eval/recovery taxonomy while
Anvil semantic quality confirmation is advisory and outside the minimal-loop
contract.

## Step 3: Evaluate C50 Slash/Plan/Command UI Helpers

1. Inspect current CommandAgent surfaces:
   - `src/agent/slash_command.rs`;
   - `src/agent/repl.rs`;
   - `/plan-run` and `/ultra-plan-run` docs;
   - existing CLI/slash tests and eval command invocation paths.
2. Compare them to Anvil C50 sources:
   `slash_commands.rs`, `plan_sections.rs`, `plan_mode_helpers.rs`,
   `commands.rs`, `tool_display.rs`, `message_push.rs`, `footer.rs`.
3. Determine whether the missing Anvil behavior is:
   - a recovery-parity requirement;
   - a CommandAgent-native UX/test gap; or
   - legacy UI rendering that should remain excluded.
4. Choose the row disposition:
   - Prefer `excluded_with_rationale` for Anvil UI rendering helpers.
   - Use `split_forward` only if a concrete CLI/REPL/slash gap has failed
     proof.
   - Use `closed_proven` only for a narrow CommandAgent-native parser/help
     proof.

Exit criteria:

- C50 has a row decision with cited CommandAgent surfaces and omitted Anvil UI
  behavior.
- No Anvil slash command or UI helper is imported wholesale.

Result: completed. C50 is excluded because CommandAgent keeps native
CLI/REPL/slash parsing and does not need Anvil UI rendering/helper
compatibility for recovery parity.

## Step 4: Update Closure Artifacts

1. Update `source_alignment_matrix.md` with final adopted/omitted behavior.
2. Update `row_closure_matrix.md` with final dispositions and proof.
3. Update `blocking_ledger.md` so C49/C50 are closed, split, or explicitly
   excluded.
4. Update `reconciliation.md` to map KI-009 and `P20-COV-006` to row outcomes.
5. Update `focused_worklist.md`:
   - If both rows are excluded, record no model-facing focused eval required.
   - If adopted/split, record exact tests or focused fixtures.

Exit criteria:

- The phase-local package can be read without referring back to summary prose.
- Each row has owner, disposition, and proof.

Result: completed. The phase-local matrices, ledger, reconciliation, and
focused worklist all record the same `excluded_with_rationale` disposition.

## Step 5: Update Project Docs

1. Update `docs/eval/legacy-control-stack-coverage-20260621.md`.
2. Update `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`.
3. Update `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`.
4. Update `workspace/mvp/logic/anvil/loadmap2/README.md`.
5. Update other docs only if the final row decision changes product behavior:
   - `docs/known-limitations.md`;
   - `docs/evaluation.md`;
   - `docs/ultra-plan-run.md`;
   - `docs/architecture.md`.

Exit criteria:

- Coverage and roadmap docs agree on C49/C50 status.
- Phase30 does not declare final migration completion.

Result: completed. Coverage, issue map, recovery plan, and roadmap README now
agree that Phase30 is `closed_excluded`.

## Step 6: Verification

Run the smallest proof set matching the decision:

1. Always run `git diff --check`.
2. If only docs/coverage decisions changed, run the relevant docs/eval report
   tests, for example `python3 tests/test_eval_report.py` if report parsing is
   touched.
3. If C49 is adopted, run the targeted classifier tests and focused fixtures
   named in `focused_worklist.md`.
4. If C50 is adopted, run the targeted CLI/slash tests named in
   `focused_worklist.md`.
5. Run broad sign-off only as a regression check when runtime/eval behavior is
   changed; record it as supplementary proof, not the row proof.

Exit criteria:

- No row remains unresolved `Missing`.
- No unbounded repair, provider-specific policy, or legacy UI compatibility
  layer was introduced.

Result: completed with docs/test checks. Broad sign-off was not required
because no runtime/eval behavior changed.

## Step 7: Exit Review

1. Review the final diff against `docs/philosophy.md` and
   `docs/adr/0002-contract-recovery.md`.
2. Verify C49/C50 decisions are stable, deterministic, and attributable.
3. Verify split-forward, if any, is narrower than the original row and has
   failed proof evidence.
4. Write `implementation_report.md` summarizing:
   - final row decisions;
   - proof commands;
   - docs updated;
   - remaining blockers;
   - review result.

Exit criteria:

- Phase30 can be marked completed for its own rows without claiming Phase32
  final migration completion.

Result: completed. Final migration completion remains Phase32-owned.

## Plan Review Result

Review changes incorporated:

- The plan now starts from decision criteria before source comparison so C49
  and C50 are not accidentally implemented from source names alone.
- The plan treats broad sign-off as supplementary regression evidence.
- The plan includes explicit documentation and split-forward paths to prevent
  unresolved `Missing` rows from surviving Phase30.
