# Issue #465 design

## Base and scope

- Fetched with `git fetch --no-tags origin develop` before implementation.
- Initial HEAD and fetched origin/develop: `e99f1ebe1e777fb792a9343407736ca754e41bdf`.
- Branch: `feature/issue-465-cli-recovery-api-cat`; initial worktree clean.
- No required predecessors. The current base includes the committed #456–#459 changes.
- Read the full Issue with `gh issue view 465 --json number,title,body,state,url`, the worker skill, and `docs/dev/dev-guardrails.md`.
- Read-only references: the original develop worktree's `workspace/tmp/0912/recovery-analysis-01/report.md` and `discussion-resolution.md`. Historical `9549f746` is a reproduction reference, not the product base.

## Design before implementation

The current loop still treats a distinct successful Bash command as execution
progress. With existing required paths, deferred contract verification and observe
enforcement, failed Edit followed by cat can complete an Implement step. Keep
general execution progress and the existing fix-only `requires_write` semantics;
introduce an independent host-owned Recovery repair obligation in a leaf module.

Bind the original diagnostics, target/import-related source snapshots, original
registered completion contract and attempt identity when the host binds Recovery
inspection. Bind responsible Implement steps and a finite confirmation boundary
from the executing StepPlan. Inspect and dedicated verification-artifact production
do not inherit an unconditional application-write requirement.

Every implementation completion route (assistant final, after-tool completion,
short circuit, Runner precheck and post-step verification) must consult the same
obligation. Path existence, tool success and model testimony cannot discharge it.
Use fresh host verification against the retained contract, with preservation checks
for diagnostic suppression, removed source/call surfaces and verifier/configuration
weakening. Preserve full failure evidence; a different early failure is still a
failure. Related store/type changes and already repaired sources can pass without
requiring a write to the diagnosed API.

For a related group with remaining responsible steps, a real related source change
may advance as explicitly pending, with owners, boundary and the existing finite
step/iteration budget recorded. It is never reported as repaired. At the group's
boundary require fresh host verification; retain the existing final verification,
acceptance and promotion gates. Do not require a whole-app build at every
intermediate step, enlarge budgets, or duplicate #466/#467.

## Verification plan

Add a deterministic Runner/loop corpus replay covering failed Edit then cat,
read variants, same-content Write, unrelated/smoke changes, suppression/removal,
early verifier failure, related edits (including Bash), fresh no-edit confirmation,
all completion exits, and bounded dependent steps. Demonstrate the old completed
route by running the unbound control against the same fixture. Run focused tests,
then fmt, all-target clippy, full cargo test, release build, version and SHA-256.
Save implementation and exact-command verification reports; commit only #465 paths
after all required checks pass. No push, PR, merge, Issue mutation, service operation,
historical evidence edit, live runtime edit or plugin-cache change.

## Refinements from implementation and interim review

- Include a later missing registered verifier producer in the confirmation group;
  require an explicit later Verify step carrying all original commands. The
  application step may advance only as pending after a related source change.
- An application owner cannot evade binding through `./` spelling, omitted
  expected paths or unrelated expected paths. Only a dedicated producer whose
  complete output set belongs to registered verification inputs is exempt.
- Retain diagnosed/imported call bindings and referenced/public exports instead
  of freezing every helper call count. Permit import aliases, const assertions
  and cleanup of unused helpers. Registered checks still decide success.
- Do not cache verification results. A source digest omits some runtime inputs;
  generated JSON can change while that digest remains identical.
- A read-only runtime file is not authority. Keep the exact attempt obligation in
  host memory, scoped to automatic Recovery execution, and compare the runtime
  record before and after confirmation and at each Runner binding. Missing,
  replaced and foreign-attempt records fail closed. Carry the original obligation
  in the host-captured continuation context, then bind a fresh attempt identity.
- The ordinary Write policy blocks the private path, but an actual Bash tool can
  unlink it. The explicit host-memory check is therefore necessary, not redundant.
- Preserve the existing final gates and fix-only safety behavior. Consolidate
  option binding into the recovery leaf to stay within unchanged growth limits.
- Preserve the configuration inventory as well as existing file contents. Before
  and after confirmation, reject newly introduced compiler/build/test configuration
  (including ignored or symlinked paths); absence is part of the original condition.
  This does not freeze the inventory of verifier scripts, so a registered missing
  smoke producer remains possible. Add an executable config-sensitive Runner
  negative and retain the producer / genuine-source-repair positive controls.

## PR #469 Linux acceptance follow-up design (before test edits)

The Linux log at `workspace/management/runs/20260912-recovery-product465-publication-01/acceptance-failed.log`
in the original develop worktree is read-only evidence (SHA-256
`cc351df6a4d98b43ffa47332ce6b28ae28892bf67f761978a6198c9cc9ee3b52`).
The old assertion scans every prompt/message for English `private` or `hidden`.
Hidden-path feedback is localized; on macOS the workspace's `/private/` prefix
can satisfy the assertion without proving the Write rejection.

Keep this follow-up on `b60ae3e63a0eb23cdf40cf52057b995bde08f8ca` in the Issue #465
worktree. A fresh `git fetch --no-tags origin develop` still resolves develop to
`e99f1ebe1e777fb792a9343407736ca754e41bdf`. No #466 changes are incorporated.
Replace the broad text scan with exact structured events from the real Runner:
the single attempted Write must produce hidden-path feedback for the exact host
record and a workspace-policy validation error, with no successful Write event.
Keep byte preservation and successful later repair assertions and require their
event order. This is a test-evidence correction; no production change is needed.
Run focused real-path tests, fmt/clippy, full tests and available Linux validation;
update reports and commit without publishing.
