# Issue #467 verification

- Status: `passed`

## Checks

- `git fetch --no-tags origin develop`: `passed`
- `git merge-base --is-ancestor origin/develop HEAD`: `passed`
- `git merge-base --is-ancestor 6c49222db4f282b75070d923479e6d41209b132a HEAD`: `passed`
- `git merge-base --is-ancestor 612ec3574716663161c707f17e2be17ea7a03c8c HEAD`: `passed`
- `cargo test --lib issue467_t3_pid_causal_control -- --nocapture > /private/tmp/issue467-baseline.log 2>&1`: `passed`
- `cargo test --lib issue467 -- --nocapture > /private/tmp/issue467-focused-final.log 2>&1`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings > /private/tmp/issue467-clippy-final.log 2>&1`: `passed`
- `cargo test --lib recovery_ -- --nocapture > /private/tmp/issue467-recovery-final.log 2>&1`: `passed`
- `cargo test > /private/tmp/issue467-full-test-final.log 2>&1`: `passed`
- `cargo build --release > /private/tmp/issue467-release-build.log 2>&1`: `passed`
- `target/release/commandagent --version`: `passed`
- `shasum -a 256 target/release/commandagent`: `passed`
- `python3 /private/tmp/issue467-check-preservation.py`: `passed`
- `git diff --check`: `passed`

## Final-code results

Verified on 2026-09-12, after both parent reviews: pre-copy restoration-source
validation and observation ID correlation are included. Focused suite: 9 passed,
zero failed or ignored, including the 12-case real preflight/control/restore
matrix and distinct equal-hash observations at attempts 0/64/128. Invalid
snapshot sources leave post-observer control bytes untouched and report
restore_invoked=false; a separate actual copying failure reports true/false for
invocation/success. Successful restoration matches the original checkpoint.

Final all-target clippy completed without warnings. Final Recovery regression:
202 passed, zero failed, 2 existing ignored. It completed before starting the
final full suite. Final full cargo test exited 0: 2,538 library tests passed,
19 existing library tests ignored; all integration targets exited successfully
and both compile-fail doctests passed. Corpus (7), generality guardrails (10),
and protection coverage audit (2) all passed within that final full run. Other
default ignored integration tests were not executed or claimed as passed.
No new ignored tests, guardrail baseline changes, or audit exemptions were added.

Release build ran only after final full verification completed and exited 0.
The binary was built from the verified precommit Issue worktree:

```text
commandagent 0.1.0 612ec357+dirty 2026-09-12T13:40:44+09:00
SHA-256: bd0e2b44c1a8192ffc24863c3c6013fc17ae0f164cae97c2a6d5bca32cc21faa
```

The dirty marker records the required precommit product changes; the version
timestamp is embedded source metadata, not the verification time.

## Causal evidence and preservation

Before modifying the recognizer, its real Source/ writer_paths entry produced:

```text
actual: paths=[] process_unsafe=true
pid_literal: paths=["data/inquiries.json"] process_unsafe=false
```

The only input change was replacing `${process.pid}` with `123` in the unchanged
T3 store (SHA-256 `522e6f0190e06a0b9d3edf37a0aa0991d7c469837d0a7ac280bc19c40f720567`).
After the fix both inputs register exactly that output. Public policy entry
tests also exercise the actual writer and executable minimum, existing/missing
JSON and the negative filesystem/binding/configuration matrix. The corpus
records these inputs and hashes. All nine pre-fix recognizer files match the
historical repository's `9549f746` versions; that revision was not used as the
product base.

Reused the parent's already completed isolated build/start/GET/POST replay,
per explicit steering. Its GET `/api/inquiries` first created the exact
2,380-byte historical JSON hash. This is a new stage measurement, not evidence
of the historical first-write stage. Independently rechecked all 48 original,
boundary and observation source hashes, central event bytes, historical JSON,
original control JSON absence, and the parent's replay record hashes. All match.
The checked summaries and source attribution are in the new corpus's
recognition-and-stage.json; existing evidence remains immutable.

Historical first write remains unknown. Historical interaction_success=false
and persistence_not_evaluated:no_mutation_observed are retained. The parent's
new API POST success did not evaluate GUI business behavior or restart
persistence. The deterministic worker tests preserve those separate failure
gates; JSON permission does not promise Recovery initiation, adoption or final
T3 success. No live-model campaign or product UAT success is claimed here.

## Earlier attempts and baseline

The initial fetch required sandbox escalation to update the shared Git
worktree metadata. Fetched origin/develop and initial HEAD both resolved to
e99f1ebe1e777fb792a9343407736ca754e41bdf. Inspected the committed passed #465/#466
reports and changes, then fast-forwarded without conflict to
612ec3574716663161c707f17e2be17ea7a03c8c, containing the #465 CI correction
6c49222db4f282b75070d923479e6d41209b132a. Ancestor checks passed before editing
and again at final verification.

The early sandboxed Recovery run encountered localhost PermissionDenied. Its
own execution session was interrupted (exit 130); its old log ends at a
long-running test and is not evidence of a currently running process. The
outside-sandbox rerun passed. An earlier full run also passed before the final
observation ID addition; it is not substituted for final verification. Logs
ending in -final listed above identify the final-code results. Final broader
Recovery and full executions were serial; no other process or shared service
was stopped, restarted, or used to resolve the sandbox restriction.

The raw logs, temporary preservation checker and release binary stay outside
the commit. Original develop edits, historical evidence, live .anvil and caches
were preserved. No push, PR, external merge, Issue mutation or other-worker
message was sent.
