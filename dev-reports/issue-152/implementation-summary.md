# Issues #152 and #149 implementation summary

## Outcome

Implemented the approved combined server slice without editing GUI
hooks/components or TUI footer/notification code. The GUI server now exposes
read-only local model discovery and structured capability-band duration means,
while delegated runs retain the selected provider hosts and Issue #230's
restricted tool authority.

## Changes

- Added `GET /api/provider-models?provider=ollama|lm-studio`.
  - Ollama reads `/api/tags`; LM Studio reads `/v1/models`.
  - Exact IDs are trimmed, deduplicated, and sorted.
  - LM Studio's optional process token is forwarded as bearer authentication.
  - Reachability, HTTP, and response-shape failures return `[]`, preserving
    manual model entry as the fallback.
  - Cloud and unknown providers return the shared coded JSON error shape and
    are never used as request targets.
- Added GUI-server `--ollama-host` and `--lm-studio-host` inputs with the same
  defaults as `commandagent`. Hosts are validated, credentials/query material
  are rejected, and redirects are disabled during discovery.
- Added `GET /api/band-means`, returning one row per registered
  profile/intent/family band identity with its duration count, arithmetic mean,
  and evidence source. The reader reuses the server's confined, symlink-safe,
  size-bounded document path and keeps missing/unmatched evidence explicit as
  zero observations plus a null mean.
- Kept `/api/bands`, provider pins, errors, session payloads, events, and
  historical evidence unchanged.
- Replaced GUI delegation's blanket `--yes` with
  `--allow read,write,bash:verify`, and passed the configured provider hosts to
  the delegated CLI. This preserves ordinary read/write work and shared verify
  commands without admitting unrestricted Bash.
- Narrowed the GUI read-only source guard so only `trial_options.rs` may own a
  redirect-disabled GET client; provider runtimes and mutating HTTP methods
  remain forbidden throughout the server.

## Tests

- Added loopback mock coverage for Ollama and LM Studio response shapes, token
  forwarding, deterministic model lists, failure fallback, and unsupported
  provider rejection.
- Added parser and server coverage for family/arm filtering, plain and suffixed
  duration encodings, mean/count output, and missing band evidence.
- Extended delegation coverage to reject `--yes` and pin the exact allow-list
  and provider-host CLI arguments.

No corpus fixture changed because no event, recovery, or corpus contract was
modified.
