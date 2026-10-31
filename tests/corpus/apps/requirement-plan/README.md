# Issue #945 requirement plan fixtures

`fixtures/plan.json` holds synthetic scripted runs for the experimental
`COMMANDAGENT_REQUIREMENT_PLAN` opt-in. Every row is synthetic: the request
text, the files, the commands, the notes, and the model replies were written
for this corpus. No benchmark task text, grader, expected value, or generated
code is included.

`tests/requirement_plan.rs` reads each row, replays the script through the
public minimal loop with a mock `ChatClient`, and compares the keys the row's
`expect` object sets.

- `default-off-*` rows (unset, empty, `off`) pin the behavior of source
  `c9f88d35`, recorded before the requirement plan existed: the provider
  request digests (serialized messages plus the native-tool flag, with the
  workspace path and `elapsed_ms` normalized), the event-name sequence, the
  provider call count, and the outcome. No `requirement_plan` event appears.
- `once-captured-native` / `once-captured-xml`: the first request alone
  carries the fixed guidance; the block is taken out of the first reply before
  its tool call runs (`captured` precedes the first `tool_execute`), and the
  next request carries the note once as assistant-side context after the
  unchanged request, even though the tool-call preamble is cleared.
- `once-plan-only-turn` / `once-plan-only-turn-xml`: a reply that is only a
  valid block is one planning turn inside the existing iterations.
  `once-budget-after-plan-only-turn` keeps the five provider calls of the
  budget-end golden and its failure.
- `once-missing-block`, `once-invalid-unknown-field`, `once-oversize-block`,
  `once-question-without-block`: no note, no second request for one, and the
  same outcome and event names as the matching golden row.
- `once-xml-unclosed-tool-tag` / `once-xml-closed-invalid-tool-tag`: a tool
  tag inside a broken or unclosed block never runs (`leak.txt` is never
  written), while the off golden shows the same text executing it.
- `once-fresh-retry`, `once-compaction`, `once-pending-feedback`: the note
  stays exactly once through the empty-response fresh retry, history
  compaction, and pending host feedback.
- `once-remaining-three-skips`, `once-profile-not-generic`,
  `once-completion-contract`, `once-resumed-conversation`: excluded runs equal
  the same script with the plan unset; `once-remaining-four-applies` is the
  boundary that still applies.
- `once-xml-valid-then-closed-block`, `once-xml-valid-then-unclosed-block`,
  `once-xml-invalid-then-closed-block`, `once-xml-invalid-then-unclosed-block`,
  `once-xml-nested-block`, `once-native-valid-then-unclosed-block`: a second,
  nested, or unclosed note region in the first reply makes the note invalid;
  no region text (and nothing after an unclosed one) reaches the tool parser,
  `leak.txt` is never written, and the legitimate Write outside every region
  (or the independent native call) still runs.
- `once-max-length-plan-turn`: the output-length guard counts a planning turn
  that hit the output limit, so `max_length_no_tool_call_repeated` stops at the
  same reply as with the plan off.
- `once-last-provider-failure`, `once-last-normalization-failure`,
  `once-last-fatal-tool`, `once-last-cancel`: a failure or cancellation in the
  last iteration is released as `run_failed` or `run_interrupted`; only the
  loop's own budget-exhaustion path is `budget_exhausted`.
- `once-cancel-after-capture`: cancellation stays a failure.
- `once-self-claimed-verified` / `once-wrong-expectation`: a model claim or a
  wrong expectation changes no existing outcome or event.

Observation limits: the rows prove the host mechanism only. `captured` means
the block had the right shape, not that the obligations or expected results
are right, and nothing here measures whether a note improves correctness. A
provider client that parses XML tool calls itself before returning the reply
is outside these mock runs.
