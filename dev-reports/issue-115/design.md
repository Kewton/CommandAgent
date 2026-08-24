# Issue 115 design

## Scope

Add a pack creation wizard to the GUI **拡張** screen. The wizard covers the
operator path required by the issue: choose a profile/intent target cell, choose
an empty scaffold or the `nextjs-acme` example as the starting point, edit the
bounded pack members, stage and verify them, pin the verified exact-byte hash,
and hand the pinned local pack to Trial.

The existing Issue 114 lifecycle API remains the only write boundary. The GUI
must call its POST routes and must not write files, compute a trusted hash, or
reimplement pack conformance. No Rust pack schema, event schema, `.anvil/`
namespace, or historical evidence changes are needed.

## UI flow and state

Implement the wizard in a new leaf component and keep the assets page change to
small wiring. Its visible steps are:

1. **対象セル** — choose one registered pack profile and intent.
2. **出発点** — choose a minimal assist scaffold or, for Next.js create,
   the repository's `nextjs-acme` example bytes.
3. **編集** — edit ID, semantic version, `assist.yaml`, `eval.yaml`, and
   direct `materials/*.md` members. The Trial token is restored from and stored
   in the existing base-path-scoped session-storage namespace.
4. **検証** — POST the complete member map to stage (which already verifies),
   show conformance/scrub/hash results, and allow an explicit re-verification.
5. **pin** — submit only the server-returned hash, disclose the local
   unapproved/unmeasured status, and expose a base-path-safe Trial handoff.

Client validation can report several identity/member problems at once. Server
errors remain authoritative; their additive diagnostic report and message are
mapped to the relevant identity, YAML, or material editor. Every displayed
problem has a button that returns to the editor and focuses that control. The
mapping is presentation only and never converts a server failure into success.

After a successful pin, identity and member controls stay disabled. Retirement
requires an explicit acknowledgement, calls the existing terminal retire
route, removes the Trial handoff, and keeps all editors disabled. The UI exposes
no delete, overwrite-pin, or unretire action; the server remains the final
enforcement boundary on reload or concurrent requests.

## API client and compatibility

Add a small TypeScript client for the five existing extension routes using the
shared base-path and Trial authorization helpers. Extend `GuiRequestError`
additively to retain an optional `report` JSON value so the wizard can render
the Issue 114 verification reason without changing the existing error code or
message behavior.

The built-in example is a UI starting template, not an admission assertion. It
uses the exact current `nextjs-acme@1.0.0` member text and becomes a `local`
pack only after the server stages, verifies, and pins it. Local precedence and
the existing `ローカル（未承認・帯域未計測）` label remain visible.

## Tests and verification

- Add a focused source guard for the five-step UI, API delegation, field-focus
  recovery, immutable pinned/retired states, and absence of forbidden methods.
- Add a provider-free `--wizard-only` browser-smoke mode for both `/` and
  `/proxy/commandagent/`. It deliberately creates an `assist.yaml` failure,
  follows the item-to-field link, repairs it, verifies, pins, confirms Trial
  preselection, and retires the pack in an isolated owner-private extension
  root.
- Update the extension user guide and help-map ownership for the new action.
- Run the focused guard and document tests, GUI typecheck/lint/build, the
  wizard smoke, then repository formatting, clippy, and full Rust tests because
  shared GUI contracts and smoke infrastructure are touched.
