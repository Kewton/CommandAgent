# Issue #465 verification

- Status: `passed`

The latest test-only PR #469 follow-up is recorded at the end of this report.
The original implementation/release measurements below are retained separately.

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

## PR #469 Linux acceptance follow-up

Parent commit: `b60ae3e63a0eb23cdf40cf52057b995bde08f8ca`. A fresh required fetch
still resolved origin/develop to `e99f1ebe1e777fb792a9343407736ca754e41bdf`.
Only the existing Write-rejection test and these Issue reports changed. There
are no production, corpus, dependency-lock, or #466 changes.

### Checks

- `cargo test --lib issue465_tool_write_cannot_replace_host_record_and_repair_can_still_proceed -- --nocapture > /private/tmp/issue465-ci-focused.log 2>&1`: `passed`
- `cargo test --lib issue465 -- --nocapture > /private/tmp/issue465-ci-replay.log 2>&1`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings > /private/tmp/issue465-ci-clippy.log 2>&1`: `passed`
- `cargo test > /private/tmp/issue465-ci-full-test.log 2>&1`: `passed`
- `docker run --rm --network none --mount type=bind,src=/Users/maenokota/share/work/github_kewton/CommandAgent-issue-465-cli-recovery-api-cat,dst=/work,readonly --mount type=bind,src=/private/tmp/issue465-linux-ci,dst=/verification --workdir /work --env CARGO_HOME=/verification/cargo --env CARGO_TARGET_DIR=/verification/target --env CARGO_BUILD_JOBS=4 --entrypoint sh commandagent-issue-28-devcontainer-final:latest -c 'cargo test --offline --lib issue465 -- --nocapture' > /private/tmp/issue465-ci-linux.log 2>&1`: `passed`
- `shasum -a 256 /Users/maenokota/share/work/github_kewton/CommandAgent-develop/workspace/management/runs/20260912-recovery-product465-publication-01/acceptance-failed.log`: `passed`
- `git merge-base --is-ancestor origin/develop HEAD`: `passed`
- `git diff --check`: `passed`

The specific native replay passed. Both macOS and Linux focused suites passed
23 tests with zero failures and two opt-in compiler tests ignored. Those two
compiler tests were not rerun for this test-only correction; their original real
TypeScript/Next.js measurements remain above. Native full verification passed
2,517 library tests (19 default ignored), every integration target including
corpus and guardrails, and both doc tests, with process exit 0. Fmt and all-target
clippy passed. Production is unchanged, so no new release artifact is claimed.

Linux focused verification used the existing `linux/arm64` development image
`sha256:1a6975b3f496ef49fab46a8723fc9f37ec54312fb96512a4a0663e1cce13ebc9`,
with Cargo 1.97.1 and Node v24.18.0. The workspace was mounted read-only at `/work`,
the test candidates used Linux temporary directories, and compilation/testing ran
without network access. This is local Linux focused verification, not a claim
that the GitHub Ubuntu acceptance job or its Python/shell stages were rerun.

The first offline Linux attempt could not compile because the copied native Cargo
cache lacked `linux-raw-sys` 0.4.15 and 0.12.1. A separate temporary container ran
`cargo fetch --locked --target aarch64-unknown-linux-gnu` into the dedicated
`/private/tmp/issue465-linux-ci/cargo` cache. The subsequent offline test command
listed above passed. The original Cargo cache was only mounted read-only for its
copy, and no shared service was changed.

The preserved CI log failed only at the broad English keyword assertion. Its
preceding successful-Runner and identical-record-byte assertions passed. The
replacement now verifies this exact event sequence from the real Runner:

1. One `tool_call_raw` event for Write.
2. One `hidden_path_feedback` event for Write, the exact
   `.commandagent/recovery-runtime/repair-obligation.json` path, and attempt 1.
3. `tool_validation_error` for Write with
   `error_kind=workspace_policy_blocked` and repeat count 1; no successful Write.
4. Unresolved repair, then successful Edit, then fresh resolved-repair evidence.

The same replay still requires unchanged host-record bytes and successful later
repair. No prompt language or absolute temporary-directory spelling contributes
to these assertions. The failure log's SHA-256 was identical before and after:
`cc351df6a4d98b43ffa47332ce6b28ae28892bf67f761978a6198c9cc9ee3b52`.
