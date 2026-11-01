# output-limit-feedback

Synthetic corpus for Issue #958: the reasoning count of an OpenAI-compatible
Chat Completions response, and one fixed recovery sentence after a reply that
hit the output limit (`--num-predict`).

This is not a model capability or bench measurement. It contains no task
text, expected answers, generated code, relay records, or run records from
any bench or smoke run, and it says nothing about how a model answers the
sentence.

## Origin

Every row in `fixtures/events.jsonl` is a scripted run written for this case.
`tests/output_limit_feedback.rs` serves the row's `script` as Chat Completions
bodies from a loopback stand-in on `127.0.0.1` (one body per request, no
credentials) to the real `openai-compatible` client, so every turn goes
through the parser, the client, `provider_call`, and the minimal loop. The
bodies hold short reply texts, empty replies, `Write` of `notes.md`, `Read`
of a seeded `seed.md`, an XML fallback `Write`, and malformed tool calls:
arguments cut off inside a string, another syntax error, a missing
`function.name` or `function.arguments`, and arguments that are not an
object. A step's `completion` is the reported `usage.completion_tokens`, its
`reasoning` the raw `usage.completion_tokens_details.reasoning_tokens` value
(valid, zero, negative, fractional, a string, or above the completion
total), and `fails` marks a body that does not parse. No model, key, or
external provider is used.

Each row's `base` was recorded from the base commit
`c84aaa8203b533c7945068f4c7a8fc09bfea2be3`, before this change, by checking
out `src/` at that commit, running the same test, and restoring `src/`:

- `outcome`: the stop reason and iteration count, or the error text;
- `calls`: the number of requests the stand-in received;
- `request_digests`: SHA-256 (first 8 bytes) of each request body, with the
  temporary root and the stand-in address normalized;
- `event_names`: the event-name sequence;
- `event_digest`: one digest of every event in order, with the numbers of
  keys ending in `_ms`, `estimated_prompt_tokens_sent`, and
  `estimated_stable_prefix_chars` replaced, and, inside strings, the root,
  the address, generated ids (UUIDs), `elapsed_ms: N`, and
  `total_duration_ms=N` normalized. Every other key and value is compared as
  written.

Every body reports `prompt_tokens` above any request size estimate, so the
process-wide context-truncation warning never fires and no row depends on
the tests that ran before it. No row depends on wall-clock time or on the
platform, so macOS and Linux produce the same digests.

With the change, the test takes the sentence back out of each request that
carries it (from the end of the last message, or the whole message when it
was the sentence alone), and removes the new event and the newly reported
`provider_reasoning_tokens` from the events; the result must equal `base`
exactly. So the call count, the outcome, the existing feedback of every
request, and every existing event stay as they were. The `expect` entries
are written from the Issue's rules, not from the new implementation's
output.

Cases that need crate-internal options (a configured max-length guard
limit, a client without XML fallback, the last iteration) run in the leaf
tests in `src/minimal_loop/loop_run/output_limit_feedback/tests.rs`.

## Reasoning count

`usage.completion_tokens_details.reasoning_tokens` becomes
`ProviderResponseMetadata.reasoning_tokens` under the #928 rule: an unsigned
integer that fits in `u64` is kept, an explicit `0` stays `0`, and a missing,
null, negative, fractional, string, or out-of-range value, or one above the
reported completion total, is unreported. `provider_turn_duration` carries
`provider_reasoning_tokens` only when the value was kept. A body that fails
to parse has no metadata, so the reasoning share of a run is measured only
over the responses that parsed; a parse-error turn is missing, never zero.

## Event contract

`output_limit_feedback` is written once for each request that carried the
sentence and reached the client:

| Field | Meaning |
|---|---|
| `kind` | `no_tool_call_at_output_limit` or `tool_arguments_truncated_at_output_limit` |
| `attempt` | requests of that kind sent so far in the run |
| `num_predict` | the configured output limit |
| `completion_tokens` | the reported completion count of the turn that hit the limit |
| `prompt_version` | `1` |
| `provider_reasoning_tokens` | only when that same turn reported a kept value |

It holds no prompt, model text, tool arguments, or path.

- `no_tool_call_at_output_limit`: a reply without tool calls with
  `completion_tokens >= num_predict` and `num_predict > 0`.
- `tool_arguments_truncated_at_output_limit`: a native tool-call parse error
  the loop retries, whose typed context says the arguments ended early and
  whose reported completion count is at least `num_predict > 0`. Another
  syntax error, a missing name or arguments, non-object arguments, or a count
  below the limit keep only the existing malformed-call feedback.

No sentence and no event follow a reply after which no request is sent: the
last iteration, a max-length guard stop, an exhausted malformed retry, or an
accepted final. A reply with a tool call never gets the sentence.
