# Issue #940 completion obligation review fixtures

`fixtures/review.jsonl` holds synthetic scripted runs for the experimental
`COMMANDAGENT_COMPLETION_OBLIGATION_REVIEW` opt-in. Every row is synthetic:
the request text, the files, the commands, and the model replies were written
for this corpus. No benchmark task text, grader, expected value, or generated
code is included.

`tests/completion_obligation_review.rs` reads each row, replays the script
through the public minimal loop with a mock `ChatClient`, and compares the
keys the row's `expect` object sets.

- `default-off-*` rows (unset, empty, `off`) pin the behavior of source
  `a49e55c9`, recorded before the review existed: the provider request
  digests (workspace path and `elapsed_ms` normalized), the event-name
  sequence, and the outcome. No review event appears.
- `once-triggered-executed-final`: the first normal final after a successful
  Write is deferred once, a Bash check runs, and the next normal final is
  returned (`triggered` then `closed`/`next_final`).
- `once-self-report-final` and `once-plain-second-final`: an unconfirmed
  ending. The next final closes the review whatever it says; no second request.
- `once-reedit-invalidates` and `once-unsuccessful-observations`: a later edit
  drops older successful Bash observations; a non-zero exit, a tool error, and
  a policy rejection are never counted as successful.
- `once-remaining-three-fires` / `once-remaining-two-skips`: the remaining
  iteration boundary.
- `once-read-only-run`, `once-profile-not-generic`, `once-completion-contract`,
  `once-remaining-two-skips`: excluded runs. Their requests, outcome, and
  non-review events equal the same script with the review unset, so the
  existing stop and assurance inputs are unchanged.
- `once-existing-blocker-first`: an existing completion blocker answers first.
- `once-cancel-after-request` / `once-budget-exhausted-after-request`: the
  existing failure is kept and the review records
  `scope_exit_before_next_final` once.

The review events carry numbers and fixed words only. They say that a command
ran successfully after the latest edit, never that a specification was
covered, verified, or accepted.
