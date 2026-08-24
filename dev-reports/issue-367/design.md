# Issue #367 design

## Goal

Let GUI Trial users choose automatic intent detection or the typed `create`,
`fix`, and `investigate` intents, then freeze the effective intent in the
existing Gate 1 confirmation identity and delegated CLI arguments.

## Design

- Add optional `IntentId` input to the shared GUI session specification used by
  both `POST api/session-proposals` and `POST api/sessions`. Serde remains the
  single decoder, so omission preserves the existing inference path and invalid
  or unknown strings are rejected before a handler runs.
- Pass a supplied intent into `ExplicitRouteBinding`. When intent is explicit,
  the deterministic router must not add request-word intent observations; it
  still infers family/profile evidence and retains the existing contradiction
  checks for those dimensions. With no supplied intent, routing remains the
  current request/workspace inference path.
- Represent the GUI's **自動判定** choice as `null` in compose state and omit the
  `intent` property from proposal/create request JSON. Explicit choices send one
  of the three typed values. Profile or intent changes clear the selected pack,
  proposal, and confirmation state.
- Filter pack choices by the selected profile and explicit intent. Automatic
  mode keeps the historical create-pack behavior until Gate 1 resolves the
  intent. A pack deep link adopts its compatible profile and intent so
  fix/investigate packs can be selected without leaving a stale incompatible
  selector in the request.
- Display the frozen effective intent in the shared Gate 2/terminal identity
  summary. The existing identity record remains the reconnect source and all
  compose controls remain locked after launch.

## Verification approach

- Rust API integration tests cover omission compatibility, all three explicit
  values, conflicting goal vocabulary, invalid/unknown values, frozen identity,
  and delegated `--intent` arguments.
- GUI smoke assertions cover all four selector labels, reset behavior,
  compatible fix/investigate pack filtering, request payloads, Gate 1 identity,
  post-launch locking, and reconnect projection under both root and proxy base
  paths.
- Run focused GUI server/read-only guard tests and GUI typecheck/smoke first,
  then repository formatting, Clippy, and the full Rust test suite because a
  shared routing contract is touched.
