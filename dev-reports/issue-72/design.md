# Issue 72 design

## Problem

The GUI server currently returns JSON containing only an `error` string, and
the browser either prints that raw body or exposes the browser's network error.
This makes authentication, Origin, confirmation, workspace lease, CLI-path,
and repository-read failures look alike even though their recovery actions are
different.

## Constraints

- Preserve every existing HTTP status and `error` string. The new response
  field must be additive so existing direct clients and the 428 browser smoke
  assertion remain compatible.
- Keep Trial authentication, Origin, confirmation, lease, and CLI delegation
  gates unchanged.
- Do not add a GUI state store or alter the `.anvil/` runtime namespace.
- Remain compatible with predecessor routes and polling/reconnect work. Those
  branches extend `AppState`, `sessions.rs`, and the Trial page, but do not
  define a coded error contract.

## Design

1. Add a leaf `gui_server/error_response.rs` module that owns the common Axum
   JSON error response. It emits `{ "code": ..., "error": ... }` and is used
   by both repository-read APIs and Trial APIs.
2. Assign stable, action-oriented Trial codes to the required failures:

   | HTTP | code | Existing error semantics |
   | ---: | --- | --- |
   | 401 | `trial_token_invalid` | a valid runtime bearer token is required |
   | 403 | `trial_origin_not_allowed` | the request Origin is not admitted |
   | 409 | `trial_workspace_conflict` | workspace/session state prevents the operation |
   | 412 | `trial_confirmation_stale` | the Gate 1 card changed |
   | 428 | `trial_confirmation_required` | the Gate 1 hash is missing |
   | 503 | `trial_execution_disabled` | Trial execution is not configured |

   Other server failures also receive codes so the same client path can handle
   read-only pages and CLI startup failures.
3. Add `gui/lib/errors.ts` with a typed HTTP error parser and the shared
   `describeError` function. Descriptions retain diagnostic detail but lead to
   the next action: reload/re-authenticate, configure
   `GUI_TRIAL_ALLOWED_ORIGINS`, re-check Gate 1, configure the execution root,
   verify `--commandagent-bin`, recover the workspace, or retry a read.
   Browser/network rejections are normalized and never expose the browser's
   raw fetch wording.
4. Route every explicit GUI fetch failure through the shared parser and
   descriptor. For an active-session 409, extract the UUID from the compatible
   existing `error` text and render a real reconnect link that resumes GET-only
   polling of that session.
5. Extend `tests/gui_server.rs` to assert both the unchanged `error` value and
   the new `code` for 401/403/409/412/428/503. Add a read-error assertion and a
   source guard proving fetch callers use the common descriptor and the raw
   browser wording is absent.
6. Add a focused deterministic Playwright smoke script. It uses a sleeping
   fake CLI delegate to verify wrong-token guidance, a rewritten Origin header,
   and a real running-workspace 409 with session ID plus reconnect link, without
   requiring a model call.

## Verification plan

- Rust integration tests for `gui_server` and the GUI source guard.
- GUI dependency install, lint, production build, and the focused Playwright
  error smoke.
- Repository-wide formatting, Clippy, and test suite because the shared Rust
  HTTP error contract is touched.
- A source grep confirming the raw browser network wording is absent.
