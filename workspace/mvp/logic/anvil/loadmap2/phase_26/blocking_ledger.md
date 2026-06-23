# Phase26 Blocking Ledger

Date: 2026-06-23 JST

| blocker id | coverage id | owner layer | incomplete contract | suspected module family | downstream task | proof command / case | closure condition | status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P26-C13-001 | C13 | recovery task / repair packet | closed: safe-stop payload now proves evidence/completion/setup/profile/semantic/action-envelope failure families. | `recovery_task`, `repair`, `repair_job`, eval scripts | Add common safe-stop payload fields and focused safe-stop fixtures. | `cargo test recovery_task`; focused root `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340` | Safe-stop payload renders owner/job/action/target/cluster/attempt/required/disallowed/rerun facts for every Phase26 family. | closed_proven |
| P26-C13-002 | C13 | final error / packet rendering | closed: repair packet carries actionable recovery context and blocks prose-only success claims. | `recovery_task`, `repair` | Render repair packet/failure packet context before minimal-loop repair. | recovery-task tests; eval report tests | Packet rendering is structured, bounded, and never claims success without evidence. | closed_proven |
| P26-C14-001 | C14 | setup lifecycle | closed: setup candidate validation and setup result ledger are represented. | `setup_lifecycle`, `setup_artifact_validation`, `runtime/setup` | Add setup manifest/readiness/authority/result/fingerprint/stale facts. | setup lifecycle/setup validation tests | Setup lifecycle records readiness, command authority, attempt key, fingerprint, stale reason, setup result, and failure signature. | closed_proven |
| P26-C14-002 | C14 | setup policy | closed: Rust/Python deterministic setup blockers are explicit setup facts. | verifier dependency classification, setup validation | Add non-Node setup policy from deterministic verifier evidence. | setup focused matrix | Rust/Python deterministic dependency/toolchain blockers produce setup facts without implicit execution. | closed_proven |
| P26-C15-001 | C15 | profile output | closed: common profile/project/scaffold facts are rendered through shared schema. | `profiles`, `profile_artifact`, `artifact_graph` | Expand common profile output schema. | profile output tests | Profiles expose root hints, manifests, entrypoints, setup/scaffold/integration artifacts, protected paths, verifiers, and behavior obligations. | closed_proven |
| P26-C15-002 | C15 | scaffold evidence | closed: scaffold artifacts and completion evidence are exposed as artifact-contract facts. | profiles, artifact completion | Add scaffold artifact contract and completion evidence. | scaffold focused fixture | Scaffold facts are evidence-bound artifact contracts, not hidden workflow mutation. | closed_proven |
| P26-C16-001 | C16 | profile failure mapping | closed: profile failures map to typed recovery job/action/target facts. | `profiles`, `recovery_policy`, `recovery_orchestration` | Map route/manifest/setup/source/scaffold/explicit-stop failures. | profile mapping tests; focused profile-failure matrix | Profile failures produce typed facts consumed by dispatch and do not select final workflow behavior. | closed_proven |
| P26-C17-001 | C17 | semantic failure report | closed: conflict inputs, observed/expected, affected cases, candidate artifacts, and ranking inputs are visible. | `semantic_failure`, `verifier_diagnostic`, `recovery_contract` | Add conflict inputs, observed/expected, affected cases, candidate artifacts, ranking inputs. | semantic-failure tests; verifier fixture | Semantic reports are data-only and expose unknown diagnostics instead of hiding them. | closed_proven |
| P26-C18-001 | C18 | semantic repair plan | closed: selected cluster, role, hypothesis, expected delta, success check, and exhaustion state are rendered. | `semantic_failure`, `recovery_task`, `repair_job`, `repair_brief` | Render selected cluster, role, hypothesis, expected delta, success check, exhausted state. | recovery-task/repair-state tests; semantic repair fixture | Repair task/brief has enough semantic context without adding retry expansion. | closed_proven |
| P26-C19-001 | C19 | repair brief | closed: brief renders root cause, target, constraints, allowed/disallowed actions, confidence, preservation, and success check. | `repair_brief`, `recovery_task` | Expand repair brief rendering and tests. | `cargo test repair_brief`; focused repair brief fixture | Brief consumes selected dispatch/action facts and does not recompute owner/action from prose. | closed_proven |
| P26-C20-001 | C20 | action envelope | closed: action-family lifecycle admission/rejection is represented before prompt rendering. | `repair_action_plan`, `recovery_orchestration`, `recovery_policy`, `recovery_task` | Add action-envelope lifecycle/status and action-family tests. | repair-action-plan/action-envelope tests; focused action-envelope matrix | Every selected action family is admitted or rejected before prompt rendering with structured evidence. | closed_proven |

## Review Result

Review findings applied:

- Split blockers by coverage row and responsible layer.
- Added separate setup-policy and scaffold-evidence blockers because those
  are common sources of false closure.
- Kept conflict resolution and target/verifier lifecycle out of Phase26 while
  allowing Phase26 to emit handoff evidence.
- No blocker uses provider throughput, model quality, CI success, or broad
  sign-off as a row-level closure condition.
