# structured-run-budget

Synthetic corpus for Issue #976 (G5): one run budget for every structured submission. It holds
no task text, run record, live reply, key or host path, and is not a capability measurement.

## Files

- `fixtures/cases.json`: each acceptance criterion (AC1–AC6) and the tests that hold it, as
  `file::name`. `tests/structured_run_budget_off.rs` fails when a listed test does not exist.
- `fixtures/off-golden.json`: v1 (Chat Completions) runs of the binary with no wallet selected.
- `source-manifest.json`: how the golden was recorded, and the normalized sha256 of each build.

## Off golden (AC6)

`ac6_with_no_wallet_a_v1_run_is_byte_equal_to_the_recording_before_976` runs the binary on three
paths against a fake on 127.0.0.1 and compares the record with `off-golden.json` byte for byte:
every request (line, body), event line, workspace file, the outcome and the ledger file count.

| path | replies | retries |
| --- | --- | --- |
| `compatible-chat-tool-call` | a Write call, then text | 0 |
| `compatible-chat-retry` | 500, then text | 1 |
| `lm-studio-chat-tool-call` | a Write call, then text (`--lm-studio-host`) | 0 |

The normalization is the eight items of `responses-run-budget` (#1006), word for word: the root
and the loopback address, the seven timing keys, `build_*`, the lock's `pid` and `run_id`, the
sandbox backend; the root is 128 bytes on every host so the prompt-size estimates are compared as
they are; nothing else. The control test shows a raw (unnormalized) run and a golden with one
changed byte both fail.

**#976 did not change off.** The recorder ran in git-archive copies of the parent and the merge of
both #976 merges (3c546943 → fea0a54e, 62634272 → d2afc7bb) and of 6e72f59e. Every normalized
record was byte-identical (sha256 `b6e60faf…c5d0`), and two raw records per build differed only in
the normalized values. The method and digests are in `source-manifest.json`. CI builds no old
commit: it compares today's binary with the golden.

## Excluded paths

`compatible-chat-ordinary-reply` and `compatible-chat-client-error` are not in the golden. The
base writes its `time_profile` event only when `time_profile.total_ms() > 0` (`src/lib.rs`, the
direct CLI command); these paths take 1–8 ms, so the line is there or not by the clock (stage 0:
one of 94 records lacked it). Allowing a missing line would be wider than #1006, so they are not
compared in any weaker form.

## Known limits

- The kept paths are not free of that condition: their `time_profile_total_ms` read 1–2 ms on an
  idle macOS host (never 0 in 120 raw path runs). A run where every provider turn takes under 1 ms
  would lack the line and fail; that is a property of the base, not of a wallet.
- Profile tool effects (`effects.rs`), a structured stage send and the planner prefetch are not
  in this golden. Their CLI runs need an extension root, a registry of host tool scripts and a
  generated plan, which do not fit this corpus. Their off is held by goldens recorded before
  #976 (`profile-tool-commit` at 85bcca4e, `structured-stage-receipts` at d2360125, both
  ancestors of 3c546943) and by the unit tests AC6 lists; under a wallet the prefetch does not
  start (`a_prefetch_under_a_run_budget_does_not_start`).
- Linux was not recorded; the sandbox backend is normalized as in #1006.
