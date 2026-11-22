# structured-run-budget

Synthetic corpus for Issue #976 (G5), one run budget for every structured submission: no task
text, live reply, key or host path; not a capability measurement.

## Files

- `fixtures/cases.json`: each acceptance criterion (AC1–AC6) and the tests that hold it, as
  `file::name`, for the #976 manifest. `tests/structured_run_budget_off.rs` fails when a listed
  test does not exist.
- `fixtures/off-golden.json`: recorded v1 (Chat Completions) runs of the binary, no wallet, on
  the three paths #976 could compare then.
- `source-manifest.json`: how that record was made, and the normalized sha256 of each build.
- `fixtures/cases-v0.1.0.json`, `fixtures/off-golden-v0.1.0.json`,
  `source-manifest-v0.1.0.json`: the Issue #1062 record, covering all five paths and compared
  live. The #976 case table and record stay as they were.

## Off golden (AC6)

`off-golden.json` is a record, not a live comparison. The recorder `records_the_off_golden` in
`tests/structured_run_budget_off.rs` runs the binary on `paths()` against a fake on 127.0.0.1
with no wallet selected, and keeps every request (line, body), event line, workspace file, the
outcome and the ledger file count:

| path | replies | retries |
| --- | --- | --- |
| `compatible-chat-tool-call` | a Write call, then text | 0 |
| `compatible-chat-retry` | 500, then text | 1 |
| `lm-studio-chat-tool-call` | a Write call, then text (`--lm-studio-host`) | 0 |
| `compatible-chat-ordinary-reply` | text | 0 |
| `compatible-chat-client-error` | HTTP 400 | 0 |

To record: `STRUCTURED_RUN_BUDGET_OFF_OUT=<file> cargo test --test structured_run_budget_off --
--exact records_the_off_golden` (`STRUCTURED_RUN_BUDGET_OFF_RAW=1`: raw).

The normalization is the eight items of `responses-run-budget` (#1006), word for word: the root
and the loopback address, the seven timing keys, `build_*`, the lock's `pid` and `run_id`, the
sandbox backend; the root is 128 bytes on every host so the prompt-size estimates are kept as they
are; nothing else.

**#976 did not change off.** The recorder ran in git-archive copies of the parent and the merge of
both #976 merges (3c546943 → fea0a54e, 62634272 → d2afc7bb) and in 6e72f59e, each with its own
target: five normalized runs per build (one with `TMPDIR=/tmp`) and two raw. All 23 normalized
records were byte-identical (sha256 `b6e60faf…c5d0`); `off-golden.json` is the 6e72f59e record and
`cmp`-equal to d2afc7bb's. The two raw records of a build differed only in the normalized values.
The method and digests are in `source-manifest.json`.

In the repository, without the binary: the committed fixtures have the digests of
`source-manifest.json`, the golden has the digest every build recorded, and one changed byte does
not. A control that drops the normalization needs a raw record; one is not committed (it holds the
run's temporary root and pid, and does not fit this corpus).

## The v0.1.0 record (#1062)

The base wrote its `time_profile` event only when `time_profile.total_ms() > 0`. On a path that
takes 1–8 ms the line was there or not by the clock, so #976 left `compatible-chat-ordinary-reply`
and `compatible-chat-client-error` out of the golden and compared no live run. Issue #1062 writes
one `time_profile` line after every stop, whatever the total, so the line no longer decides a
record.

`off-golden-v0.1.0.json` is the recorder's record of all five paths, and
`ac6_a_current_off_run_is_the_v010_record_byte_for_byte` compares a live run with it byte for
byte, normalization included. `source-manifest-v0.1.0.json` holds the digests and the repeats: the
recorder ran 12 times and the 12 records were identical (`differing: 0`); the client-error path,
which the base could record without the line, records `total_ms` 0 with the line present.

`fixtures/cases.json` and `off-golden.json` remain the #976 record and are still checked
(`ac6_the_committed_golden_has_the_digest_every_build_recorded`). `fixtures/cases-v0.1.0.json`
keeps the same AC1–AC6 table for the returned paths and points AC6 at the new record.

## Known limits

- The v0.1.0 live comparison runs the binary five times. A host whose prompt-size estimates differ
  would be seen as a difference; the fixed 128-byte root keeps them equal across hosts, as for
  #1006.
- Profile tool effects (`effects.rs`), a structured stage send and the planner prefetch are not
  in this golden. Their CLI runs need an extension root, a registry of host tool scripts and a
  generated plan, which do not fit this corpus. Their off is held by goldens recorded before
  #976 (`profile-tool-commit` at 85bcca4e, `structured-stage-receipts` at d2360125, both
  ancestors of 3c546943) and by the unit tests AC6 lists; under a wallet the prefetch does not
  start (`a_prefetch_under_a_run_budget_does_not_start`).
- Linux was not recorded; the sandbox backend is normalized as in #1006.
