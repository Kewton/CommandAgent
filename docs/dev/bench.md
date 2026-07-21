# bench v0 UAT harness

`bench.py` turns the repeatable parts of a UAT measurement protocol into a
reproducible run, while leaving adjudication to reviewers.

## Usage

From the repository root, choose a workspace outside the checkout:

```bash
python3 workspace/management/scripts/bench.py run \
  --suite dfix-synthesis --workspace-root /tmp/commandagent-bench --dry-run
```

Omit `--dry-run` to execute each suite run once. `--resume` finds the newest
campaign for the suite, skips terminal runs, and records any run left in
`starting`/`running` as `interrupted(environment)` without retrying it.
Use `--skip-suite-tests` only when explicitly accepting the recorded preflight
deviation. `--min-head` overrides the suite's ancestor requirement.

Each campaign contains `uat-meta.json`, `report-skeleton.md`, per-run copied
inputs, precheck logs, the unwrapped product command, console tails, and
archived `.anvil/` evidence.

## Scrub check

`bench.py scrub --path <directory>` scans evidence before it is considered for
publication. The same scan runs after every run is archived; a failure is
recorded as `scrub_failed` in `uat-meta.json` and warned about without stopping
evidence retention. Findings are reported masked as the first two characters
and total length. Name mentions without a value are allowed: the target is
secret existence, not vocabulary. This is an E2-style precision refinement,
not a relaxation; unconditional provider-key, JWT, and private-key patterns
remain failures. Dangerous `.env`/key files, derived directories, oversized
files, and long environment dumps also fail. Optional suite `scrub_allow`
entries require both a regular expression and a reason and are transferred to
metadata and the skeleton for review.

## Rules made mechanical

| Protocol lesson | Harness guarantee |
|---|---|
| Instructions are self-contained | Suite TOML carries the complete goal, model, provider, profile, and run matrix. |
| Procurement must be real | Only declared `copy` paths are copied; symlinks and `.git` are rejected and every declared input hash is checked. |
| Execute the exact command | argv is built from the suite, lexically checked for wrappers/control tokens, and launched directly. |
| Execute in segments | Runs are sequential and each run is started at most once. |
| Show a search method for absence claims | The report records the recursive `events.jsonl` glob, `event` field, and regex patterns used. |
| Do not skip environment gates | Clean git, HEAD/ancestor, cargo tests, release install/version, and `NODE_ENV` are recorded before procurement. |

## Deliberately not automated

The harness does not decide pass/fail, classify an incident, consume retries,
or write settlement comments. It transfers verdict, assurance, and terminal
reason text when present and leaves the corresponding fields blank in the
report skeleton for human review.
