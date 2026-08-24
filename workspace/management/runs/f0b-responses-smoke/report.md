# F-0b OpenAI Responses smoke

- Result: **passed**
- Observed: 2026-08-02T00:52:54Z (2026-08-02T09:52:54+09:00)
- Path: existing `provider_call` chokepoint, `api=responses`, native tools enabled
- Request: one bounded turn requiring one `Read` function call for `README.md`
- Native tool result: `Read` call emitted; HTTP 400 regression did not recur
- Returned model: `gpt-5.6-luna`
- Response ID: `resp_0847532958af635b016a6e94e64200819a84eb234eb3af000c`
- Service tier: `default`; system fingerprint: not supplied (`null`)
- Usage: input 216 (cached 0), output 28 (reasoning 0), total 244 tokens
- Duration: 4,935 ms
- Estimated cost: USD 0.000384, using the 2026-08-02 official Luna rates
  (USD 1.00/M uncached input, USD 0.10/M cached input, USD 6.00/M output)
- Credential handling: the key was loaded into the launching process only; the
  test output, provider events summarized above, this report, and the staged
  files contain no key value. The Responses-specific reflected-key negative
  fixture also passed before this live call.

This is a smoke probe, not a band campaign. It verifies endpoint/tool protocol
compatibility and metadata capture only.
