# Run summary

Status: complete
Command status: completed
Task status: completed (reduced assurance)
Result: <redacted>

## Secret redaction

- exact-value catalog scrub: enabled across session, events, evidence, trace, summary, provider conversation/tool schema, and the display stream callback. Every registered value is replaced by an exact, case-sensitive match; no free field is redacted whole.
- registration policy: a credential value shorter than 8 characters, a credential value equal to a reserved marker or colliding with a fixed identifier, an unreadable source, and a combined overflow past 1024 values each refuse startup honestly without naming the value. Ordinary short values (`PORT=3000`, `true`, `dev`) stay ordinary.
- fixed identifiers: a top-level event/schema/status/verdict/type identifier and a tool protocol name are preserved only when the value is itself a fixed identifier; any other value under a fixed key is scrubbed. `FIXED_IDENTIFIERS` is the single list.
- summary values: a `Status:`/`Result:` value is preserved only when it is a fixed machine identifier (`completed`, `incomplete`, `failed`, ...); any other free-input value is scrubbed to `<redacted>`.
- rejected Bash command prefix: omitted. `command_prefix_bytes` is an empty array and cannot reconstruct the raw value; `schema_version`, `command_sha256`, and `command_bytes` are retained.
- invalid `${ENV}` base_url: the error chain reports the field and validation kind only; the expansion value (`<redacted>`) never appears in stderr or doctor output.
- runnable YAML or confirmation identity that still contains a registered secret: refused honestly, without naming the value.

Stop reason: completed
Next action: none
