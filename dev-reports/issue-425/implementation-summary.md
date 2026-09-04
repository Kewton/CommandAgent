# Issue #425 Implementation Summary

## Outcome

Automatic Recovery can now start after a failed Next.js or generic create run
whose host-generated run-level completion contract initially has no
`verify_commands`. The Recovery contract-authority boundary registers the
failed plan's already-normalized handoff commands in that generated contract,
validates the completed contract, persists it, and binds subsequent Recovery
checks to the resulting contract.

## Implementation

- Passed the failed candidate's typed verification-command handoff into the
  Recovery contract binder before Recovery preflight.
- Limited generated-contract completion to the host-generated
  `completion-contract-ultra-plan-run.json`, an empty command list, and the
  `nextjs` or `generic` profile. Explicitly configured contracts and generated
  data-profile contracts remain unchanged.
- Reused `CompletionContract::validate` before persistence, so normalization,
  deduplication, and command-safety checks remain authoritative.
- Added the additive
  `recovery_generated_completion_contract_completed` provenance event and the
  additive `registered_verify_commands_from_failed_plan` binding field.
- Added the additive `recovery_plan_auto_run_stop_summary` field and included a
  readable explanation alongside the stable machine stop code in candidate
  binding errors. Existing event names and stop-reason fields are unchanged.

## Tests and compatibility

- Added unit coverage for generated Next.js and generic contract completion,
  the first Recovery execution after a `core-implementation` build failure,
  readable stop summaries, configured-contract immutability, and data-profile
  non-augmentation.
- Added the Issue #425 corpus fixture for command provenance, first Recovery
  execution, stable stop reason, and readable stop summary.
- Preserved the Issue `4962f472` rule for configured and data-profile Recovery:
  only commands already registered in their completion contracts are usable.
- Kept all existing event names and fields backward compatible; the event
  contract changes are field additions plus one new provenance event.
