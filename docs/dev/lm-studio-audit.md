# LM Studio provider discipline audit

Date: 2026-08-17
Scope: `src/providers/lm_studio.rs` and its shared request, configuration, and
execution boundaries.

## Result

The provider conforms to the core provider discipline after one bounded
correction: `--lm-studio-host` now rejects URL user information. Server
credentials remain declared exclusively through `LM_STUDIO_API_TOKEN`, so a
credential cannot be copied into endpoint diagnostics or the client's `Debug`
output. No allowlist was added and no Ollama or OpenAI request path was changed.

| Audit item | Result | Evidence |
| --- | --- | --- |
| Provider-call chokepoint | Conformant | `LmStudioClient` implements `ChatClient` and `boxed_clone`; the production `.chat(...)` call remains in `src/provider_call.rs`. `tests/protection_coverage_audit.rs` rejects chat sites outside that boundary, and its bounded-execution allowlist remains unchanged. Direct `/v1/models` traffic is discovery, not a model turn. |
| Credential and endpoint declaration | Conformant after correction | The optional Bearer credential is read by `LmStudioClient::from_env` only from `LM_STUDIO_API_TOKEN`. The endpoint is declared by `--lm-studio-host` (default `http://localhost:1234`) and normalized by `normalize_lm_studio_host`. Query, fragment, non-HTTP schemes, and now URL user information are rejected. `api_token_is_redacted_from_http_error_events_and_debug` proves response-event and `Debug` redaction; the configuration test proves a rejected URL does not echo either username or password. |
| Undeclared parameter injection | Conformant | LM Studio Chat Completions sends the common declared fields plus `max_tokens`; Responses sends the common declared fields plus `max_output_tokens`. `reasoning_effort` is passed as absent, and Responses omits the OpenAI-only `store` and `include` fields. Native tool schemas are sent only when `native_tools_enabled` is true. The LM Studio request tests cover these exclusions. |
| `tool_protocol` declaration | Conformant | `src/minimal_loop/tool_protocol.rs` resolves only the explicit `native`/`text` declaration and the provider capability. `supports_native_tools` ignores the model string, so there is no model-name sniffing. Explicit `text` disables native tool schemas and uses the existing validated XML parser/repair path; `native` uses the OpenAI-compatible tool schema. `allows_xml_fallback` preserves the established error-driven fallback policy and does not infer protocol from a model name. |

## Verification contract

The focused audit is:

```bash
cargo test --lib lm_studio
cargo test --test provider_onboarding
cargo test --test tool_protocol
cargo test --test protection_coverage_audit
```

The final repository gate remains `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo test`.
