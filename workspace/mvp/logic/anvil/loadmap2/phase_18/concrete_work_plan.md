# Phase 18 Concrete Work Plan

## Work Package 1: Baseline Focused Blockers

Re-run or inspect the current sign-off findings:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase16-focused-local-llm/20260622T173940 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Expected current focused blockers:

- S001 / P17-F001: docs literal mismatch;
- S002 / P17-F002: Next.js dependency setup;
- S003 and S005 / P17-F003: Next.js endpoint smoke;
- S004 / P17-F004: Next.js route integration.

## Work Package 2: Targeted Reproduction Strategy

The eval runner accepts directories, so use temporary focused case subsets when
single-file execution is needed. Do not edit the canonical case tree just to
filter cases.

Recommended pattern:

```bash
mkdir -p /tmp/commandagent-phase18-cases/<case-family>
cp eval/cases/focused/control-recovery/<path-to-case>.yaml \
  /tmp/commandagent-phase18-cases/<case-family>/

bash scripts/eval_agent_slice.sh \
  --cases-dir /tmp/commandagent-phase18-cases/<case-family> \
  --out eval/runs/loadmap2-phase18-targeted-<case-id> \
  --runs 1 \
  --provider ollama \
  --model qwen3.6:27b-coding-nvfp4 \
  --binary target/release/commandagent \
  --timeout-secs 900
```

After each targeted run:

```bash
python3 scripts/eval_report.py <targeted-root> --cases-dir /tmp/commandagent-phase18-cases/<case-family>
python3 scripts/eval_report.py <targeted-root> --cases-dir /tmp/commandagent-phase18-cases/<case-family> --recheck
```

## Work Package 3: Row P17-F001 Docs Literal Mismatch

Case:

```text
eval/cases/focused/control-recovery/docs/docs-literal-mismatch.yaml
```

Current mismatch:

- expected `eval_assertion_failed` / source repair;
- observed `step_policy_failed` / explicit stop.

Decision point:

- If the impossible literal is intentionally impossible and explicit stop is
  the correct recovery behavior, update the focused expected fields and record
  stale expectation.
- If source repair should be admitted, fix recovery task / step policy owner
  selection so the row reports source repair with admitted target.

Proof:

- targeted docs rerun passes expected assertions;
- full focused matrix has no docs literal finding.

## Work Package 4: Row P17-F002 Next.js Dependency Setup

Case:

```text
eval/cases/focused/control-recovery/nextjs/dependency-setup.yaml
```

Current mismatch:

- expected completed runtime success;
- observed explicit stop.

Decision point:

- Verify whether offline focused eval can legitimately complete `npm run build`
  for this row.
- If yes, fix setup/profile/recovery behavior.
- If no, update expected fields to setup-owned explicit stop only if
  owner/action/evidence are complete.

Proof:

- targeted dependency setup rerun passes expected assertions;
- no regression in manifest repair or port conflict focused cases.

## Work Package 5: Row P17-F003 Next.js Endpoint Smoke

Case:

```text
eval/cases/focused/control-recovery/nextjs/endpoint-smoke.yaml
```

Current mismatch:

- expected ok/runtime success;
- observed plan lint failure and raw `rc:1`.

Decision point:

- First confirm the current code path now reports
  `plan_lint.invalid_expected_path` instead of raw `rc:1`.
- If the raw diagnostic is fixed but the assertion still fails, decide whether
  the remaining issue is plan lint, expected fields, or runtime behavior.

Proof:

- targeted endpoint smoke rerun has no raw undiagnostic `rc:*`;
- expected assertion passes or row is split with rationale.

## Work Package 6: Row P17-F004 Next.js Route Integration

Case:

```text
eval/cases/focused/control-recovery/nextjs/route-integration-repair.yaml
```

Current mismatch:

- expected ok/runtime success;
- observed plan lint failure with manifest repair ownership.

Decision point:

- Determine whether manifest repair is a downstream effect of dependency
  setup, or whether route integration obligation projection is wrong.
- If route ownership is correct, fix profile/plan-lint obligation and active
  job fields.
- If manifest ownership is correct, update expected fields only after proving
  dependency evidence is the true source of truth.

Proof:

- targeted route integration rerun passes expected assertions;
- dependency setup row remains correctly classified.

## Work Package 7: Full Focused Matrix

After targeted rows pass:

```bash
bash scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery \
  --out eval/runs/loadmap2-phase18-focused-local-llm \
  --runs 1 \
  --provider ollama \
  --model qwen3.6:27b-coding-nvfp4 \
  --binary target/release/commandagent \
  --timeout-secs 900
```

Generate reports:

```bash
python3 scripts/eval_report.py <phase18-focused-root> \
  --cases-dir eval/cases/focused/control-recovery
python3 scripts/eval_report.py <phase18-focused-root> \
  --cases-dir eval/cases/focused/control-recovery \
  --recheck
```

Pass condition:

- `focused_assertion_failed` count is zero;
- raw focused `rc:*` count is zero;
- previously passing focused rows remain passing.

## Work Package 8: Focused Sign-off

Run the broad sign-off checker using the Phase18 focused root and existing
smoke/fixture/large roots:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=<phase18-focused-root> \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Expected result:

- command may still exit non-zero because Phase19 large findings remain;
- output must not contain S001-S005 focused findings.

## Work Package 9: Documentation And Ledger Update

Update:

- `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`
  - P17-F001 through P17-F004 statuses to `closed_proven`;
- `docs/eval/loadmap2-phase18-focused-recovery-<date>.md`
  - targeted roots;
  - full focused root;
  - sign-off before/after;
  - remaining Phase19 blockers.

Update `docs/evaluation.md` or `docs/known-limitations.md` only if Phase18
changes interpretation or leaves an accepted limitation.

## Work Package 10: Review Before Closing

Answer before closing:

1. Are P17-F001 through P17-F004 closed by proof, not by narrative?
2. Are S001-S005 absent from sign-off output?
3. Did any new focused finding appear?
4. Did non-Next focused rows regress?
5. Are all remaining sign-off findings Phase19 large blockers?
6. Is Phase20 still blocked?

If any answer is no, Phase18 remains open.

## Review Result Reflected

The initial Phase18 shape could have overreached into broad migration closure.
This plan limits Phase18 to focused sign-off recovery, requires targeted
proof before full matrix proof, and keeps remaining large findings assigned to
Phase19.
