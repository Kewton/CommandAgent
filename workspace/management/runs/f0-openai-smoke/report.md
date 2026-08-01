# F-0 OpenAI Luna smoke

## Result

The minimal live probe completed through the production `provider_call`
chokepoint. It used the exact executor model ID `gpt-5.6-luna`, the OpenAI Chat
Completions endpoint, a bounded 60-second deadline, no tools, and the prompt
`Reply with exactly: hello`. The reply was non-empty and the turn completed
with `finish_reason=stop`.

The local ignored `.env` was used only by the operator shell to populate the
child process environment without printing the value. `OpenAiClient` did not
read that file: its production credential boundary accepted
`OPENAI_API_KEY` from the child process environment only.

## Command boundary

The smoke entry point was:

```text
COMMANDAGENT_LIVE_PROVIDER_TESTS=1
COMMANDAGENT_OPENAI_SMOKE_MODEL=gpt-5.6-luna
COMMANDAGENT_OPENAI_SMOKE_EVENTS=/private/tmp/f0-openai-smoke-events.jsonl
cargo test --test live_provider live_openai_luna_chokepoint_smoke -- --ignored --nocapture
```

The key assignment is intentionally omitted. The test calls
`provider_call::chat` rather than `ChatClient::chat`, so timeout, cancellation,
bounded execution, and the central provider-turn event are exercised together.

## Response metadata

| field | observed value |
|---|---|
| requested model | `gpt-5.6-luna` |
| provider model ID | `gpt-5.6-luna` |
| response ID | `chatcmpl-E7vwZHvc4jUW3mM0UKjt2Mjfq4HXU` |
| system fingerprint | `null` (provider returned no fingerprint) |
| service tier | `default` |
| provider created epoch | `1785559739` |
| caller scope | `executor` |
| duration | `1,871 ms` |
| input / output tokens | `11 / 4` |
| result | `ok=true`, `finish_reason=stop` |

The curated machine-readable record is
[`provider-turn-metadata.json`](provider-turn-metadata.json). The temporary
three-event stream had SHA-256
`65ae139ff9fbee07a35e7363cf15807abac0136a65b38e079edca61d80fb4bea` and is not
committed as a raw log.

## Cost and time

The provider-created epoch was `1785559739`; the audit record was captured at
`1785559768` (`2026-08-01T04:49:28Z`). Provider-turn duration was `1.871 s`.

The official Luna standard price observed on 2026-08-01 was USD 1.00 per
million input tokens and USD 6.00 per million output tokens. For 11 input and
4 output tokens, the estimated API charge is:

```text
(11 × $1.00 + 4 × $6.00) / 1,000,000 = $0.000035
```

No cache discount was assumed.

## Credential and redaction audit

- The API key value was never printed or copied into this run directory.
- The deliberate reflected-secret negative test passed across returned errors,
  `provider_error` events, run-summary evidence, and the client's `Debug`
  representation.
- The bench scrubber recognizes modern `sk-` keys containing hyphens and
  underscores.
- `bench.py scrub --path workspace/management/runs/f0-openai-smoke` returned
  `ok=true` with zero findings.
