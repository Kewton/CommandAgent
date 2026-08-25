# GUI Trial

[GUI index](gui.md) | [Getting started](getting-started-gui.md) |
[History](gui-history.md) | [Operations](gui-operations.md)

The Trial area delegates the existing non-interactive `commandagent` product
binary only after an exact Gate 1 confirmation. The GUI server does not call a
provider or runner in process, and it does not maintain a separate run-state
database. Trial has four fixed pages; `base-path` may be `/` or a configured
proxy prefix such as `/proxy/commandagent/`.

| Page | Fixed route | Responsibility |
| --- | --- | --- |
| 実行指示 | `try/` | Create a request and complete Gate 1. |
| 実行状況 | `try/status/?session=<id>` | Reconnect to and observe one in-flight session. |
| 実行履歴 | `try/history/` | Scan compact session summaries. |
| 結果詳細 | `try/history/detail/?session=<id>` | Read one terminal verdict, diagnosis, acceptance, events, and artifacts. |

Each page has its own title, heading, and active Trial navigation item. The
execution-instruction page does not embed the history list, so the current
task and Gate 1 remain the only primary actions there.

## Trial run: Gate 1 through Gate 3/4

Open **トライアル** and enter a goal, admitted or extension-root draft
profile, execution intent, execution/planning providers, and exact
executor/planner model IDs.
Goal and model fields start empty so a demo request cannot be delegated
accidentally.

**実行目的** offers **自動判定**, **作成**, **修正**, and **調査**. Automatic
detection preserves the historical request-word inference path. The other
choices send the typed `create`, `fix`, or `investigate` value to Gate 1;
request wording cannot replace that explicit choice. Gate 1 displays and
freezes the effective value before delegation to CLI `--intent`.

The browser obtains profiles/providers from `GET api/trial-options` and
admitted plus conformant pinned local packs from `GET api/pack-options`.
External profile rows are labeled **下書き**, show their exact-byte manifest
hash and `保証上限 static`, and fix the pack selector to **選択なし**. An
additive overlay also names its admitted base. **実行プロバイダー** maps to
CLI `--provider`, while **計画プロバイダー** independently maps to
`--planner-provider`; the adjacent model fields map to `--model` and
`--planner-model`, respectively. The **Executor / 実行** and **Planner / 計画**
groups each keep Provider and Model on one desktop row. On mobile, each Model
follows its Provider before the next role begins. Changing either provider
never rewrites its model ID. For Ollama and LM Studio, each model input obtains
candidates using its own selected provider, while exact IDs can still be
entered manually.

**Ollama thinking** is optional and maps to one exact CLI argument:
`--think=<value>`. Accepted values are `true`, `false`, `low`, `medium`, and
`high`. The selector is available only while the execution or planning role
uses Ollama; removing the final Ollama role clears and disables it. The server
also rejects a selected value with HTTP 422 when neither role is Ollama. An
unspecified value adds nothing to the Gate 1 identity or delegated arguments,
so existing confirmation hashes and records remain compatible.

The Gate indicator is **依頼 → 確認 → 実行 → 結果**. The separate Trial
navigation keeps page purpose visible while the indicator describes the
selected session's lifecycle. Only the current workflow state is shown; a
completed form is not left stacked above the next action.

### Pack selection and frozen identity

For a compatible admitted profile/intent, the selector shows exact
`id@version` pack choices and their supply source. A selected pack adds its
version, exact-byte hash, injection point, and source to Gate 1. Changing the
profile or intent clears an incompatible pack as well as the proposal and
confirmation check. Changing the pack invalidates the card hash and requires a
fresh confirmation. A pack handoff from **拡張** selects its matching profile
and intent, including compatible fix/investigate packs. Draft profiles cannot
select packs and never inherit admission from local supply.

The **LM Studio** choice maps to `lm-studio` for the corresponding
`--provider` or `--planner-provider` CLI flag. Enter the exact model identifier
exposed by the server. The delegate uses the CLI's host option and inherits
`LM_STUDIO_API_TOKEN` only when configured in the GUI server environment.

### Gate 1: confirm before execution

1. When token authentication is on, enter the runtime Trial token.
2. Select **契約と見積りを確認**. Browser validation runs before the proposal
   request. No child process is started.
3. Read the server-rendered card: exact identity, required checks, comparable
   successful runs, measured mean/cost where recorded, and canonical write
   boundary. A draft profile also shows its source/path/hash and
   `profile_not_admitted` assurance cap. A selected Ollama thinking value is
   shown on this card and becomes part of its confirmation ID.
4. Select the confirmation checkbox. **確認して CLI を実行** stays disabled
   until this explicit action, and the API independently requires the exact
   card hash. An accepted launch moves to that session's **実行状況** page.

GUI confirmation never lowers, replaces, or satisfies a required check.

### Gate 2: execution and monitoring

The server starts `commandagent` directly without a shell. The **実行状況**
page is read-only, and progress is rebuilt from the session JSONL. Launch
identity fields remain read-only so an in-flight contract cannot be edited.
The Gate 2 screen keeps the confirmed goal, profile, exact executor/planner
provider and model IDs, and exact `id@version` pack (or `選択なし`) visible
above the progress. A selected Ollama thinking value is also shown. The same
frozen identity is restored from the Gate 1 confirmation record after
reconnecting.

Execution state and monitoring health (`connected`, `degraded`, or `lost`) are
separate. Transient monitoring failures use capped backoff while the delegated
CLI may continue. The browser elapsed clock is an observation, and the
measured mean shown beside it is a comparison, not an ETA. `Phase x / N` is
shown only when file-backed phase evidence has a nonzero total.

When the session contains the typed StepPlan lifecycle events introduced by
#375, the status page also shows the current phase, current task ID, and
`task / total` position. Tasks are grouped by `plan_execution_id`, so an
initial run and a confirmed additional request remain separate even when they
reuse the same Step ID. The task view derives outcomes only from matching
`plan_step_started`, `plan_step_completed`, and `plan_step_failed` records; a
later event or phase completion is never treated as proof that a task passed.

The confirmed intent is shown with the other launch identity fields. It is not
editable after Gate 1, and reconnect restores it from the persisted
confirmation rather than re-running request inference.

Use recent events and artifact browsing for bounded, read-only evidence. There
is no cancel, interrupt, phase-edit, or gate-override control in the GUI.

### Working directory and run records

As soon as a launch reaches **実行状況**, the page shows the absolute
**CLI 作業ディレクトリ** used for both the delegated process `current_dir`
and its explicit `--cwd` argument. The same panel remains on terminal
**結果詳細** and is restored when that result is opened from history. Use
**パスをコピー** to put the exact value on the clipboard; the button supports
normal keyboard activation and announces success or failure to assistive
technology.

Do not confuse this directory with the adjacent **実行記録の保存先**.
Generated code and execution targets belong below
`<execution-root>/sessions/<session-id>/`. CLI-owned records such as
`events.jsonl` and `summary.md` belong below the separately displayed run
record directory. If the working directory was removed after the run, the
panel keeps the historical path visible but marks it **削除済み** and states
that generated code or execution targets are no longer present.

Absolute paths are intentionally absent from create/status/index, public
artifact/event, runtime-status, and static projections. Only the GET-only
`api/sessions/{id}/paths` endpoint returns them. When Trial token
authentication is enabled, a valid token is required. When an administrator
starts the server with Trial token authentication disabled, that startup
configuration is trusted and the same endpoint is readable without a bearer
token. Invalid IDs, traversal forms, symlinks, and paths resolving outside the
configured execution root are rejected in either mode.

### Gate 3/4: read the result

At terminal state the browser moves from **実行状況** to the session's
**結果詳細** page. Read result, assurance level, and execution status as
separate fields. If no final verdict exists, the page says so; an assurance
identifier such as `static` is not substituted for the verdict. The result
card repeats the run's confirmed goal, profile, model pins, and pack so the
outcome remains bound to the execution it describes.

The result card also shows a **フェーズ別タイムライン** with each recorded
phase's start time, end time, and duration, plus the terminal command's total
processing duration. Boundary timestamps are recorded for new runs. Older
sessions keep unavailable boundary values as **未記録** rather than inferring
times from file metadata or event order; their total is still shown when the
terminal time profile recorded it.

The result detail lists every task represented by a typed lifecycle interval.
`completed`, `short-circuited`, `FAILED`, and `interrupted` each have a symbol
and visible text label. FAILED tasks open automatically and include the bounded
failure summary, verification failures, changed paths, and an `events.jsonl`
evidence action. If a Plan reports more tasks than have typed events, the page
shows the unrecorded count without guessing whether those tasks ran. Sessions
that ended before the #375 contract are marked `unsupported` and show no
invented success count.

For a failed Gate 4 terminal, **失敗した場所、原因、次の操作** is a typed,
bounded explanation of the final execution interval. The five sections are:

1. the one-based execution interval, exact typed Plan/Step execution IDs,
   phase, and task;
2. the primary cause, classified as planning, execution, verification,
   release gate, infrastructure, interrupted, or unknown;
3. bounded command/exit/output, verification, acceptance, probe, missing-path,
   changed-path, and evidence-path observations;
4. completed phase/task progress, repair attempts, and the Issue #374 working
   directory/partial-artifact state; and
5. only the recovery actions recorded by `recovery_prompt_saved`, including
   viable actions, repair prompt, Recovery Plan, suggested command, suggested
   YAML command, and continuation eligibility.

The projection starts after the latest confirmed additional-request boundary.
It therefore never carries a failed initial interval into a successful
continuation. Exact Issue #375 step fields are accepted only as a matching
schema-v1 started/terminal pair; incomplete or older records display an
`unknown` fallback instead of guessed task details. Each free-form string and
list has an independent cap and truncation marker. A truncated command remains
visible as evidence but cannot be copied as a complete recommendation.
An explicitly failed recovery prompt/YAML parse, missing recovery artifact, or
invalid command target makes continuation ineligible rather than optimistic.

**repair prompt を開く** and **Recovery Plan を開く** use the GET-only
`api/sessions/{id}/recovery-document` endpoint. When Trial token authentication
is on, the valid token remains mandatory. When an administrator starts the
loopback server with authentication off, the documents are readable without a
bearer token. In either mode, the endpoint reads only the exact non-truncated
path projected from the current terminal interval, below the available
per-session working directory; traversal, symlinks, unrelated paths, and a
missing workspace are refused. The reader does not create, rewrite, or execute
a recovery artifact. After a successful open, the page scrolls the document
viewer into view, moves keyboard focus to it, and announces the opened file name
with **文書を開きました**. A failed read shows the existing error and does not
announce success or move focus.

The command copy buttons are keyboard-operable and announce that no execution
occurred. **推奨内容を追加の依頼欄へ反映** only prefills and focuses the
existing additional-request textarea. It does not save, confirm, dispatch, or
run recovery. Credential scrubbing, exact-byte directive display, explicit
confirmation, fixed checks, and the existing Gate 1/continuation boundary
still apply.

Inspect `summary.md`, the event tail, and acceptance-related text artifacts.
You may **追加の依頼を確認用に準備**; the directive is credential-scrubbed,
exact-byte hashed, displayed, and separately confirmed, and cannot lower fixed
checks. Or end without another run and return to an editable draft. The prior
proposal/session/directive is cleared while tab-scoped launch inputs remain.

Gate 4 is an honest failure or missing-evidence outcome. Preserve its evidence
and follow a displayed recovery path; do not weaken verification to turn it
into Gate 3.

## Reconnect monitoring

New in-flight links use `try/status/?session=<id>` and terminal links use
`try/history/detail/?session=<id>`. The launched session ID, never the token,
is placed in the query string. A legacy `try/?session=<id>` deep link reads the
session and replaces itself with the correct status or detail route.

A same-tab reload restores the base-path-scoped `sessionStorage` token and
**Reconnect monitoring** calls only `GET api/sessions/{id}` plus the dedicated
authenticated `GET api/sessions/{id}/paths` projection. A separately
opened tab can reconnect after entering its own runtime token. The elapsed
clock resumes from the server-owned session start and the measured mean is
restored from the confirmed band, so neither value resets after reload.
Reconnect cannot delegate another process. A workspace 409 response supplies
the same status link.

Task polling uses the same conditional session response and returns only the
bounded typed projection, not raw events. Unchanged polls remain body-free
304 responses. Task disclosures are keyboard-operable, synchronize
`aria-expanded`, and keep state labels independent of color.

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
3. When no child remains, preserve the complete `.commandagent/runs/<session-id>`
   directory in an operator-chosen archive outside the execution root. Move it
   as one unit; do not delete confirmation files and do not append a synthetic terminal event.
4. Restart with the same execution root, re-enter the token, and inspect the
   lease. It must report `Idle` before another Trial is launched. Repeat for
   any other exact unfinished session instead of bypassing the lease.

The archive remains evidence for the incomplete run. Restoring it under
`.commandagent/runs` intentionally makes startup require recovery again. A
legacy session restored under `.anvil/runs` remains readable and has the same
recovery behavior.
