# Step 1 cleanup adjudication

## Step 1 baseline

The complete pre-change outputs were captured before any repository mutation:

- `git status --porcelain=v1 --untracked-files=all`: 1,018 entries, all under
  `workspace/management/runs/` (the verbatim capture is retained as the
  operator artifact `/tmp/step1-status.txt`).
- `git stash list`: `stash@{0}: On develop: uat-test0717-dfix-003 preflight isolation`
  (verbatim capture: `/tmp/step1-stash.txt`).

Classification: measurement records (i) 1,018; separate work (ii) 0;
derived junk (iii) 0. Total measured size was 2,715,424 bytes and no file
exceeded 10 MiB or matched `node_modules/`, `.next/`, `target/`, or `.DS_Store`.

## Scrub and Step 1b adjudication

Command used:

```bash
find workspace/management/runs -type f \( -name '*.md' -o -name '*.json' -o -name '*.jsonl' -o -name '*.txt' -o -name '*.toml' -o -name '*.yaml' -o -name '*.yml' -o -name '*.log' \) -print0 | xargs -0 grep -nriE 'api[_-]?key|secret\s*[:=]|authorization:|BEGIN [A-Z ]*PRIVATE KEY|sk-[A-Za-z0-9]{16,}'
```

56 matches were classified M (name/description only), 0 V. Pattern counts:
`api[_-]?key` 56; `secret\s*[:=]` 0; `authorization:` 0; `BEGIN ... PRIVATE KEY` 0;
`sk-[A-Za-z0-9]{16,}` 0. The 56 rows are all Markdown prose: issue-analysis.md
32, uat-report.md 20, worker-sessions.md 4. No adjacent high-entropy value was
present. Additional scans for AIza, ghp_, xox, AKIA, JWT, and quoted
api-key/secret/token values found zero matches. This all-M distribution is
empirical support for bench scrub v0.1's allow-list rule that name mentions
without values are allowed by default.

The 1,018 measurement files are distributed across 73 timestamped
`*-orchestrate` campaigns. The complete campaign/file table is the original
status capture at `/tmp/step1-status.txt`; no paths were omitted from the
classification.

## Stash scope

`git stash show --include-untracked --name-only 'stash@{0}'` listed 160 paths;
all were under `workspace/management/runs/uat-test0715-ff1-001/`.

## Recording commits

- Measurement records: `e6ac2c8` (`Record pending campaign outputs before bench fixes`),
  staged range verified as exactly `workspace/management/runs/` (1,018 files).

The stash restoration and final cleanup are performed in the next step after
this adjudication record is committed.
