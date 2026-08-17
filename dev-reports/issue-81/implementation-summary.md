# Issue 81 implementation summary

## Implemented behavior

- Added a client-only Trial token storage helper backed exclusively by
  `window.sessionStorage`. Its key includes the normalized GUI base path, so
  root and `/proxy/commandagent/` deployments remain isolated on one origin.
- Hydrated the password field after client mount, persisted each edit, and
  removed the key when the field is cleared. Storage exceptions retain the
  existing in-memory fallback without logging or exposing the value.
- Classified rejection by the stable `trial_token_invalid` code. The main
  Trial actions, workspace inspection, reconnect, polling, evidence reads, and
  session index all remove only the exact rejected value. A stale in-flight
  response cannot erase a newer edit, and generic proxy 401/403 failures do not
  clear storage.
- Preserved the password input, the dedicated
  `X-CommandAgent-Trial-Authorization` header, token-free reconnect URLs, and
  every existing server authentication and delegation boundary. No Rust server
  or API schema changed.

## Tests and documentation

- Added a focused Playwright storage smoke for `/` and
  `/proxy/commandagent/`. It verifies same-tab reload and authenticated access,
  independent-tab isolation, edit/clear/rejection behavior, scoped keys, and
  absence of token values from `localStorage`, URLs, static/rendered output,
  console/error output, and server diagnostics.
- Updated the full GUI smoke from its former memory-only reload expectation and
  expanded the Rust GUI source guard to pin the new security boundaries.
- Repaired stale stage/value selection in the existing authentication/Origin
  smoke so its Gate, lease-conflict, reconnect, and fake-CLI checks execute
  against the current staged UI.
- Documented ordinary tab lifetime, browser duplication/crash restore caveats,
  XSS and lost-device residual risk, base-path isolation, and token rotation.
