# profile-tool-registration

Synthetic corpus for Issue #972 (G1): a selected profile or pack references opaque
tool IDs, and an operator registry outside the workspace owns what they mean. This
is not a model-capability or bench measurement, and it holds no task text, expected
answer, generated code, or run record from any bench or smoke run. No live model,
real key, external provider, or Jev call is used.

## Origin

Base commit: `104a166b5680422d89db5524ee1d93caed281a11`.

The two accepted profiles are generic synthetics: `tool-config-a` reads a settings
object and `tool-index-b` reads a document index. They go through the same Rust
path, with a scripted `ChatClient` and in-process fake executors, in a temporary
workspace.

## Files

- `fixtures/cases.json`: the accepted rows, the rejection rows with their refusal
  kind (each runs through the public minimal loop and the public step-plan planner
  and must reach zero provider calls), and the off rows.
- `fixtures/off-golden.json`: the off control, recorded by running the public entries
  against an export of the base commit, before this feature existed. Each row holds
  the outcome, every request (messages, tool schema, native flag), every event line,
  the workspace artifacts, and the unused replies.

## What the off comparison normalizes

Nothing is left out. Inside the recording, exactly these are replaced by a placeholder:

- The numbers of five keys: `occurred_at_epoch_ms`, `duration_ms`, `elapsed_ms`,
  `estimated_prompt_tokens_sent` (it counts the system prompt that names the temporary
  root), and `estimated_stable_prefix_chars` (measured against a process-wide cache).
- Inside strings: the temporary root, generated UUIDs, `elapsed_ms: N`, and
  `total_duration_ms=N`.

## Known limits

- The binary registers no in-process runtime in G1, so through the CLI a tool ID is
  refused as `unavailable`. The accepted rows use fakes a Rust test registers.
- Mode and `--allow` intersection is checked by the unit test
  `profile_allowlist_intersects_mode_and_allow_policy` in `src/tools/registry.rs`,
  because the session always runs in Act and the policy installer is crate-private.
