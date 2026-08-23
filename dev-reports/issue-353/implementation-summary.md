# Issue #353 implementation summary

## Implemented

- Split the GUI Trial compose form into **実行プロバイダー** and
  **計画プロバイダー** selectors while retaining the existing executor
  selector test ID for compatibility.
- Removed the client-side rewrite that copied `provider` into
  `planner_provider`. Either provider now changes only its own `SessionSpec`
  field and invalidates the current Gate 1 proposal through the existing update
  path.
- Gave the executor and planner model inputs separate provider-scoped candidate
  fetches, datalists, and unknown-candidate checks. Local discovery now follows
  `provider` for the executor and `planner_provider` for the planner.
- Extended the provider-only browser smoke with opposite OpenAI/Gemini role
  pairs. It asserts the proposal/session request, Gate 1 card, frozen run
  identity, and delegated `--provider` / `--planner-provider` plus model flags.
- Extended the synthetic compose smoke to prove an Ollama executor list and an
  LM Studio planner list remain distinct, and that the request preserves an
  independently selected cloud executor.
- Applied the inspected Issue #352 smoke-path compatibility update so delegated
  arguments are read from the canonical `.commandagent/runs` record namespace.
- Documented the independent role-to-CLI mappings and role-scoped local model
  discovery in `docs/user/gui-trial.md`.

## Compatibility

No server API, event schema, Gate 1 identity schema, or CLI flag behavior was
changed. The implementation exposes the already-supported independent backend
fields without changing their wire names.
