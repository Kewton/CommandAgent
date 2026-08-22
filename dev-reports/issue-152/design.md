# Issues #152 and #149 design: server-side model discovery and band means

## Scope and predecessor

- This combined row owns only the GUI server portions of Issues #152 and #149:
  local provider-model discovery, structured band-duration means, and the
  minimal server/delegate wiring needed to keep those values aligned with an
  actual CLI run.
- Issue #230 commit `a450f53f` was inspected and fast-forwarded before this
  design was written. Its explicit `--allow` policy is authoritative for GUI
  delegation. The delegate must not regain blanket `--yes` authority.
- The existing GUI state transition already keeps `provider` and
  `planner_provider` aligned, and the server already preserves both pins. No
  GUI hook/component, TUI footer/notification, event schema, historical run
  evidence, corpus fixture, or `.anvil/` path changes are in scope.

## Provider-model discovery

- Add read-only `GET /api/provider-models?provider=ollama|lm-studio`. Its JSON
  body is a sorted, deduplicated array of non-empty exact model IDs.
- Ollama is read from `/api/tags`; LM Studio is read from `/v1/models`, with
  the existing optional `LM_STUDIO_API_TOKEN` forwarded only to LM Studio.
  Network, HTTP-status, and response-shape failures degrade to `[]` so exact
  manual entry remains possible. Non-local or unknown provider values are
  rejected through the existing coded JSON error shape rather than being
  used as a request target.
- Add explicit GUI-server Ollama and LM Studio host inputs with the same local
  defaults as `commandagent`. The request never accepts a host. The configured
  hosts are also passed to the delegated CLI so discovery and execution do not
  silently address different services.

## Band mean data

- Add read-only `GET /api/band-means`. It returns one additive row for every
  registered capability-band identity, keyed by profile, intent, and family,
  with `duration_n`, `average_duration_seconds`, and the evidence source.
- Derive values only from the repository's existing band-summary tables.
  Match the registered family and band arm before accepting a duration; accept
  the existing plain-second and `Ns` cell encodings. Missing or unmatched
  evidence remains honest as `duration_n: 0` and a null mean.
- Keep the existing `/api/bands` document-array contract byte-compatible. The
  new endpoint is additive and does not rewrite or infer historical evidence.

## Delegation and compatibility

- Replace GUI delegation's `--yes` with
  `--allow read,write,bash:verify`, preserving read/write work plus the shared
  verify-command boundary while excluding unrestricted Bash.
- Keep the existing confirmation, session, error, event, and provider-pin
  fields unchanged. No model discovery failure blocks Gate 1.

## Verification

- Add mock-server coverage for both local provider response shapes, sorting,
  deduplication, failure fallback, and rejection of cloud/unknown providers.
- Add focused band parser/server coverage for arm filtering, duration counts,
  means, and missing evidence.
- Pin delegated CLI arguments to the restricted allow policy and configured
  provider hosts. Then run formatting, Clippy, and the full Rust suite because
  shared CLI policy and GUI server contracts are present in the combined tree.
