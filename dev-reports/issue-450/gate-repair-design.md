# Authorized retrospective test maintenance

After the viewport work was committed as `9b313c5157db836b7d73d6a71dc7e2dc8017e653`,
the orchestrator authorized a separate, test-only repair for the remaining broad
gate failure. This supersedes its earlier instruction to leave the retrospective
test untouched. Production scanners, historical evidence, manifests and guardrail
baselines remain outside the edit scope.

Read the failing test, scanner's read-only inventory/coverage functions, frozen
`workspace/management/runs/f1-retrospective-001/final-vectors.jsonl`, and source
commit `d39c84a3368d8b59ca0d450b439ad32133e8b78e` (`Record Luna ingest measurement`).
That commit adds the six `uat-test0802-ingest-luna-001` list/table rows and updates
`band_aggregate.py`, without updating this retrospective test's older denominator.

Keep the original 287-row count, per-profile counts and full=10 assertions on the
frozen run IDs. Additionally compare every actual historical row dictionary to
the frozen snapshot, and reject duplicate IDs. Require exactly the six known new
Luna IDs, each ingest/full. Assert current total=293, ingest=60 and full=16, retaining
score=100 and every-atom=pass checks across every current full row. This preserves
the old cohort's contract while adding exact historical content and new-cohort
checks, rather than simply increasing a denominator.

Run the focused retrospective module and Ruff, then the original unchanged broad
command with `login=false`; the observed interpreter is Python 3.12.3. Preserve
the original failures in the verification report and record final results only
after the rerun. The parent restored eight ignored A15 files at their frozen hashes
and built missing debug/release binaries locally; none will be staged or committed.
