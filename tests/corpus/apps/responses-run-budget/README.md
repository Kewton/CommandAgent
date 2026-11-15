# responses-run-budget

Synthetic corpus for Issue #1006 (G5): a `usage_tariff` run budget for direct, non-streaming
OpenAI Responses, and the per-call cache-write fields of the provider events. This is not a
model-capability or bench measurement. It holds no task text, expected answer, generated code, or run
record, and no live model, real key, external provider, or Jev call is used. Every price, usage,
reply, and item is synthetic.

## Files

- `fixtures/cases.json`: each acceptance criterion (AC1–AC9) and the tests that hold it.
- `fixtures/off-golden.json`: the wallet-off control, recorded on the base commit's production code
  before this feature existed. Each path holds every request (line and body), the proxy's CONNECT
  count, every event line, the workspace files, the outcome, and the count of ledger files.
- `source-manifest.json`: the base, how the recording was made, the normalization with its evidence,
  and the digests.

## Acceptance criteria and their tests

The full list is `fixtures/cases.json`. In short:

- **AC1 route and pre-send refusal**: the binary on the direct route with a v2 declaration
  (`tests/responses_run_budget.rs` `ac1_*`); the request admission and item shapes
  (`structured_budget::responses`); the client admission (`structured_budget::admission`).
- **AC2 durable attempt**: a proxy reads the ledger as each attempt arrives (`ac2_*`); the direct
  client against a fake (`providers::openai::run_budget_tests`), including the replay of reasoning
  that a budget for text alone does not let out.
- **AC3 prices and breakdown**: an integer oracle written apart from the code
  (`structured_budget::usage_tariff`); the tier a bound alone reaches, through the binary (`ac3_*`).
- **AC4 billed origin**: the budget charge apart from an unreported provider bill; v1 and v2 refuse
  each other's record; a changed declaration is refused and nothing comes back (`ac4_*`).
- **AC5 unknown, late, abnormal**: transport failures, an ended process recovered as
  `interrupted_after_start`, a reply of another tier or model. The late usage path is ledger-level
  (`structured_budget::responses`, `outcome::tests::responses_1006`): the direct client never delivers
  a late reply (review 2, N3).
- **AC6 G2, sessions, one wallet**: one physical send and N logical submissions; final repair on
  the direct client; another session and a resume get no budget back.
- **AC7 wallet off**: this corpus's `off-golden.json` (`ac7_*`), and the direct client's captures in
  `providers::openai::run_budget_tests` and `provider_call::cache_write_event_tests`.
- **AC8 per-call events**: every kind of report told apart, Chat read at its own paths only, no
  carry-over, a sum over turn events alone.
- **AC9 sensitivity**: each task's two-point record (a production change per rule turns its test red).

## Identities

- **Send**: one physical ID per HTTP POST, issued by the ledger before the send, its digest the
  bytes sent.
- **Response**: issued only from the budget charge written to disk; one per charged send.
- **Logical**: one per tool call the host looked at, of that response; N calls are N submissions of
  one send. A repeated call is refused.

## Unknown and explicit zero

A missing count is never a zero. An explicit `0` is recorded. An unknown usage key passes only as the
integer `0`; any other value stops the wallet and keeps the counts it could validate as observed. A
sent body that reads no input or costs nothing is unknown.

## Cache write fields

| reply path | event field |
| --- | --- |
| Responses `usage.input_tokens_details.cache_write_tokens` | `provider_cache_write_input_tokens` |
| … `cache_write_tokens_details.ttl_30m_tokens` | `provider_cache_write_input_tokens_30m` |
| … `cache_write_tokens_details.ttl_12h_tokens` | `provider_cache_write_input_tokens_12h` |
| Chat `usage.prompt_tokens_details.…` (the same three) | the same three |

A total with no valid split gives the total field only; a split that does not add up gives none.
`provider_turn_duration` and the same response's `provider_response` carry the same values; count
them once per turn, from the turn event.

## Off recording

Base `ff920681b8bfa9d6e11560837a83ffd96f6340bd`, a fresh git-archive copy with its own target, only
`tests/responses_run_budget.rs` added (re-recorded in F6 with the fixed root length). The recorder ran
there with the default `TMPDIR` and with `TMPDIR=/tmp`; the outputs were byte-identical.
The normalization is listed in `source-manifest.json`, each value with its evidence from two raw
(un-normalized) runs of the base:

- the temporary root and the loopback address: each run's own;
- a non-negative integer under `duration_ms`, `occurred_at_epoch_ms`, `executor_ms`, `provider_ms`,
  `total_ms`, `time_profile_provider_ms`, `time_profile_total_ms`: wall-clock, different in every
  occurrence between the two runs;
- a string under `build_commit` or `build_timestamp`: the binary's identity, another build;
- a boolean under `build_dirty`: equal here (false), kept normalized as the state of the building
  tree, not of the run;
- `pid` and `run_id` in the workspace lock file: different in every run;
- in a `bash_sandbox_status` event, a `backend` that is `seatbelt`, `landlock` or `none` (the names in
  `src/tools/os_sandbox.rs`): the host's OS sandbox, `seatbelt` on macOS and `landlock` on Linux,
  not something the run chose. Any other name, another type, or the field gone is a difference, and
  the rest of the event (`applied`, `mode`, `reason`) is compared.

A value of another type than the one named is not normalized, and a line that is not JSON is kept as a
difference. Nothing else is normalized.

**Fixed root length.** The prompt-size estimates (`estimated_prompt_tokens_sent`,
`estimated_stable_prefix_chars`, and the sums of them in `context_truncation_suspected`,
`tui_command_stop.time_profile` and `time_profile.profile`) count the raw workspace path, which
`<ROOT>` hides in the request bodies. They are not normalized. Instead, each off path runs in a root
that is exactly 128 bytes after canonicalization on every host: the temporary directory with one
directory of `p`s under it (`World::fixed_length`). A temporary directory longer than 126 bytes
fails the test before the run. The base was recorded with the same rule, and the estimates are
compared exactly.

**Diagnostics.** A difference names the path, the event line and its event, the first differing
field and both values, e.g. `responses-ordinary-reply: event line 7 (provider_turn_duration): field
estimated_prompt_tokens_sent: base 573 now 586`. The message changes, not the verdict.
