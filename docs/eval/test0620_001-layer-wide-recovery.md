# test0620_001 Layer-Wide Recovery Evidence

## Context

`workspace/mvp/logic/test0620_001/eval` asked for the same deterministic
failure-evidence direction to cover every layer that detects the current
problem class, without expanding retries, hidden continuation, or provider
specific policy.

The target layers are:

- planning: plan lint and profile obligation correction evidence;
- provider transport: shared response parser failures such as malformed XML
  fallback or JSON tool-call payloads;
- tool protocol: parsed tool calls that violate CommandAgent tool schemas;
- step policy: read-only step mutation and related execution-contract failures;
- profile: deterministic profile verification failures;
- verifier/setup: command failure, dependency missing, and one approved setup
  attempt diagnostic context;
- eval reporting: layer-oriented failure grouping for triage.

## Change Summary

- Plan correction prompts render a `Recovery task` derived from existing
  plan-lint evidence when the evidence is specific enough.
- Provider transport parse failures now emit `provider_transport` contract
  evidence in the step runner, using shared parser diagnostics only.
- Tool protocol and read-only step-policy evidence now include failure kind,
  diagnostic code, failure signature, and observed/expected facts.
- Step-policy evidence also covers setup steps that attempt to mutate source
  files and model-issued dependency setup commands. The setup/source case uses
  a setup/config-only execution envelope rather than broad file mutation.
- Verifier evidence includes an affected command and observed/expected pair in
  addition to command, diagnostic, candidate artifact, and repair target data.
- Next.js profile verification maps more failure codes to deterministic repair
  targets and required actions, including missing dependencies, script drift,
  Tailwind/PostCSS drift, TypeScript alias/root drift, and dev-port drift.
- `scripts/eval_report.py` groups failures by layer:
  `planning`, `provider_transport`, `tool_protocol`, `step_policy`, `profile`,
  `verifier`, `setup`, `quality`, `unknown`, and `ok`.

## Design Check

The change follows the current architecture:

- deterministic guard output is structured into bounded evidence;
- existing bounded correction or repair paths consume that evidence;
- original guard/verifier/profile checks remain authoritative;
- retry budgets and automatic continuation are unchanged;
- setup/source policy recovery preserves the setup/config mutation boundary;
- provider transport diagnostics do not introduce provider/model-specific
  behavioral policy;
- profiles still emit domain facts and repair targets, not workflow decisions.

## Verification

Local verification passed on this checkout:

- `cargo fmt --check`
- `cargo test recovery_task`
- `cargo test repair_loop`
- `cargo test profile_replan_packet`
- `cargo test runtime::prompts`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `python3 tests/test_eval_report.py`
- `python3 -m py_compile scripts/eval_report.py`
- `cargo build --release`

Focused live UAT was run with Gemini from clean temporary workspaces:

- Provider/model: `gemini` / `gemini-3.1-flash-lite`
- Planner model: `gemini-3.5-flash`
- Command:
  `target/release/commandagent --yes --context-budget 65536 --provider gemini --model gemini-3.1-flash-lite --planner-model gemini-3.5-flash "/ultra-plan-run --profile nextjs Create a Space Invaders style Next.js app that can run on port 3011."`

Observed runs:

- `/private/tmp/commandagent-test0620-layerwide-XMiEcN` exposed an uncovered
  step-policy class: setup step attempted to write `app/globals.css`, but the
  saved repair packet had `Contract evidence: none`. This caused the
  `setup_step_source_mutation` evidence and setup/config envelope change.
- `/private/tmp/commandagent-test0620-layerwide-K5SovG` still failed, but the
  first failure was attributed: `step_policy` /
  `setup_step_source_mutation`, target path `src/app/globals.css`, recovery
  task envelope `setup_config_mutation`, and disallowed source-route/component
  edits. The same run also produced a profile repair packet for
  `nextjs_route_not_integrated`, targeting `src/app/page.tsx` with
  `src/lib/gameEngine.ts` as the candidate artifact.

Result:

- The run did not complete and app quality was not evaluated.
- The target contract-boundary improvement was observed: the first failing
  layer and recovery task were explicit instead of generic prose.
- The remaining product issue is planning/profile decomposition quality: a
  step named as setup attempted to create source CSS. That is now detected and
  bounded, but not automatically replanned into a create/edit source step.
