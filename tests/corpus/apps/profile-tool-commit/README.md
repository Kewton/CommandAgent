# profile-tool-commit

Synthetic corpus for Issue #974 (G3): a process the operator pinned proposes bytes for one
declared output, and only the host, after checking them, adopts them. This is not a
model-capability or bench measurement, and it holds no task text, expected answer, generated
code, or run record from any bench or smoke run. No live model, real key, external provider,
or Jev call is used. Every secret is a dummy.

## Origin

Base commit: `85bcca4e1b85ffc33549a908a29250fcf87dd261`.

The two accepted profiles are generic synthetics: `commit-config` proposes a settings object
and `commit-index` proposes a document index. They go through the same Rust path, with the
real Chat Completions client against a disposable fake on 127.0.0.1, a fixed script as the
pinned process, and a temporary workspace.

## Files

- `fixtures/cases.json`: the accepted rows; the results that must save nothing; the rows
  refused before the start, which every public entry must refuse with zero provider,
  classifier, Jev, process, and save; the unsupported rows; and the off rows.
  `tests/profile_tool_commit.rs` asserts these rows are the ones it runs.
- `fixtures/off-golden.json`: the off control, recorded by running the public session against
  the base commit's production code with a recorder that does not remain in this repository,
  before this feature existed. Each row holds the outcome, every request body as it reached
  the fake, every event line, the workspace artifacts, and the unused replies.

## What the off comparison normalizes

Nothing is left out. Inside the recording, exactly these are replaced by a placeholder:

- The numbers of six keys: `epoch`, `occurred_at_epoch_ms`, `duration_ms`, `elapsed_ms`,
  `estimated_prompt_tokens_sent` (it counts the system prompt that names the temporary root),
  and `estimated_stable_prefix_chars` (measured against a process-wide cache).
- Inside strings: the two temporary roots, the fake's loopback URL, generated UUIDs,
  `elapsed_ms: N`, `total_duration_ms=N`, and `"epoch":N` in an evidence file.

## Which test runs where

Nothing here is skipped. On macOS a failed strict probe fails the test: a skip is never
evidence of isolation. Off macOS the same entries expect `unavailable`.

| Test | macOS | Linux and others |
| --- | --- | --- |
| `off_wire_events_and_artifacts_match_the_base_recording_byte_for_byte` | Yes | Yes |
| `off_starts_nothing_and_reads_no_registry` | Yes | Yes |
| `every_entry_refuses_before_any_provider_classifier_process_or_save` | Yes, each row's own kind | Yes, `unavailable` for a row that needs the probe first |
| `the_binary_refuses_before_a_provider_or_jev_on_each_action_and_selection` | Yes | Yes |
| `a_binary_workflow_child_is_refused_before_its_provider_call` | Yes | Yes |
| `a_library_workflow_refuses_on_its_origin_before_the_reproducer` | Yes | Yes |
| `repl::a_repl_refuses_free_text_before_its_classifier` | Yes | Yes |
| `repl::a_profile_named_after_the_free_text_gate_is_refused_at_its_command` | Yes | Yes |
| `the_corpus_manifest_pins_its_fixtures`, `the_corpus_names_what_this_binary_runs` | Yes | Yes |
| `not_macos::every_entry_expects_unavailable_with_no_provider_or_process` | No | Yes |
| `not_macos::the_binary_expects_unavailable` | No | Yes |
| `macos::a_config_object_and_a_document_index_commit_through_the_same_mechanism` | Yes | No |
| `macos::every_session_entry_reaches_the_commit` | Yes | No |
| `macos::an_inspect_step_is_refused_before_its_provider_call` | Yes | No |
| `macos::a_planner_only_call_under_a_planning_action_is_refused` | Yes | No |
| `macos::every_bad_result_saves_nothing_and_ends_the_run_without_a_retry` | Yes | No |
| `macos::exit_0_with_nothing_and_a_nonzero_exit_with_a_valid_result_are_refused` | Yes | No |
| `macos::a_broken_or_repeated_argument_starts_nothing_and_saves_nothing` | Yes | No |
| `macos::the_argument_and_schema_limits_hold_at_their_edges` | Yes | No |
| `macos::a_result_over_its_narrowed_limit_is_stopped_and_saves_nothing` | Yes | No |
| `macos::a_stuck_process_is_stopped_at_its_clock_and_saves_nothing` | Yes | No |
| `macos::a_process_that_never_reads_a_large_request_does_not_hold_the_host` | Yes | No |
| `macos::a_process_that_tries_to_fork_gets_no_descendant_and_no_commit` | Yes | No |
| `macos::a_cancel_during_the_process_stops_it_and_saves_nothing` | Yes | No |
| `macos::a_cancel_after_the_commit_keeps_the_commit_and_its_receipt` | Yes | No |
| `macos::the_public_registry_cannot_go_around_the_guard` | Yes | No |
| `macos::the_public_registry_delivers_a_cancel_and_keeps_a_commit_it_already_made` | Yes | No |
| `macos::the_process_cannot_write_read_or_connect_and_only_the_host_commits` | Yes | No |
| `macos::the_launch_watch_sees_a_tool_that_really_starts` | Yes | No |
| `macos::what_a_failed_process_said_stays_on_the_event_as_scrubbed_data` | Yes | No |
| `macos::what_a_successful_process_said_does_not_reach_the_model` | Yes | No |
| `macos::a_dependency_rewritten_during_the_process_stops_the_commit` | Yes | No |
| `macos::a_changed_env_value_after_admission_is_a_pin_mismatch` | Yes | No |
| `macos::a_path_swapped_after_the_start_is_refused_by_the_commit` | Yes | No |
| `macos::the_binary_commits_on_prompt_run_plan_and_each_pack_selection` | Yes | No |
| `macos::the_binary_refuses_an_explicit_read_only_policy_before_any_call` | Yes | No |
| `macos::the_binary_is_unavailable_when_the_strict_probe_really_fails` | Yes | No |
| `macos::every_library_entry_is_unavailable_when_the_strict_probe_really_fails` | Yes | No |
| Unit: `tools::profile_tools::{protocol,process,commit,effects}::tests` | Yes | Yes, with an unconfined launch only a test build can ask for |
| Unit: `tools::profile_tools::isolation::tests::on_macos_the_real_probe_passes` and `real_isolation::*` (the `errno` of each attempt) | Yes | No |
| Unit: `tools::profile_tools::isolation::tests::off_macos_the_probe_and_the_launch_are_unavailable` | No | Yes |

`unavailable` on macOS is measured by a real failure of the probe: the binary and a child of
the test binary run under an outer `sandbox-exec`, where a second one cannot start. Production
has no switch for this.

## Known limits

- The process runs only on macOS. No Linux backend is implemented here, so nothing on Linux
  shows a process running; the Linux expectation is the refusal, with nothing started.
- A descendant cannot start under the policy, so the reaping in `process.rs` is exercised with
  a real `setsid` child in its unit tests, whose launch is unconfined.
- The fake process is bash with builtins, because it cannot fork, so its two candidates and
  their digests are computed by the test and written into the fixed script.
- The mutation rows named in the issue's dispatch are measured by the leader, not here.
