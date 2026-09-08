# Evaluation Browser preflight

Use this opt-in gate for **new** local GUI evaluation campaigns. It does not alter
the frozen 0908 harness or automatically gate other existing launch paths. Python
3.10+, Pillow, and (for the authorized fallback) Playwright with an explicitly
selected installed Chrome/Chromium executable are required. No dependency, plugin,
extension or browser is installed by these scripts.

## Request

Save a request JSON before performing operations:

```json
{
  "session_id": "current-conversation-id",
  "method": "isolated-playwright",
  "target_url": "http://127.0.0.1:8080/",
  "authorization": "Owner approved this local page, Check browser button and screenshot",
  "isolated_fallback_authorized": true,
  "executable": "/absolute/path/to/installed/chrome",
  "action": {
    "role": "button",
    "name": "Check browser",
    "before": "Ready for browser check",
    "after": "Browser action confirmed"
  },
  "conditions": {
    "headless": false,
    "viewport": {"width": 1000, "height": 800},
    "locale": "en-US",
    "timezone": "Asia/Tokyo"
  }
}
```

Inspect the target and choose a harmless, visible action with a verifiable effect.
The adapter clicks that exact role/name once. Do not name generation, submission,
payment or destructive controls as the preflight action. The isolated fallback is
restricted to explicitly authorized, unauthenticated loopback targets. It creates
its own temporary profile; it never reads/copies a user profile or attaches to an
existing Chrome process. It closes only the browser it launches. Redirects away
from the exact requested URL fail validation. Avoid putting secrets into requests
or page fixtures because the page text and images become evidence.

```sh
python3 scripts/browser-preflight.py probe --request request.json --ledger session-ledger
```

The report records the actual Playwright/browser versions and environment, plus
the URL, before/after text, image format/dimensions/hash and cleanup result. HTTP
success and installed files do not substitute for those capabilities.

## Existing Browser connection

Read the current session's Browser skill first. Reserve an attempt with `prepare`
before any bootstrap or browser operations. Preserve an existing healthy binding;
only if none exists, bootstrap once using that skill's exact current entry point.
Read the selected browser's complete documentation before invoking the adapter.
If bootstrap/module loading fails, record that ticket's `ticket_id`, failing
`stage` and exact `error` with `record` before requesting another attempt. The
same durable retry limit applies to setup failures and later operation failures.
Use `method: "browser-plugin"` and supply an instruction attestation:

```json
{
  "session_instruction": {
    "session_id": "current-conversation-id",
    "skill_path": "/exact/current/plugin/skills/control-in-app-browser/SKILL.md",
    "sha256": "SHA256 of that session's skill file",
    "entry_point": "/exact/current/plugin/scripts/browser-client.mjs"
  }
}
```

Take the path from the current session's supplied instructions, never from a
historical run or a highest-version directory search. The gate checks the source
hash, expected relative entry point, file existence and entry hash. Installed
versions are diagnostic inventory only. Missing source provenance remains unknown.
The attestation describes what instructions this conversation received; a local
script cannot independently authenticate the conversation's tool configuration.

Run `prepare --request request.json --ledger session-ledger`. Only when its
decision is `probe` and `operation_allowed` is true, invoke the adapter through
the supported Browser Node tool:

```js
const { probeBrowser } = await import("/absolute/repo/scripts/browser_preflight/plugin_probe.mjs");
const observation = await probeBrowser(browser, "/absolute/path/to/ticket.json", observedConditions);
```

Pass the **existing** browser binding. This module never imports a plugin,
initializes a connection, selects a browser, resets a session, or closes user
tabs. Empty tab lists are recorded and a task-owned tab is created from the same
connection. It closes only that tab. Obtain `observedConditions` from supported,
verified environment observations; do not guess them from installed files. All
six environment fields shown in the report are required for freezing, including
`browser_version` and `automation_version`. If the supported Browser surface
cannot expose them, leave them unknown: successful page operations can still be
recorded, but that method cannot be frozen for a campaign.

Then record its saved receipt:

```sh
python3 scripts/browser-preflight.py record --ticket /absolute/path/to/ticket.json --observation /absolute/path/to/observation.json
```

`prepare` returns `reuse` for a fresh healthy result; do not invoke any adapter or
reinitialize the connection in that case. Each adapter refuses to execute the
same ticket twice. A pending attempt blocks duplicate attempts. If interrupted,
record an observation containing its `ticket_id`, `stage` and exact `error` before
any retry. Keep the same ledger across process restarts in this conversation.
Two failed attempts with the same conditions suppress further attempts, even if
other conditions were tried between them. Changed conditions require `--reason`;
the previous/current fingerprints and reason are stored. A reason alone does not
reset the limit. For recovery under otherwise identical configuration, supply
`recovery_evidence` pointing to a new evidence file and explain the change. Only
its content hash changes the fingerprint. Never manufacture evidence just to
obtain another attempt. Other-session success is recovery evidence, not proof
of a global outage or a permanently repaired plugin.

## Freeze and start

```sh
python3 scripts/browser-preflight.py freeze --request request.json --report /absolute/path/to/report.json --campaign new-campaign --campaign-id unique-campaign-id
python3 scripts/browser-preflight.py gate --request request.json --manifest new-campaign/browser-manifest.json
python3 scripts/browser-preflight.py launch --request request.json --manifest new-campaign/browser-manifest.json -- your-authorized-evaluation-command
```

`launch` recomputes capability validation, verifies the latest session result,
artifact hashes, current request and dependency fingerprints, and requires evidence
no older than 15 minutes. It reserves the campaign start once before executing the
supplied command without a shell. Nonzero workload exits propagate. An interrupted
or already started campaign cannot launch again. Keep later browser operations
under the frozen conditions; changes require another campaign directory/ID with
fresh preflight and a saved reason. This wrapper does not instrument arbitrary
commands or detect condition changes concealed by a caller after launch.

Receipts and manifests are local evaluator evidence, not signed security
attestations. Report/manifest/artifact hash validation detects accidental changes;
it does not defend against a caller intentionally rewriting the entire evidence
and ledger. The runtime ledger and `.lock` files should remain local and unstaged;
retain immutable tickets, reports, screenshots and campaign manifests for review.

## Verification and owned GUI smoke

```sh
python3 -m pytest scripts/tests/test_browser_preflight.py -q
node --test scripts/tests/test_browser_plugin_probe.mjs
python3 scripts/browser-preflight-smoke.py run --output /new/task-owned/smoke-directory --executable /absolute/path/to/chrome
```

The smoke uses a small owned loopback GUI fixture, clicks once, saves a screenshot,
checks healthy reuse and a denied changed-target launch, then launches a harmless
Python command through the successful gate. No model generation is started. The
server/browser are cleaned up in `finally`. `serve --output /new/directory` keeps
only this fixture server alive for plugin smoke; stop that owned process with
Ctrl-C after closing its task-owned tabs. `--headless` is an explicit alternative
condition and must use a new smoke/campaign output directory.

Automatic diagnosis and retry suppression are implemented here. Plugin-internal
permanent repair and lifecycle/update recurrence testing are **not established**.
