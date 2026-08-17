# Issue 81 design: tab-lifetime Trial token storage

## Scope

Implement the Issue #79 decision without changing Trial authentication, API
schemas, Origin enforcement, Gate confirmation, runtime leases, reconnect
semantics, or CLI delegation. The runtime token remains a password field value
sent only through `X-CommandAgent-Trial-Authorization`.

## Design

- Add a client-only leaf helper for reading, writing, and removing the Trial
  token in `window.sessionStorage`. Storage failures fall back to the existing
  in-memory behavior and are neither logged nor reflected with secret-bearing
  errors.
- Derive the storage key from the normalized build-time GUI base path. `/` and
  `/proxy/commandagent/` therefore use distinct keys on the same origin.
- Hydrate React state after mount. Every field edit writes the new value, and an
  empty value removes the key. No storage event, `BroadcastChannel`, opener
  messaging, or other application synchronization is added.
- Remove a rejected value only when an API response carries the stable
  `trial_token_invalid` code. Preserve that code in monitoring failures so the
  polling and reconnect paths follow the same rule. Compare the rejected value
  with the current field/storage value before clearing so an older in-flight
  rejection cannot erase a newer edit. Generic upstream 401/403 responses are
  not definitive Trial-token rejections and do not clear storage.
- Keep the token out of URLs, rendered/static output, console output, server
  diagnostics, and user-visible errors. Existing session IDs remain the only
  reconnect query value.

## Verification strategy

- Extend the Rust GUI source guard for the scoped session-storage helper,
  password field, header-only request path, exact rejection classification,
  absence of `localStorage`, and absence of application cross-tab sync.
- Add a focused Playwright smoke that runs both `/` and
  `/proxy/commandagent/`, verifies reload restoration and authenticated Trial
  access, independent-tab isolation, edit/clear behavior, rejected-token
  removal, scoped keys, and absence of the token from URLs, static export,
  browser console/error text, and server diagnostics.
- Update the existing full smoke expectation from memory-only reload behavior
  to session restoration so the established Gate, lease, reconnect, Origin,
  polling, and delegation coverage remains compatible.
- Run GUI lint/typecheck/build and focused browser/server/source checks before
  the repository-wide formatting, Clippy, and test gates.
