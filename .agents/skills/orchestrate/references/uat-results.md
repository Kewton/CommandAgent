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
      "evidence": "Screenshot, command output, or concise manual observation"
    }
  ]
}
```

Use only `passed`, `failed`, or `blocked` for `status`. A merge requires complete scenario coverage, `passed` status, and non-empty `actual` and `evidence` fields for every result. Missing, duplicate, unexpected, failed, blocked, or evidence-free results block merging. Collect results only after `ci-report.md` records all checks as passed, and use the latest draft-PR commit or candidate build. Keep temporary input outside frozen historical run directories; the generated `uat-report.md` records the evaluated result.
