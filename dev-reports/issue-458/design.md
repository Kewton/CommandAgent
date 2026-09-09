# Issue #458 design

Written before implementation. Base: `f2253cb7c55424ee9a98015230d65b9ef738aeec`.
Inspected verified predecessor commits `32f997f4` (#456) and `f2253cb7` (#457),
then fast-forwarded this dedicated branch. Read GitHub Issues #458 and #387,
the worktree AGENTS.md, worker skill, and development guardrails.

## Decision

Candidate rejection and control retention are transaction decisions. They do
not alone mean another automatic attempt is unsafe. Retry only diagnosed
implementation failures with a validated, distinct host-generated continuation,
or a failed registered post-execution observation with a newly diagnosed plan.
Never reuse treatment source as control. All #387 limits and safety stops remain.

| Finish / boundary condition | Retry decision and reason | Next plan, diagnosis and workspace |
| --- | --- | --- |
| Execution failure with a typed implementation/verification or local repair exhaustion handoff | Eligible only for explicitly recognized failure kinds, nonempty diagnosis, valid child YAML and safe control/treatment state | Validate child before rebuilding; keep latest diagnosis and original provenance, normalize targets to workspace-relative paths, bind original registered checks, discard treatment completed-artifact claims; save a new plan under control runtime plans |
| Read-only stagnation | Conditional, never by parsing an error string; require typed local-repair failure, valid next plan and all checks above | Same normalization and fresh control treatment; identical normalized plan stops as a cycle |
| Execution failure without a child, unknown failure kind, configuration/authentication/infrastructure failure | Stop `not_recoverable`; no evidence of a safe implementation continuation | Retain control and honest latest failure; no automatic next plan |
| Registered post-execution observation establishes a supported product failure | Eligible; execution success cannot promote a failing product | Build host continuation from original contract and latest failed observation, with stable post-verification scope; discard candidate deltas; repeated semantic failure stops as a cycle even if timing changes the diagnostic plan text |
| Next.js readiness startup/spawn/early exit, occupied port, timeout, environment conflict, skipped/unknown failure, or build failure without typed compiler diagnostics | Unavailable; failure to observe the application does not establish an implementation defect | Keep readable readiness reason; no continuation. Typed compile diagnostics or a matching actual HTTP failure response establish supported readiness failures |
| Verification unavailable, missing dependencies, unsupported observer or commands not configured | Stop; inability to verify is not an implementation diagnosis | Retain control; no continuation |
| Observations pass but completion acceptance is inconsistent | Stop; preserve acceptance gate | Retain control; no continuation |
| Missing transaction snapshot/config/treatment/observer or observer authority changed | Stop; safety state cannot be trusted | Retain/restore control where possible; no continuation |
| Control drift, protected-path treatment mutation, path escape, unsafe or redirected child YAML/targets | Stop even if control can be restored | Preserve original snapshot; no treatment source or permissions transferred |
| Promotion fails | Stop; filesystem/adoption failure is outside model repair | Existing restore/retention path; report restoration failure honestly |
| Execution, registered observation, completion acceptance and promotion succeed | Stop `recovery_succeeded` | Promote only verified treatment; final result contains this success, not older failures |
| Interrupted | Stop `interrupted`, including during treatment execution | Reject treatment and retain control; preserve interruption classification |
| Child YAML missing/corrupt/review-required or resume drift | Stop with existing candidate stop reason | Validate original child before reconstruction so rebuilding cannot erase a review or drift refusal |
| Normalized next plan already executed | Stop `cycle_detected` | UUID/path changes do not count as new plans; no next execution |
| Limit reached (0/1/2, maximum 20 unchanged) | Stop `limit_reached`; initial execution is excluded | At most N recovery starts; 0 keeps legacy fast path and emits no auto events |

Eligibility never guarantees execution: control preflight, contract binding,
YAML roundtrip/review, resume confinement/drift, normalized cycle detection,
fresh snapshot/treatment and inspection authority binding still run before
the next model execution. Rejected candidates remain historical evidence.
Diagnostics are data, not permission or executable commands. Registered
commands always come from the original control contract.

## Implementation shape

Add a leaf module under `src/planner/auto_recovery/` for retry classification,
safe continuation rebuilding and finish safety checks. Keep runner chokepoints
and their baselines unchanged. Preserve existing event names/fields; use existing
candidate stop codes for invalid continuations. Candidate rejection remains
visible before any next attempt start. Preserve interruption and attach current
failure context rather than rewriting it as success.

Preflight currently hard-codes checkpoint 0, which would collide on a second
attempt. Pass consumed count into preflight and allocate non-overlapping
checkpoint numbers (separate from treatment boundary numbers); never overwrite
existing snapshots or observations. Do not spend additional observation work
after the configured execution budget is exhausted.

### Review refinements

The execution allowlist is `implementation_compile_error`, `compile_error`,
`verification_failed`, `bounded_repair_exhausted`,
`compile_repair_no_source_change`, `verify_repair_progress_unchanged`,
`model_stagnation:read_only_loop`, and `model_stagnation:no_progress_recorded`.
Unknown or generic `phase_execute_error` candidates do not establish eligibility.

Post-observation cycles use a separate semantic failure identity as well as the
unchanged canonical plan comparison. Identity retains command names **and
semantic reasons/output**, compiler path/line/column **and diagnostic message
including error code**, and the individual profile failures. It normalizes the
known observation root and removes only `run_checked`'s numeric `elapsed_ms`
header before stdout. Raw readable diagnostics remain in events and the newly
saved plan. Distinct command/compiler/profile diagnoses remain distinct even
at the same command/location. Unstructured application output is not parsed as
clock metadata: arbitrary numbers and unknown text remain semantic data, and
the configured maximum still bounds such attempts. Unknown readiness outcomes
are explicitly unavailable, never mislabeled as identical-plan cycles.

The readiness classifier retains normalized typed compiler locations and
code/messages in both the readable next diagnosis and its identity. This
matters because the readiness error takes precedence over the later verifier
report: a fixed `build_verifier_failed` reason alone would otherwise give
different compiler failures the same canonical next plan. A focused production
classifier-to-continuation test checks root-only equivalence and independent
path/code/message changes in both the next plan and failure identity.

Continuation publication uses the additive `recovery_continuation_prepared`
event after control retention and validation. It carries the control-owned YAML
path and manual command so a limit/cycle stop selects the latest safe plan;
existing success projection clears old failure/handoff information. Canonical
`.env` targets receive the same denial as their requested spelling in both
control and treatment. Existing parent/canonical workspace confinement remains.

A no-observer transaction is refused by production startup before `finish`'s
defensive NotConfigured branch can be reached. Test both real preflight/startup
refusal and safe finish without a started transaction; do not fabricate observer
authority to claim that this unreachable defensive branch is a normal path.

## Verification plan

Deterministic tests delegate prepare/start/finish to RunnerRecoveryDriver and
inject only execution outcomes and candidate workspace effects. Cover execution
failure to second-attempt promotion, failed post-observation to retry, every
terminal finish category above, exact bounds, cycles, interruption, invalid child
YAML, protected paths, drift and unchanged control hashes. Verify events and final
completion projection. Add a data-driven recovery corpus fixture. Reuse existing
real model-protocol replay tests from #456 and safety regressions; no live-model
success rate is inferred from deterministic execution injection.

Run focused Issue #458 and recovery tests, corpus and guardrails, format, clippy,
full cargo test, release build/version, and diff checks. Report deterministic
control-flow results separately from live-model repair efficacy (unmeasured;
the later #452 evaluation owns it). Commit locally; parent owns push/PR/merge
and Issue lifecycle changes.

## Read-only evidence

Root: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`.
Read `workspace/tmp/0909/issue-451-452-investigation-summary.md`,
`issue-451-452-review-resolution.md`, `issue-451-452-rereview-criteria.md`,
`issue-451-452-review-r2/result.md`, and both
`dev-reports/issue-{451,452}/cause-analysis.md` there. The saved #452 campaign
under `workspace/tmp/0909/result/orchestrate-451452-resume-20260909/` shows one
rejected Recovery out of two allowed, with no product delta. Its event hash is
`f7a30626804db6072de3f0f332259cf40f6a36a2ab932d847e7e5d55b236fe38`.
The review passed documentation accuracy, not product repair or live efficacy.
No historical evidence, other worktree source, or live `.anvil/` is modified.
