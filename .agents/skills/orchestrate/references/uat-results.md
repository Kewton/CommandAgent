# UAT Result Input

Before an authorized merge, prepare a JSON file and pass it with `--uat-results-json`. Supply exactly one result for every numbered acceptance-criterion scenario in `uat-report.md`.

```json
{
  "results": [
    {
      "issue_number": 10,
      "scenario_index": 1,
      "status": "passed",
      "actual": "Observed behavior",
      "evidence": "Screenshot, command output, or concise manual observation",
      "candidate_head_sha": "0123456789abcdef0123456789abcdef01234567"
    }
  ]
}
```

Use only `passed`, `failed`, or `blocked` for `status`. `candidate_head_sha` is the full 40-character commit OID of the PR head or candidate build that was actually tested. During merge, the harness resolves the current `headRefOid` for each PR and requires every scenario's value to match exactly, then reads the head again immediately before merge. Missing, malformed, stale, or subsequently moved head binding blocks merging.

A merge also requires complete scenario coverage, `passed` status, and non-empty `actual` and `evidence` fields for every result. Missing, duplicate, unexpected, failed, blocked, or evidence-free results block merging. Collect results only after `ci-report.md` records all checks as passed. Keep temporary input outside frozen historical run directories; the generated `uat-report.md` records the evaluated result and head SHA.

By default the input must contain only the scenarios requested by the invocation. Pass `--allow-uat-superset` to reuse an aggregate file: entries for other Issues are ignored and counted in `uat-report.md`, while duplicate or unexpected scenarios for requested Issues remain blocking.
