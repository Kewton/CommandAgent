# Issue #472 verification

- Status: `passed`

## Checks

- `git fetch origin develop`: `passed`
- `git rev-parse HEAD origin/develop`: `passed`
- `cargo test --offline --test missing_read_recovery --test missing_read_paths --test missing_read_limits --test missing_read_inspection`: `passed`
- `cargo test --offline --test generality_guardrails`: `passed`
- `cargo test --offline --lib tools::read_missing -- --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --offline --all-targets -- -D warnings`: `passed`
- `cargo test --offline`: `passed`
- `cargo build --offline --release`: `passed`
- `target/release/commandagent --version`: `passed`
- `git diff --check`: `passed`

## Final candidate

The parent implemented this candidate from fetched origin/develop
`43c6bdf2cdcd592ef994aa99de4bcc96a82d0f5a` and discussed/reviewed it with Codex2
through cmate-delegate. Both local parent and remote develop resolved to that
SHA before publication preparation. The separate develop checkout's unrelated
changes were preserved. This report does not claim a separately dispatched
implementation worker completed the work.

The final full run completed at 2026-09-12T10:23:00Z with exit 0: 2,544 library
tests passed, 19 existing library tests ignored, all 66 integration targets
completed successfully, and both compile-fail doctests passed. Default ignored
integration tests were not run or counted as passed. The new four integration
test modules contain 18 passing tests. Five Read failure/race unit tests and
the target-counter resolution test also run in the full suite. Generality
guardrails, corpus and existing Recovery contracts passed in that same run.

Final fmt and all-target Clippy passed before that full run. No new ignored
tests, growth-baseline increases, policy exceptions or acceptance exemptions
were added. Verification-evidence.json binds the 20 production/test/fixture
files and local raw-log/review artifacts by SHA256. Only these reports and
their compact provenance are committed; raw logs remain in the requested
local investigation directory.

The release build completed at 2026-09-12T10:24:58Z and version execution exited
0. It was built before the candidate commit, from the verified modified tree:

```text
commandagent 0.1.0 43c6bdf2+dirty 2026-09-12T14:18:03+09:00
SHA256 68a942c30d67c8bd1f5ce28311206f03bc0997a5499dd3d4cf230bd8e39055fc
```

The dirty marker and embedded timestamp identify precommit source metadata;
they are not the build completion time. This binary has not been installed
into the shared GUI runtime by these verification steps. Integration CI/UAT
and the merged release are later gates.

## Reproduction, review and corrections

Before production edits, a real minimal-loop/Read replay at the same base
failed with `path does not exist: src/lib/persistence.cjs`, before the scripted
model could Write. After the change, it receives a failed Read result, writes
the implementation and passes the original registered Node verifier. Runner
tests also carry missing-file diagnosis through Inspect to Implement, reject
Inspect writes, and reject wrong/empty implementations and unfulfilled done
claims while preserving contract and verifier bytes.

Actual unprivileged EACCES was exercised: the test requires OS PermissionDenied
and does not treat chmod alone as proof. Missing parents, root disappearance
or replacement, non-directories, symlinks, protected inputs and suffix fallback
are covered. A test-only one-shot seam deletes the actual selected fallback
inside the registry; its error remains nonrecoverable rather than being
misattributed to the original missing path.

Codex2 round 4 requested two changes: move path feedback below the existing
growth limit and exercise suffix fallback through the actual registry. Both
were implemented, and round 5 found them resolved. At that review's observation
time, full verification was unfinished; the parent separately checked its
later exit 0 and unchanged source hashes. The unverified short identifier in
round 4's closing text is not used as a code revision; the exact base and file
hashes above are authoritative.

Earlier candidate-verification records are retained. One initial added test
incorrectly assumed max_iterations=8 was the total provider-call cap. The
existing completion-contract artifact-recovery allowance was inspected, and
the test now strictly requires 12 calls and the existing three-attempt
artifact_recovery_exhausted stop. Product budgets were unchanged. A Clippy
expression simplification preserved semantics. An earlier full run correctly
caught tool_feedback's growth violation before the leaf extraction.

Sandboxed full tests could not create local listeners and reported Operation
not permitted; the owned attempt was interrupted and retained as incomplete.
The first outside-sandbox run then exposed the existing Python reference's
missing PyYAML. Reusing the installed Python 3.12.3/PyYAML 6.0.3 through the
child PATH resolved that failure without package installation. The final full
run used that environment outside the sandbox. Shared services were not
restarted or reconfigured.

## Limits

New recoverability is limited to proven ordinary relative targets on Unix.
Absolute/salvaged and workspace-symlink missing paths retain prior errors;
successful allowed reads retain prior behavior. Failure rechecks do not make
Read/open atomic or prove all TOCTOU safety. The dedicated counter lives for
one run_session; outer budgets are unchanged.

This verification does not establish automatic-Recovery promotion, generated
business UI quality, application persistence or live-model R0 success. New
R0 and comparison campaigns remain separate #452/#468 gates. #451/#460
external Browser blockers, provider messages and Issue closure are excluded.
