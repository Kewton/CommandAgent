# GUI Trial

[GUI index](gui.md) | [Getting started](getting-started-gui.md) |
[History](gui-history.md) | [Operations](gui-operations.md)

The Trial page delegates the existing non-interactive `commandagent` product
binary only after an exact Gate 1 confirmation. The GUI server does not call a
provider or runner in process, and it does not maintain a separate run-state
database.

## Trial run: Gate 1 through Gate 3/4

Open **トライアル** and enter a goal, admitted or extension-root draft
profile, provider, and exact executor/planner model IDs. Goal and model fields
start empty so a demo request cannot be delegated accidentally.

The browser obtains profiles/providers from `GET api/trial-options` and
admitted plus conformant pinned local packs from `GET api/pack-options`.
External profile rows are labeled **下書き**, show their exact-byte manifest
hash and `保証上限 static`, and fix the pack selector to **選択なし**. An
additive overlay also names its admitted base. Changing a provider never
rewrites either model pin.

The compact indicator is **依頼 → 確認 → 実行 → 結果**. Only the current
workflow state is shown; a completed form is not left stacked above the next
action.

### Pack selection and frozen identity

For a compatible admitted profile/intent, the selector shows exact
`id@version` pack choices and their supply source. A selected pack adds its
version, exact-byte hash, injection point, and source to Gate 1. Changing the
pack invalidates the card hash and requires a fresh confirmation. Draft
profiles cannot select packs and never inherit admission from local supply.

The **LM Studio** choice maps to CLI `--provider lm-studio`. Enter the exact
model identifier exposed by the server. The delegate uses the CLI's host
option and inherits `LM_STUDIO_API_TOKEN` only when configured in the GUI
server environment.

### Gate 1: confirm before execution

1. When token authentication is on, enter the runtime Trial token.
2. Select **契約と見積りを確認**. Browser validation runs before the proposal
   request. No child process is started.
3. Read the server-rendered card: exact identity, required checks, comparable
   successful runs, measured mean/cost where recorded, and canonical write
   boundary. A draft profile also shows its source/path/hash and
   `profile_not_admitted` assurance cap.
4. Select the confirmation checkbox. **確認して CLI を実行** stays disabled
   until this explicit action, and the API independently requires the exact
   card hash.

GUI confirmation never lowers, replaces, or satisfies a required check.

### Gate 2: execution and monitoring

The server starts `commandagent` directly without a shell. Progress is rebuilt
from the session JSONL. Launch identity fields remain read-only so an in-flight
contract cannot be edited.

Execution state and monitoring health (`connected`, `degraded`, or `lost`) are
separate. Transient monitoring failures use capped backoff while the delegated
CLI may continue. The browser elapsed clock is an observation, and the
measured mean shown beside it is a comparison, not an ETA. `Phase x / N` is
shown only when file-backed phase evidence has a nonzero total.

Use recent events and artifact browsing for bounded, read-only evidence. There
is no cancel, interrupt, phase-edit, or gate-override control in the GUI.

### Gate 3/4: read the result

At terminal state the inventory opens automatically. Read result, assurance
level, and execution status as separate fields. If no final verdict exists,
the page says so; an assurance identifier such as `static` is not substituted
for the verdict.

Inspect `summary.md`, the event tail, and acceptance-related text artifacts.
You may **追加の依頼を確認用に準備**; the directive is credential-scrubbed,
exact-byte hashed, displayed, and separately confirmed, and cannot lower fixed
checks. Or end without another run and return to an editable draft. The prior
proposal/session/directive is cleared while tab-scoped launch inputs remain.

Gate 4 is an honest failure or missing-evidence outcome. Preserve its evidence
and follow a displayed recovery path; do not weaken verification to turn it
into Gate 3.

## Reconnect monitoring

The launched session ID, never the token, is placed in `?session=<id>`. A
same-tab reload restores a tab-scoped token and **Reconnect monitoring** calls
only `GET api/sessions/{id}`. The elapsed clock resumes from the server-owned
session start and the measured mean is restored from the confirmed band, so
neither value resets after reload. Reconnect cannot delegate another process.
A workspace 409 response supplies the same session link.

Monitoring failures have explicit boundaries:

- 401 with definitive `trial_token_invalid` asks for a new runtime token and
  removes only the rejected tab value.
- 403 asks the operator to verify the allowed Origin.
- an upstream manual redirect requires proxy re-authentication and reload.
- a fetch failure requires checking the proxy/network while assuming the CLI
  may still run.

HTTP 413 and invalid event JSONL stop only after their bounded terminal-error
limit. Other transient status failures continue at capped intervals. The token
is never stored in `localStorage`, included in a URL, or compiled into export
assets.

## Workspace lease inspection and recovery

The Trial page's **ワークスペースのリースを確認** action performs only
authenticated `GET api/trial-workspace`. It reports `Idle`, `Running`, or
`Recovery required` and includes the exact session ID for non-idle states. The
card cannot clear the lease, dispatch a process, or modify artifacts.

If the configured binary cannot be spawned, HTTP 500 names its path and the OS
cause. Because no child exists, the server rolls back that new session and
releases the lease. Fix `--commandagent-bin` and retry.

`Recovery required` means a confirmed child may have existed but no current
terminal event was observed. Treat it as a possible live process:

1. Record the session ID and stop `gui_server` so no new Trial is admitted.
2. Use operating-system process inspection to verify that no delegated
   `commandagent` remains for the execution root and the session state path. If
   one runs, do not clear or archive the lease; use existing CLI/runtime
   procedures and inspect events again.
3. When no child remains, preserve the complete `.anvil/runs/<session-id>`
   directory in an operator-chosen archive outside the execution root. Move it
   as one unit; do not delete confirmation files and do not append a synthetic terminal event.
4. Restart with the same execution root, re-enter the token, and inspect the
   lease. It must report `Idle` before another Trial is launched. Repeat for
   any other exact unfinished session instead of bypassing the lease.

The archive remains evidence for the incomplete run. Restoring it under
`.anvil/runs` intentionally makes startup require recovery again.
