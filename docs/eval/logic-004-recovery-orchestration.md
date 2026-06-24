# Logic 004 Recovery Orchestration

Date: 2026-06-20
Base commit: `a8a4121`
Working tree: dirty during implementation

## Change

This slice adds a bounded Recovery Orchestration layer between deterministic
failure evidence and Recovery Task Contract rendering.

Implemented surfaces:

- `ArtifactGraph` projection for observed contract paths, artifact lifecycle,
  setup manifests, source artifacts, and integration targets.
- Recovery orchestration decisions for active job, repair action, admitted
  target, target priority, tool policy projection, explicit stop reason, and
  artifact graph summary.
- Contract evidence and recovery task rendering for the new orchestration
  fields.
- Plan-lint evidence for inspecting future artifacts that do not yet exist.
- Eval report contract-layer summaries.

## Design Check

The implementation keeps the minimal loop as the execution engine. It does not
add hidden retry loops, provider/model-specific policy, package-manager
execution from profiles, or profile-owned workflow control. Orchestration
selects and explains the bounded repair path; the original guard, verifier, or
profile check remains authoritative.

## Verification

Local checks:

```text
cargo fmt --check: pass
cargo clippy --all-targets -- -D warnings: pass
cargo test: pass
cargo build --release: pass
```

Focused local LLM eval:

```text
eval/runs/logic-004-smoke/20260620T232758
success: 2/3
failure categories: ok=2, quality=1
contract layers: ok=2, eval_success_contract=1
remaining failure: smoke-docs-readme missed required README.md content signal `usage`
```

```text
eval/runs/logic-004-large/20260620T232838
success: 0/6
failure categories: planning=1, profile=1, quality=1, verifier=3
contract layers: planning_contract=1, profile_contract=1, verification_contract=3, eval_success_contract=1
```

The large run still fails all large cases, but the report now separates the
actionable boundary. The Next.js modify case produced profile verification
evidence with `active_job=route_integration_repair`,
`repair_action=connect_artifact_to_selected_route`,
`tool_policy_projection=file_mutation_repair`, `target_admission`,
`target_priority`, and `artifact_graph_summary`.

Eval metadata dry-run:

```text
eval/runs/logic-004-dry-run/20260620T234059
summary.tsv columns include failure_category and contract_layer
meta.json includes failure_category and contract_layer
```

Gemini focused E2E:

```text
workspace: /private/tmp/commandagent-logic004-gemini
provider/model: gemini / gemini-3.1-flash-lite
planner model: gemini-3.5-flash
command: /ultra-plan-run --profile nextjs --style default --artifact package.json --artifact app/page.tsx ...
result: pass, exit 0
generated: package.json, app/layout.tsx, app/page.tsx, tailwind.config.js, postcss.config.js, next-env.d.ts
setup: one bounded npm install --include=dev attempt recorded under .commandagent/setup/
build: next build completed during phase verification
repair packets: none
```

The generated UI was a simple counter/dashboard rather than a high-quality app.
That is an output-quality limitation, not a recovery-orchestration failure in
this slice.
