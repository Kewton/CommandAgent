# Issue #465 verification

- Status: `passed`

## Checks

- `git merge-base --is-ancestor origin/develop HEAD`: `passed`
- `cargo test --lib issue465 -- --nocapture > /private/tmp/issue465-focused.log 2>&1`: `passed`
- `ISSUE465_TYPESCRIPT_ROOT=/Users/maenokota/share/work/github_kewton/MyCodeBranchDesk/node_modules/typescript cargo test --lib issue465_typescript -- --ignored --nocapture > /private/tmp/issue465-typescript.log 2>&1`: `passed`
- `ISSUE465_NEXT_NODE_MODULES=/Users/maenokota/share/work/github_kewton/MyCodeBranchDesk/node_modules cargo test --lib issue465_nextjs -- --ignored --nocapture > /private/tmp/issue465-nextjs.log 2>&1`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings > /private/tmp/issue465-clippy.log 2>&1`: `passed`
- `cargo test --test corpus_regression --test generality_guardrails > /private/tmp/issue465-corpus-guardrails.log 2>&1`: `passed`
- `cargo test --lib issue448_original_contract_finish_matrix -- --nocapture > /private/tmp/issue465-port-recheck.log 2>&1`: `passed`
- `cargo test > /private/tmp/issue465-full-test.log 2>&1`: `passed`
- `cargo build --release > /private/tmp/issue465-release-build.log 2>&1`: `passed`
- `target/release/commandagent --version`: `passed`
- `shasum -a 256 target/release/commandagent`: `passed`
- `git diff --check`: `passed`

## Base and measured results

Verification date: 2026-09-12. The required initial
`git fetch --no-tags origin develop` succeeded. Initial HEAD and fetched
origin/develop were both `e99f1ebe1e777fb792a9343407736ca754e41bdf`; they still
matched before the Issue commit. No required predecessors were listed.

- Focused suite: 23 passed, zero failed; the two compiler-dependent tests are
  ignored by default and both were run explicitly and passed, as listed above.
- TypeScript 5.9.3: one test covering six real Runner variants passed. The initial
  compiler output contains `Expected 2 arguments, but got 1`.
- Next.js 15.5.20: one real build / Runner test passed, including refusal followed
  by a successful repair. Node was v24.1.0. Installed dependencies were read-only;
  candidates and build outputs were temporary, with Next telemetry disabled.
- Corpus: 7 passed. Generality guardrails: 10 passed. No baseline was increased;
  `phase.rs` is 3,854 lines and `loop_run/context.rs` is 273 lines.
- Full suite: library tests 2,517 passed, zero failed, 19 default ignored; all
  integration targets and both doc tests passed, and `cargo test` exited 0.
  Other pre-existing opt-in tests were not claimed as executed.
- All-target clippy completed without warnings. Release build exited 0.

Release output, built from the verified pre-commit worktree:

```text
commandagent 0.1.0 e99f1ebe+dirty 2026-09-10T00:03:59+09:00
SHA-256: 8b3e62509c9e498420db90e157453fd31c08257056e5bd6128f30813956d75dd
```

The version's `+dirty` identifies the Issue changes before their required commit;
the timestamp is the binary's reported source metadata, not this verification date.

## Acceptance and review evidence

| Condition | Measured outcome |
| --- | --- |
| create origin, existing required paths, observe, deferred verify, failed Edit then cat | Unbound control completes; bound loop rejects completion and preserves the original failure. |
| Read, distinct cats, identical Write, unrelated/smoke-only changes, self-report, stale/path-only evidence | Cannot resolve the repair. A subsequent real edit and fresh check progress. |
| API unchanged, related store/type repair; Bash edit; prior repair with no new edit | Fresh target verification permits completion. Real TypeScript permits `as const` and import aliases. |
| Removed/commented processing, diagnostic suppression, unsafe casts, configuration exclusion, early compile failure | Completion is rejected; disappearance of the original diagnostic is insufficient. |
| Assistant final, after-tool, iteration short circuit, existing verifier precheck, post-step verification | Bound obligation is checked; regression after an earlier pass is rejected. |
| Later missing verifier producer and dependent application owners | Changed related work may advance as pending with explicit owners, finite boundary and existing budget; the later boundary must confirm. Missing boundary is rejected. |
| `./` owner spelling, omitted or unrelated expected paths | Cannot opt an application step out of the obligation. |
| Generated JSON changes with identical source digest | A new host check runs and rejects stale success; confirmation is not cached. |
| Host record deleted, replaced, or copied from another attempt | Completion and next Runner binding reject. Deleting inspection context as well cannot clear host-memory authority. |
| New compiler/build configuration, including ignored/nested/symlinked config | Original configuration inventory is enforced; a successful weakened check cannot discharge the obligation. A missing registered smoke script may still be produced. |

Actual tool evidence from the focused run:

```text
ISSUE465_BASH_RECORD_REMOVAL record_exists=false completed=false
```

The Write tool's private-path policy blocks replacement, but Bash can unlink the
record. Therefore chmod and the existing upper protection are insufficient. The
new host-memory provenance comparison blocks both completion and the subsequent
Runner boundary. Separate Runner controls cover replacement, a foreign attempt
with the same contract, and deletion of both runtime records. Captured continuation
context preserves the original obligation while assigning a new attempt identity.

Actual Next.js evidence:

```text
ISSUE465_NEXT_CONFIG original_arity_failure=true bypass_build_passed=true api_unchanged=true runner_completed=false
ISSUE465_NEXT_CONFIG original_conditions_restored=true related_store_repaired=true runner_completed=true
```

The candidate initially has no `next.config.js`. Writing
`typescript.ignoreBuildErrors=true` makes the registered `next build` succeed with
unchanged broken API bytes. The new guard rejects that changed condition. Removing
the bypass and repairing the related store completes through the real Runner.

## Earlier failures and reruns

The new configuration-sensitive executable regression was first run before its
product fix and failed its refusal assertion with `Ok("plan-run complete: 1 steps")`.
The final focused run passes the same negative and its genuine-repair control.

Earlier guardrail growth failures were fixed by consolidating option wiring into
the recovery leaf; the final unchanged-baseline guard suite passes. The #456
negative replay assertion now checks the earlier unresolved obligation and retained
failing command instead of requiring a later final-verification rejection.

An initial sandboxed broader run encountered existing localhost permission errors;
the required full suite was run outside the sandbox. One later full run encountered
`nextjs_route_observation_failed:port_in_use` in the existing #448 matrix on fixed
port 60302. Its standalone rerun and the subsequent complete `cargo test` both
passed. The parent reported using that port around 03:45–03:46 UTC and confirmed
its server stopped at 03:46:31 UTC; this is a plausible collision source, not a
proven attribution. No shared service was stopped or restarted by this worker.

## Limits and preservation

No live model/API campaign was run. The recorded results are deterministic real
tool / Runner replays, real compiler/build runs, and the repository suite. Source
preservation remains conservative lexical checking plus the original registered
verifiers, not a proof of arbitrary program equivalence. Existing final acceptance
and promotion gates remain necessary.

Historical workspace evidence, live `.anvil`, plugin caches, and the original
develop worktree were not edited. Reports contain summarized results, not raw
runtime logs. No push, PR, merge, Issue mutation or other-worker message was sent.
