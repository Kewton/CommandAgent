# Loadmap2 Phase 9 Repair Action Envelope

Date: 2026-06-22

Phase 9 adds a deterministic repair-action admission gate after active-job
dispatch, target admission, and semantic repair planning. The gate checks the
selected job/action pair, projected tool category, source-of-truth authority,
target role, and allowed change kind before rendering a repair prompt.

New evidence fields:

- `allowed_tool_category`
- `repair_root_cause`
- `repair_hypothesis`
- `expected_improvement`
- `target_confidence`
- `must_preserve`
- `disallowed_actions`
- `success_check`
- `repair_plan_rejection_reason`

If the action envelope is rejected, the repair loop stops before asking the
model for another repair turn. This keeps recovery bounded and observable; it
does not add hidden retry or provider-specific behavior.

Focused checks added:

- incompatible job/action is rejected by the action envelope
- verifier-contract authority cannot select artifact mutation
- rejected action envelopes are terminal before a repair turn
- eval TSV/reporting keeps repair envelope fields

