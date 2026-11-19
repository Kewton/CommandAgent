# structured-run-budget

Synthetic corpus for Issue #976 (G5), one run budget for every structured submission: no task
text, live reply, key or host path; not a capability measurement.

## Files

- `fixtures/cases.json`: each acceptance criterion (AC1–AC6) and the tests that hold it, as
  `file::name`. `tests/structured_run_budget_off.rs` fails when a listed test does not exist.
- `fixtures/off-golden.json`: recorded v1 (Chat Completions) runs of the binary, no wallet.
- `source-manifest.json`: how the golden was recorded, and the normalized sha256 of each build.

## Off golden (AC6)

`off-golden.json` is a record, not a live comparison. The recorder `records_the_off_golden` in
`tests/structured_run_budget_off.rs` runs the binary on three paths against a fake on 127.0.0.1
with no wallet selected, and keeps every request (line, body), event line, workspace file, the
outcome and the ledger file count:

| path | replies | retries |
| --- | --- | --- |
| `compatible-chat-tool-call` | a Write call, then text | 0 |
| `compatible-chat-retry` | 500, then text | 1 |
| `lm-studio-chat-tool-call` | a Write call, then text (`--lm-studio-host`) | 0 |

To record: `STRUCTURED_RUN_BUDGET_OFF_OUT=<file> cargo test --test structured_run_budget_off --
--exact records_the_off_golden` (`STRUCTURED_RUN_BUDGET_OFF_RAW=1`: raw); unset, it does nothing.

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

## Excluded paths

`compatible-chat-ordinary-reply` and `compatible-chat-client-error` are not in the golden: the
base writes its `time_profile` event only when `time_profile.total_ms() > 0`, and these paths take
1–8 ms (stage 0: one of 94 records lacked the line). They are not compared in any weaker form.

## Known limits

- **No live byte comparison of off in the tests.** The kept paths have the same base condition
  (`src/lib.rs:669`; the same `> 0` test is in `src/eval_events.rs` and `src/tui/slash.rs`): with
  a fake that answers at once their `time_profile_total_ms` is 1–2 ms, and a run whose provider
  turns all take under 1 ms has no `time_profile` line. A comparison of today's binary with the
  golden lacked it in 2 of 11 `cargo test` runs and 9 of 30 direct runs. Widening the
  normalization or slowing the fake is not done. Issue #1062 takes up the base; add the byte test
  once it is fixed.
- Profile tool effects (`effects.rs`), a structured stage send and the planner prefetch are not
  in this golden. Their CLI runs need an extension root, a registry of host tool scripts and a
  generated plan, which do not fit this corpus. Their off is held by goldens recorded before
  #976 (`profile-tool-commit` at 85bcca4e, `structured-stage-receipts` at d2360125, both
  ancestors of 3c546943) and by the unit tests AC6 lists; under a wallet the prefetch does not
  start (`a_prefetch_under_a_run_budget_does_not_start`).
- Linux was not recorded; the sandbox backend is normalized as in #1006.
