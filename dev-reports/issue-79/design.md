# Issue 79 Design: Trial runtime token lifetime

## Scope and current behavior

The GUI is deployed behind Cloudflare Access, but Trial APIs also require the
server's `GUI_TRIAL_TOKEN`. The exported page currently keeps that token only in
React memory, displays it in a password input with autocomplete disabled, and
sends it only in the dedicated Bearer header. Reloading, navigating away, or
discarding the page therefore requires another manual transfer of the 64-digit
token, which is particularly costly on a phone.

This issue makes a security design decision only. It does not change the page,
server, API, token format, Cloudflare policy, or static export. Implementation
must be handled by a separate issue after this design is accepted.

The branch is integrated with current `develop` (`062386ff`), including Issues
63, 64, 66-77, and 80. Issue 63 is the relevant overlap: it permits a session
ID in `?session=<id>` for reconnect but explicitly keeps the token out of URLs
and browser storage. This decision changes only the future memory-only storage
rule; it preserves the token-free reconnect URL and composes with the other
integrated predecessor behavior.

## Security assumptions and assets

- Cloudflare Access remains the first authentication boundary. The Trial token
  remains an independent secret and is not a substitute for a restrictive
  Access policy.
- The protected asset is the bearer token and the Trial API authority it
  grants. Possession permits authenticated Trial reads and confirmed writes
  allowed by the existing API; existing Origin and Gate checks still apply.
- HTTPS is required between the browser and the Access endpoint. Browser
  extensions, a fully compromised browser profile, and an unlocked device can
  act with the user's browser authority and cannot be made safe by a Web
  Storage choice alone.
- Same-origin script execution is inside the storage trust boundary. A
  successful same-origin XSS can read Web Storage and make authenticated API
  requests.

## Options and threat comparison

| Option | Access account/policy bypass | Same-origin XSS | Lost or stolen device | Usability and decision |
| --- | --- | --- | --- | --- |
| React memory only | Access alone does not reveal the second secret. | XSS can read application state or issue requests while the page holds the token. | Exposure is limited to a live page, subject to OS/browser state recovery. | Lowest ordinary lifetime, but reload and navigation require re-entry. Rejected because it preserves the reported mobile burden. |
| `sessionStorage` per tab | Access alone still does not reveal the second secret; a new independent tab normally requires the token again. | XSS can read and exfiltrate the token for the remaining server-token lifetime. This is the accepted residual risk. | An unlocked device with the tab open can use the token. Closing the tab normally removes it, although crash/session restore and duplicated-tab behavior are browser-dependent. | Survives reload and same-tab navigation without surviving as a durable browser secret. **Adopted.** |
| `localStorage` | Access alone still does not reveal the secret, but a previously used browser profile retains it for an attacker who later passes Access. | XSS can read a durable token, including when no Trial tab was previously active in the current session. | Persists across tab and browser restarts until explicitly cleared, materially increasing loss and shared-device exposure. | Best convenience, but its unbounded lifetime is unnecessary. Rejected. |
| URL query or fragment | A copied URL can bypass the intended separation once Access is passed. | XSS can read the URL; query strings also reach servers and intermediaries. | Browser history, sync, screenshots, clipboard, and link sharing can retain the secret. Fragments avoid normal HTTP transmission but not those browser-side channels. | Rejected. The token must never enter a URL. |
| Cookie or server-side Access-to-token exchange | Could collapse the independent bearer prompt into ambient browser authority unless a new second-factor/session design preserves it explicitly. | An `HttpOnly` cookie resists direct reads, but XSS can still send same-origin requests; automatic attachment also requires a deliberate CSRF design. | Cookie lifetime and server session revocation would define exposure. | Potentially stronger against exfiltration, but changes the authentication architecture and threat model. Out of scope and not selected for this Trial. |
| Compile into assets, render into HTML, or log for retrieval | Anyone who obtains the deployment artifact or log may obtain the second secret. | Page scripts can read an embedded value. | Cached artifacts and logs outlive a tab. | Unacceptable in every variant. Rejected. |

## Decision

Store the runtime Trial token in `window.sessionStorage`, scoped to the current
browser tab lifetime. Keep it out of `localStorage`, URL query parameters and
fragments, logs, analytics, error text, rendered HTML, build inputs, and
static assets. Continue sending it only in the existing Trial authorization
header; do not change server authentication or place it in response bodies.

The implementation issue must preserve these boundaries:

- Hydrate the password field from `sessionStorage` only in browser code after
  the page mounts. Server/static rendering must never contain the token.
- Persist only the token value. Namespace the storage key by the configured GUI
  base path so two GUI deployments on one origin do not accidentally share a
  Trial token.
- Update or remove the stored value when the user edits or clears the field,
  and remove a rejected value after a definitive Trial-token authentication
  failure. Do not copy it to another persistence mechanism.
- Retain `type="password"`, `autoComplete="off"`, and the existing header-only
  request path. Do not print the value to the console, test output, server log,
  or error report.
- Treat tab duplication and browser crash/session restore as browser-dependent:
  `sessionStorage` may be cloned or restored. Do not claim guaranteed deletion
  on process exit. Closing the tab is the ordinary lifetime boundary; operators
  should still revoke/rotate `GUI_TRIAL_TOKEN` after device loss or suspected
  compromise.
- Do not synchronize the token between independent tabs. Manual re-entry in a
  new tab is an intentional limit.

## Threat conclusions

- **Access compromise:** the attacker who only defeats Cloudflare Access still
  lacks the Trial bearer token. A previously populated browser tab or stolen
  browser state is a separate, stronger compromise covered below.
- **XSS:** session storage does not protect the token from same-origin XSS. The
  attacker can read it or invoke Trial APIs as the user. CSP, dependency hygiene,
  output encoding, and prompt token rotation remain necessary defenses; this
  decision accepts that residual risk to gain reload continuity.
- **Device loss:** an unlocked device with the populated tab can exercise Trial
  authority. The token is not intentionally durable across tabs or future
  browser sessions, but browser restoration can extend its practical lifetime.
  Access session revocation and `GUI_TRIAL_TOKEN` rotation are the response.

## Follow-up implementation issue

After this design is accepted, create a separate implementation issue covering
the client-only storage helper, base-path-scoped key, hydration/update/removal
behavior, authentication-failure clearing, user documentation, and focused GUI
tests. The tests must prove reload restoration in the same tab, no
`localStorage` use, no token in URLs/static output/logging, no cross-tab
synchronization added by the application, and preservation of the existing
password/header behavior.

After the orchestrator accepted this design and searched for duplicates, it
created the implementation follow-up as Issue #81:
<https://github.com/Kewton/CommandAgent/issues/81>.
