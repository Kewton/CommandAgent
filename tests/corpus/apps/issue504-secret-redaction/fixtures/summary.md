# Run summary

Status: complete
Command status: completed
Task status: completed (reduced assurance)

## Secret redaction

- exact-value catalog scrub: enabled across session, events, evidence, trace, summary, provider conversation/tool schema, and the display stream callback.
- rejected Bash command prefix: omitted. `command_prefix_bytes` is an empty array and cannot reconstruct the raw value; `schema_version`, `command_sha256`, and `command_bytes` are retained.
- invalid `${ENV}` base_url: the error chain reports the field and validation kind only; the expansion value (`<redacted>`) never appears in stderr or doctor output.
- runnable YAML or confirmation identity that still contains a registered secret: refused honestly, without naming the value.
- ordinary short values (`PORT=3000`, `true`, `dev`) stay ordinary; a short credential named `TOKEN`/`API_KEY` is still registered.

Stop reason: completed
Next action: none
