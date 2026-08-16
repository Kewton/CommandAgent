# Issue 63 design: resilient GUI trial monitoring

## Cause

The Trial page schedules its next status request only after a successful `GET`.
Any thrown `fetch`, authorization response, proxy redirect, or other failed poll
therefore leaves the delegated CLI running while the browser permanently stops
observing it. Execution status and monitoring health are also rendered as one
green `running` label, so stale execution data looks live.

## Design

- Keep execution unchanged. Poll and reconnect only through the existing
  authenticated `GET /api/sessions/{id}`, which reconstructs status from the
  current JSONL events and acceptance artifacts.
- Move polling policy into a small GUI leaf module. Successful polls retain the
  750 ms cadence; failures use exponential backoff capped at 12 seconds.
  Oversized event streams and invalid event JSONL stop after four attempts.
  Other failures continue retrying at the capped interval.
- Model monitoring separately as `connected`, `degraded`, or `lost`, with the
  consecutive attempt count and last successful update timestamp. The execution
  label's green indicator is enabled only while monitoring is connected.
- Use `redirect: "manual"` for status GETs. Treat `opaqueredirect` as an upstream
  Access re-authentication boundary, distinguish thrown browser fetch failures,
  and give 401/403 token-specific guidance.
- Put only the session ID in `?session=<id>`. Keep the runtime token in component
  memory. A reload exposes a reconnect form; after token re-entry, its action
  performs one GET and resumes polling without any POST or CLI dispatch.
- When session creation returns the existing 409 workspace-lease message,
  extract its session ID, place it in the reconnect field/URL, and show the same
  GET-only recovery action.

## Tests and verification

- Extend the two-base-path Playwright smoke so the first root status fetch throws
  and later succeeds, while the proxy case observes a manual Access redirect and
  recovers. Assert Gate 3/4 is reached in both cases.
- Reload the terminal session URL, re-enter the token, reconnect, and assert all
  reconnect-path session calls are GETs.
- Add a mobile viewport probe for `/` and `/proxy/commandagent/`.
- Run GUI lint/build first, focused GUI server and delegation guard tests next,
  then repository formatting, clippy, and the full Rust test suite.
