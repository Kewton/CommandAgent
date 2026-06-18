# Gemini/OpenAI XML Fallback Smoke

Date: 2026-06-18
Commit: `6a0cbe0`
Dirty: true
Binary: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop/target/release/commandagent`
Entrypoint: `/Users/maenokota/.local/bin/commandagentdev`
Smoke root: `/private/tmp/commandagent-api-smoke-20260618170957`

## Scope

Manual opt-in provider smoke using repo-local `.env` exported by the shell.
This check is intentionally outside `cargo test`, CI, and default eval scripts.
API key values were not printed or copied into this report.

## Result

| Provider | Model | Check | Result |
| --- | --- | --- | --- |
| Gemini | `gemini-3.1-flash-lite` | one-shot `hello.txt` creation | pass |
| OpenAI | `gpt-5.4-mini` | one-shot `hello.txt` creation | pass |

Created files:

- Gemini: `hello.txt` contained `Gemini XML fallback smoke`
- OpenAI: `hello.txt` contained `OpenAI XML fallback smoke.`

Secret scan over the smoke root passed. No `GEMINI_API_KEY` or
`OPENAI_API_KEY` value was found in generated smoke artifacts.

## Notes

The first live Gemini attempt created `hello.txt` but repeated XML fallback
`Write` calls until max iterations. That exposed missing assistant-history
preservation after provider-level XML parsing stripped the tool-call block from
assistant content. The implementation now renders parsed XML fallback tool
calls back into assistant history for the next provider request.
