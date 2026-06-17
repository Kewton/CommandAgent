# Local Recovery Large Eval: 20260617T120336

Date: 2026-06-17

## Scope

This report is the LR-4 focused rerun from `.workspace/CommandAgent/local-recovery-plan.md` after:

- LR-1/LR-2: final artifact enforcement timing and required artifact dedupe (`0800f0b`)
- LR-3: canonical verifier prompt plus quote-aware plan lint (`1ac5019`)

The goal is not to prove large-task success. The goal is to check whether the migration contract bugs moved out of the way and to reclassify remaining failures into common residuals vs profile/eval-specific issues.

## Run

- root: `eval/runs/local-recovery-large/20260617T120336`
- command: `scripts/eval_large_tasks.sh --runs 1 --out eval/runs/local-recovery-large --model qwen3.6:27b-coding-nvfp4 --timeout-secs 1200`
- provider: `ollama`
- model: `qwen3.6:27b-coding-nvfp4`
- binary: `target/release/commandagent`
- commit: `1ac5019730c0c530318e25be134d01be40787f03`
- dirty: `false`

## Summary

| Case | Result | Previous dominant issue | Current direct issue | Classification |
| --- | --- | --- | --- | --- |
| `large-fastapi-app-modify` | 0/1, `rc:1` | Python import isolation | `pytest` imports `app.main` from `/Users/maenokota/share/work/github_kewton/MySwiftAgent/expertAgent/app/main.py` | Profile/eval-specific import isolation |
| `large-fastapi-app-new` | 0/1, `rc:1` | Plan lint false positive on quoted Python semicolon | Tests run; 12 pass, 2 fail due in-memory item state leaking between tests | Implementation/repair quality residual |
| `large-nextjs-app-modify` | 0/1, `rc:1` | Dependency/build and artifact issues | Generated invalid plan used `true` verifier and `python -m py_compile app/page.tsx ... || true`; final report step hit blocked `true` and final-answer guard | Common verifier/report contract residual |
| `large-nextjs-app-new` | 0/1, `rc:1` | Plain `grep` block and Next.js contract | App artifacts exist; `npm run build` reports `dependency_missing` because `node_modules/.bin/next` is absent | Profile/eval dependency policy |
| `large-rust-app-modify` | 0/1, `rc:1` | Early `missing required final artifacts: src/lib.rs` | Final artifacts exist; `cargo check` is blocked as `Unknown` by Bash policy | Common verifier/policy mismatch |
| `large-rust-app-new` | 0/1, `rc:1` | Brittle exact `grep` verifier | Rust artifacts exist; `cargo check` is blocked as `Unknown` by Bash policy | Common verifier/policy mismatch |

Overall success is still `0/6`, but the failure shape changed materially.

## Acceptance Checks From Local Recovery Plan

### Final artifact contract is no longer a phase-local hard gate

Pass. All six cases had their final expected artifacts present at the end:

- `large-fastapi-app-modify`: no missing final artifacts
- `large-fastapi-app-new`: no missing final artifacts
- `large-nextjs-app-modify`: no missing final artifacts
- `large-nextjs-app-new`: no missing final artifacts
- `large-rust-app-modify`: no missing final artifacts
- `large-rust-app-new`: no missing final artifacts

The previous `large-rust-app-modify` failure was `missing:src/lib.rs` after an early analyze phase. In this root, `src/lib.rs` exists and the failure moves to `rc:1`, so LR-1 fixed the timing bug.

### `required_artifacts` duplicate noise is gone

Pass. Generated plan files in this root did not contain duplicate `required_artifacts` entries.

### Quoted semicolon plan-lint false positive is gone

Pass for the original failure class. `large-fastapi-app-new` no longer stops at invalid plan lint for `python -c "import ast; ..."`. It creates `app/main.py`, `app/models.py`, `app/__init__.py`, `tests/test_app.py`, `requirements.txt`, and reaches pytest.

### Brittle exact source grep is reduced but not fully solved

Partial. `large-rust-app-new` no longer fails on the exact derive grep fixture. It reaches `cargo check`. However, the common policy does not currently allow `cargo check`, even though the plan prompt now names it as canonical.

## Remaining Common Residuals

### C1: verifier prompt and Bash policy are still inconsistent

`plan_generation_prompt` now recommends `cargo check`, but `src/tools/bash.rs::classify_simple` only allows:

- `npm run build`
- `npm test`
- `cargo test`
- `cargo build`

It does not allow `cargo check`, so both Rust large cases fail with:

```text
command: cargo check
reason: blocked:Unknown: offline policy could not classify command
```

This is a common contract mismatch, not a Rust profile issue. The narrow fix is to add `cargo check` to the BuildTest allowlist and test it.

### C2: report/no-op verifier contract is unclear

`large-nextjs-app-modify` generated a report step with:

```yaml
kind: report
verify:
  - true
```

`true` is blocked as `Unknown`, and the repair turn then hits the final-answer guard while trying to summarize. This is a common DSL issue: report steps should not need no-op shell verifiers. The safer direction is to make report steps use `verify: []` and reject or correct `true`, rather than allow arbitrary no-op commands that can hide missing verification.

### C3: verifier lint still receives shell redirection/fallback forms

The saved invalid plan for `large-nextjs-app-modify` included:

```text
python -m py_compile app/page.tsx 2>/dev/null || true
```

Rejecting this is correct because it is a fallback/chained shell form and also attempts Python compilation of TSX. The remaining gap is planner guidance/correction for report and inspection steps, not Bash allowlist broadening.

## Remaining Profile/Eval-Specific Residuals

### P2: Next.js dependency setup policy

`large-nextjs-app-new` reaches `npm run build`, but verifier reports:

```text
dependency_missing: verifier_unavailable: npm run build requires node_modules/.bin/next, but it is missing.
```

The tool correctly prevents fake success by refusing to rewrite `scripts.build`. This should be resolved as an eval/profile policy decision: preseed dependencies, explicitly allow setup installs in a controlled mode, or classify dependency-missing separately from implementation failure.

### P3: Python import isolation

`large-fastapi-app-modify` still imports from an unrelated repo path:

```text
ImportError: cannot import name 'ItemCreate' from 'app.main' (/Users/maenokota/share/work/github_kewton/MySwiftAgent/expertAgent/app/main.py)
```

This is the same import isolation family identified before. It is not fixed by common plan contract changes. Candidate fixes remain package markers (`app/__init__.py`), workspace-first import execution, or fixture hygiene.

### P4: Implementation/repair quality residual

`large-fastapi-app-new` now reaches pytest and has concrete failing assertions:

```text
assert response.json() == []
assert 5 == 1
```

The failure is an in-memory state isolation bug in the generated FastAPI service/tests. This is no longer a migration-contract failure. It may need better Python profile guidance or bounded repair evidence, but it should not be mixed with LR-1/LR-3 recovery.

## Follow-up Common Cleanup Implemented

After this LR-4 triage, two common residual fixes were implemented in the same recovery pass:

1. `cargo check` is now allowed as `BuildTest` in Bash offline policy, matching the canonical verifier prompt.
2. `true` is rejected as a no-op verifier in plan lint, and the planner prompt now tells report steps to use `verify: []` instead of `true`.

Verification:

- `cargo test step_runner --lib`: pass, 70 tests
- `cargo test`: pass, 156 tests

These changes do not address profile/eval-specific residuals such as Next.js dependency setup policy or Python import isolation.

## Recommendation

Do one small common fix before returning to profile-specific work:

1. Add `cargo check` to Bash BuildTest allowlist, because it is already advertised as canonical verifier syntax.
2. Tighten report-step verifier contract so report steps use `verify: []`; do not broadly allow `true` as a shell command unless there is a concrete reason.
3. Rerun the Rust cases and `large-nextjs-app-modify` only if a case filter is added; otherwise rerun full large once.

Defer these until after that common cleanup:

- Next.js dependency install/preseed policy
- Python import isolation
- FastAPI state-reset repair/profile guidance

## Design Check

The recovery remains aligned with the CommandAgent philosophy. The changes and next recommendations remove ambiguous contracts and policy contradictions. They do not add larger repair loops, provider-specific prompts, sidecar summarization, or legacy-style advisory layers.
