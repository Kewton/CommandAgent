# profile-tool-schema

Synthetic corpus for Issue #973 (G2): nested arguments checked against a schema bound
to an input revision, at the host's receiving boundary. This is not a model-capability
or bench measurement, and it holds no task text, expected answer, generated code, or
run record from any bench or smoke run. No live model, real key, external provider, or
Jev call is used. Every secret is a dummy.

## Origin

Base commit: `29d4d369536eda948a226a1af74d6038a2203389`.

The two accepted profiles are generic synthetics: `schema-config` submits a settings
object and `schema-index` submits a document index. They go through the same Rust path,
with the real Chat Completions client against a disposable fake on 127.0.0.1, in-process
fake executors, and a temporary workspace.

## Files

- `fixtures/cases.json`: the accepted rows, on the native wire; the rejected rows, which
  run on the native wire and must reach zero executions; the rows refused before the start,
  which every public entry must refuse with zero provider, classifier, and Jev calls,
  including a schema tool selected on the text tool protocol; and the off rows.
  `tests/profile_tool_schema.rs` asserts these rows are the ones it runs.
- `fixtures/off-golden.json`: the off control, recorded by running the public session
  against the base commit's production code with a recorder that does not remain in this
  repository, before this feature existed. Each row holds the outcome, every request body
  as it reached the fake, every event line, the workspace artifacts, and the unused
  replies. It covers the Chat Completions parser's last-value and recovery behavior, the
  explicit-text XML parser's strict, relaxed, repaired, and refused paths, a flat
  extension's accepted and refused calls, and a dummy secret next to the words this issue
  reserves.
- `fixtures/responses-off-golden.json` (Issue #1005): the Responses API with no schema tool,
  recorded on `responses_off_base_sha` before the Responses schema wire existed, through the
  OpenAI-compatible entry and a loopback fake. `tests/profile_tool_schema_responses.rs` holds the
  recorder's own code (`responses_off`) and compares the same paths with it byte for byte. Its
  normalization is the six rules written in the golden, and nothing in a request body is
  normalized.
- `fixtures/responses-cases.json` (Issue #1005): the Responses rows, on two axes, `api:
  responses` and `tool_protocol: native`. The `wires` of `cases.json` stay as they are and are
  not read as a Responses tool protocol. Each row's expectation is the same calls on Chat
  Completions in the same test, the accepted rows of `cases.json` among them, or a golden; none
  comes from the Responses implementation. `id_errors` names the call ID cases, which the test
  asserts are the ones it runs.

## What the off comparison normalizes

Nothing is left out. Inside the recording, exactly these are replaced by a placeholder:

- The numbers of six keys: `epoch`, `occurred_at_epoch_ms`, `duration_ms`, `elapsed_ms`,
  `estimated_prompt_tokens_sent` (it counts the system prompt that names the temporary
  root), and `estimated_stable_prefix_chars` (measured against a process-wide cache).
- Inside strings: the two temporary roots, the fake's loopback URL, generated UUIDs,
  `elapsed_ms: N`, `total_duration_ms=N`, and `"epoch":N` in an evidence file.

## Known limits

- A schema tool runs on native tool calls only: Chat Completions, and the Responses API since
  #1005. On the text (XML) tool protocol it is refused before the start as
  `schema_unsupported`; XML support is #978. The XML rows in the off recording are the old
  parser's control, with no schema tool, and stay.
- A fake shows the host's contract on the Responses wire, not that a service accepts `strict`,
  a call turn without its reasoning, or a turn with or without its preamble.
- The binary registers no in-process runtime or compiler, as in G1, so through the CLI a
  schema tool is refused as `unavailable`. Success rows run through the library entries.
- The mutation rows named in the issue's dispatch are measured by the leader, not here.
