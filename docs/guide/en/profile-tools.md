# Operator Root, Profile Tools, and Run Budget

[日本語](../ja/profile-tools.md) | [Guide index](../README.md)

Three features read files that you, the operator, keep outside the workspace:
the **operator root**, **profile tools**, and the **run budget**. A model, a
profile, or a workspace file cannot create or change what lives there. This page
shows what to put in those files and how to read the result.

## Operator Root

The operator root is one absolute directory named by the environment variable
`COMMANDAGENT_ADAPTER_CONFIG_ROOT`. It must lie outside the workspace.

```bash
export COMMANDAGENT_ADAPTER_CONFIG_ROOT=/operator/config
```

The same root holds the declarations of more than one feature. Each feature has
its own directory and its own protocol; the root shares a place, not a format.

| Path under the operator root | Holds | Protocol |
| --- | --- | --- |
| `profile-tools/<id>.json` | One descriptor per profile tool | `profile-tool.descriptor/v1` |
| `run-budgets/<id>.json` | One declaration per run budget | `run-budget.policy/v1` or `run-budget.policy/v2` |

- With the variable unset, or set to a relative path, a feature that needs the
  root is refused before the run sends anything.
- The files are read without following a symbolic link below the root. A file
  that resolves into the workspace is refused.
- A run budget declaration is also refused when a command of the run could write
  its location.

## Profile Tools

A profile tool is an opaque tool ID that a profile or an eval pack may reference.
The profile or pack says which IDs it uses; you say, in the operator root, what
each ID runs. Only what you pinned reaches the model.

### Declare tools in a profile or pack

A profile manifest with `schema_version = "v2"`, and an eval pack, may carry a
`[profile_tools]` table. A v1 manifest, an overlay, and an assist pack cannot.
Without the table the feature is off and nothing changes.

```toml
[profile_tools]
schema_version = "profile-tool.declaration/v1"
builtin_tools = ["Read", "Grep"]
tools = [
  { id = "config-json-propose", effect = "propose-artifacts", max_output_bytes = 4096 },
]
```

- `builtin_tools` is required. It lists the built-in tools that stay available;
  a submission-only profile leaves out `Write`, `Edit`, and `Bash`.
- A declaration can only remove or narrow. It cannot name an executable, an
  argument list, a hash, or a permission, and it cannot add back a built-in tool
  or widen a limit.
- When both a profile and its selected pack declare, the built-in set is their
  intersection, and a tool ID that appears in both is refused.

### Write the descriptor

Create `<operator root>/profile-tools/<tool id>.json`. This descriptor runs one
pinned process that proposes bytes for one declared output. The host checks the
bytes before it adopts them.

```json descriptor-example
{
  "protocol": "profile-tool.descriptor/v1",
  "tool_id": "config-json-propose",
  "name": "ProposeConfigJson",
  "version": "1.0.0",
  "description": "Propose the contents of the configuration file.",
  "effect": "propose-artifacts",
  "parameters": [],
  "schema": {
    "protocol": "profile-tool.schema/v1",
    "root": {
      "type": "object",
      "additionalProperties": false,
      "required": ["mode"],
      "properties": {"mode": {"type": "string", "enum": ["strict", "lenient"]}}
    }
  },
  "schema_sha256": "<sha256 of the compact JSON of schema, keys sorted>",
  "runtime": {"kind": "pinned_process", "id": "config-proposer"},
  "process": {
    "protocol": "profile-tool.process/v1",
    "executable": {"path": "/operator/tools/propose", "sha256": "<sha256 of the file>"},
    "dependency_directories": [{"path": "/operator/tools", "sha256": "<directory pin>"}],
    "argv": ["/operator/tools/serializer.js"],
    "cwd": "/operator/tools",
    "env": {"TOOL_MODE": "strict"},
    "limits": {"timeout_ms": 30000, "max_result_bytes": 65536},
    "outputs": [{"id": "artifact", "path": "config/settings.json"}]
  },
  "limits": {"max_input_bytes": 1024, "max_output_bytes": 4096}
}
```

- The two `sha256` values and the directory pin are placeholders. Compute them
  from the real files; a descriptor whose pin does not match is refused as
  `invalid_pin`.
- Every pinned path is absolute, canonical, and outside the workspace.
- The environment of the process is exactly the keys you declare. Nothing is
  inherited, and keys that a loader reads as a search path or as code are refused.
- `outputs` holds exactly one entry. `path` is workspace-relative; the process
  never sees it and cannot choose it.
- The pins are compared again right before every execution. A mismatch ends the
  run with the tool not executed.

### What the command-line program can run

- A `pinned_process` tool with the `propose-artifacts` effect runs through the
  `commandagent` binary. It needs a macOS Seatbelt sandbox that passes its
  probe; elsewhere the tool is refused as `unavailable` before the run starts.
- A tool whose runtime is `in_process` needs a runtime that a program embedding
  CommandAgent registers. The binary registers none, so such an ID is refused as
  `unavailable`.
- A tool result never satisfies a required artifact or a stage by itself. The
  acceptance gates decide as before.

### When a tool is refused

Admission refusals happen at start-up, before the first provider call. The
message has the form `profile_tool_refused: <kind>: <reason>`, and the command
ends with exit code 2.

The pins are also compared again right before each execution. If a pinned file
changed after admission, that check refuses the call at run time, when the model
calls the tool: the tool is not executed and the run ends. This refusal comes
after provider calls have already been made.

| Kind | Typical cause |
| --- | --- |
| `registry_unavailable` | The operator root is unset or cannot be resolved |
| `unknown_tool_id` | No descriptor is registered for the ID |
| `invalid_descriptor` | The descriptor does not decode or validate |
| `invalid_pin` | The schema, the executable, or a dependency does not match its pin |
| `workspace_descriptor` | The descriptor or a dependency resolves into the workspace, or the descriptor is a symbolic link |
| `unavailable` | A required capability, such as the sandbox or a runtime, is not available |

The full list of kinds, the schema subset, and the limits are in the developer
document `docs/dev/profile-tools.md`.

## Structured Stages

A profile manifest or an eval pack can order its `propose-artifacts` tools as
stages. A stage is complete only when, for its current inputs, the host adopted
the bytes and every check you require passed. What a model or a tool says never
decides this. For a walkthrough over a working sample, see
[Build a Two-Stage Extension](operator-root-tutorial.md).

### Declare stages

Stages need `profile-tool.declaration/v2`:

```toml
[profile_tools]
schema_version = "profile-tool.declaration/v2"
builtin_tools = ["Read"]
tools = [
  { id = "example-parse", effect = "propose-artifacts" },
  { id = "example-check", effect = "propose-artifacts" },
]

[profile_tools.stages]
stages = [
  { id = "parse", targets = [{ id = "main", tool_id = "example-parse" }] },
  { id = "check", targets = [{ id = "main", tool_id = "example-check" }] },
]
```

- An edge between stages comes from the schemas: a tool whose `schema.inputs`
  names another tool's output depends on that tool's stage. A stage can also list
  `depends_on`.
- A stage can add `required_checks`. Limits under
  `[profile_tools.stages.limits]` can only narrow.
- An eval pack carries the same fields in YAML under `profile_tools.stages`.

Each stage tool's descriptor carries your floor. Without it the tool is refused
at admission as `stage_contract_refused`:

```json
"stage": {
  "required_checks": ["lines"],
  "attempts_per_target": 3,
  "attempts_per_run": 8,
  "deadline_seconds": 600
}
```

The tool's schema must take a required string argument, `expected_revision`.
The host keeps it out of what the process is asked.

### What the model is told

Each call to a stage tool is answered with data the model reads. Every answer
carries `attempts`, the counts used and allowed for the target and for the run.

| `status` | Meaning | What the model does next |
| --- | --- | --- |
| `sealed` | The bytes were adopted and every required check passed. `stage.downstream[].revision` names the revision each stage below now expects | Calls the next stage with that revision |
| `resubmit` | A check failed (`code: check_failed`), with the process's diagnostics. What was sealed stays sealed | Calls the same tool again |
| `stale` | The revision is older, or another call in the reply committed. `revision` is the current one | Calls again with `revision` |
| `not_ready` | A stage above is not sealed | Seals that stage first |
| `unknown_revision` | The revision is not one of this run. `initial` names a target that never changed | Uses a revision the host named |
| `no_change` | The same call on bytes already sealed | Moves on |
| `input_not_current` | An input is not what the stage above adopted | Calls again once the stage above is sealed |

These end the run instead: the deadline passed, a target's or the run's
attempts are spent, a fixed input changed, the process exited non-zero, or it
reported no check at all.

### How a run ends

- A `--prompt` run that wants to end with a stage not sealed is sent back, at
  most 8 times, and otherwise ends as `stage_incomplete`. Exit 0 means every
  declared stage is sealed.
- The record lives below `--state-dir`, outside the workspace, one per workspace
  and profile. A new process starts the counts and the deadline over;
  `--resume` continues them and `--fresh-session` starts over.
- A stage set needs the OS sandbox: `--sandbox off` is refused as
  `stage_contract_refused`. So is `--recovery-plan-auto-runs` above 0.
- `--summary-json` reports the outcome in the `structured_stages` object
  described below.

## The Process Envelopes

A `pinned_process` tool talks to the host through two envelopes. The host
writes one request line to the process's standard input and reads one result
document from its standard output.

### The request

`profile-tool.request/v1` is one line of compact JSON:

| Field | Holds |
| --- | --- |
| `request_id` | A new ID for this call |
| `tool_id`, `tool_version` | The descriptor's `tool_id` and `version` |
| `descriptor_sha256`, `runtime_sha256` | The digests of the descriptor and of its process declaration |
| `input_revision`, `schema_sha256` | The revision of the adopted inputs and the schema's pin |
| `arguments` | The validated arguments, without `expected_revision` |
| `inputs` | Each adopted input as `path`, `sha256`, and `bytes_base64` |
| `outputs` | The one output's `id` |

The inputs are the bytes the host adopted, never read again from a path.

### The result

`profile-tool.result/v1` is one JSON document:

```json
{"protocol": "profile-tool.result/v1", "request_id": "...", "tool_id": "...",
 "runtime_sha256": "...", "input_revision": "...", "schema_sha256": "...",
 "checks": [{"id": "lines", "status": "passed"}],
 "outputs": [{"id": "items", "sha256": "...", "bytes_base64": "..."}],
 "diagnostics": [{"code": "bad_line", "message": "line 2 is not name,quantity"}]}
```

It is accepted only if:

- the process exited 0, took the whole request, and left nothing behind;
- no field is unknown, missing, or repeated, and each binding field equals the
  request's;
- there is at least one check, with distinct IDs, each `passed`;
- there is exactly the declared output, once, in canonical base64, with a
  `sha256` equal to the bytes.

A failed check comes back with exit 0; in a stage it is returned to the model as
`resubmit`. A secret value of the run found anywhere in the output ends the call
as `secret_value_in_output`, and nothing of the output is kept.

### The sandbox

| Limit | Value |
| --- | --- |
| Policy | `deny default` Seatbelt, macOS only, after the probe passes |
| Processes | No fork. Exec of the pinned executable only |
| Reads | The executable, each dependency directory, system libraries, and a few device files |
| Writes | `/dev/null` only |
| Network | None, loopback and unix sockets included |
| Environment | Exactly the declared keys (at most 16). Keys a loader or shell reads as code or a search path are refused |
| Time | 30 s from spawn, or the descriptor's narrower `timeout_ms` |
| Result | 64 KiB of stdout and stderr together, or the descriptor's narrower `max_result_bytes` |
| Diagnostics | At most 8, code at most 64 bytes, message at most 512 bytes |

On macOS, `/bin/bash` works as the pinned executable. `/bin/sh`,
`/usr/bin/python3`, and `/usr/bin/perl` are launchers that exec another file,
which this policy refuses, so the process fails. A shell script can use
builtins only; [What Bash may do in the sandbox](operator-root-tutorial.md#what-bash-may-do-in-the-sandbox)
lists them. The executable and every dependency directory must be owned by you
or root, with no group or other write.

## Run Budget

A run budget is a wallet of limits for the provider calls of one run. Before
each send the host reserves the worst case on disk. A send that would pass a
limit does not go out, and a send whose cost cannot be counted stops the run.
The operator declares the wallet; the run only selects it.

### Two bases

There are two declaration versions. They count in different ways and are never
read as one another.

| | `run-budget.policy/v1` | `run-budget.policy/v2` |
| --- | --- | --- |
| Counts | The amount the provider reports it billed | The reported token usage times a tariff you declare (`usage_tariff`) |
| Route | OpenAI Chat Completions through a generic client (`route.api` is `chat_completions`) | Direct, non-streaming OpenAI Responses (`route.api` is `responses`) |
| Tariff in the file | One set of prices, used for the reservation | Prices by input-length tier, with the endpoint, response models, and input bound |
| Billed amount in the ledger | Recorded as reported | Always `unreported`; the charge is computed from usage |

### Declare a v1 budget

Create `<operator root>/run-budgets/<budget id>.json`. The `budget_id` inside the
file must equal the file name without `.json`.

```json run-budget-v1-example
{
  "protocol": "run-budget.policy/v1",
  "budget_id": "wallet-example",
  "limits": {
    "cost_usd": "5",
    "physical_sends": 200,
    "logical_global": 100,
    "logical_per_stage": 20,
    "tokens": 2000000,
    "wall_seconds": 3600,
    "final_repair_attempts": 3,
    "final_repair_seconds": 600
  },
  "tariff": {
    "revision": "tariff-example-1",
    "input_per_million": "0.5",
    "cached_input_per_million": "0.05",
    "cache_write_per_million": "0.5",
    "output_per_million": "2",
    "request_fee": "0"
  },
  "route": {
    "api": "chat_completions",
    "model": "example-model",
    "upstream_only": ["example-upstream"],
    "allow_fallbacks": false,
    "effort": "low",
    "reasoning_in_output": true
  },
  "max_output_tokens": 4096,
  "max_input_tokens": 120000,
  "input_overhead": 256,
  "ledger_dir": "/operator/ledgers"
}
```

- Money values are decimal strings, never numbers. A price of zero for input or
  output is refused, because a free route would let the cost limit stop nothing.
- `upstream_only` names the upstream providers a request may use, each once.
  `allow_fallbacks` must be `false` and `reasoning_in_output` must be `true`.
- `wall_seconds` and `final_repair_seconds` must be above zero.
- An unknown field, a repeated key, or a value of the wrong type refuses the
  whole file.

### Declare a v2 budget

A v2 file is for direct OpenAI Responses runs. The provider reports token usage
but no billed amount, so the wallet charges your declared tariff on the
validated usage.

```json run-budget-v2-example
{
  "protocol": "run-budget.policy/v2",
  "accounting_basis": "usage_tariff",
  "budget_id": "wallet-example",
  "limits": {
    "cost_usd": "5",
    "physical_sends": 200,
    "logical_global": 100,
    "logical_per_stage": 20,
    "tokens": 2000000,
    "wall_seconds": 3600,
    "final_repair_attempts": 3,
    "final_repair_seconds": 600
  },
  "route": {
    "api": "responses",
    "endpoint": "https://api.openai.com/v1/responses",
    "model": "example-model",
    "response_models": ["example-model-snapshot"],
    "effort": "high",
    "service_tier": "default"
  },
  "tariff": {
    "revision": "tariff-example-1",
    "price_conditions": {"region": "global", "regional_surcharge": "none"},
    "cache_write_mode": "none",
    "tiers": [
      {
        "above_input_tokens": 0,
        "input_per_million": "2",
        "cached_input_per_million": "0.2",
        "output_per_million": "8",
        "request_fee": "0"
      }
    ]
  },
  "input_bound": {
    "method": "body_bytes_plus_overhead_or_provider_max",
    "input_overhead": 256,
    "provider_max_input_tokens": 400000
  },
  "max_output_tokens": 4096,
  "max_input_tokens": 400000,
  "ledger_dir": "/operator/ledgers"
}
```

- `route.endpoint` must be the endpoint the direct client posts to,
  `https://api.openai.com/v1/responses`. It is compared with where the transport
  really posts, and a different value refuses every send. Keep it as written.
- Replace the model, the response models, the prices, and the limits with your
  own values. The ones above are examples, not real prices.
- `response_models` lists the exact response model names you accept. A reply from
  another model leaves the send unknown.
- `cache_write_mode` is `ttl_30m_12h` (writes are priced per TTL, and every tier
  then needs `cache_write_30m_per_million` and `cache_write_12h_per_million`) or
  `none` (no write is priced, and no tier may carry a write price).
- `tiers` start at `above_input_tokens` 0 and rise strictly. The tier a request's
  input falls into prices the whole request.
- `price_conditions` are your statement of region and surcharge. The host cannot
  verify them.
- A request that replays an opaque reasoning item is bounded by
  `provider_max_input_tokens`; without that value such a request is not sent.
- Every role of the run must use the one model and effort of the file.

### The shared path, the ledger, and a change of basis

- Select the wallet with `COMMANDAGENT_RUN_BUDGET=<budget id>`. Unset means off,
  and nothing about the budget is read. The ID uses letters, digits, `-`, `_`,
  and `.`, is at most 64 bytes, and does not start with a dot.
- v1 and v2 files live at the same path, `<operator root>/run-budgets/<id>.json`.
  The host reads the `protocol` field to choose the decoder; any file that does
  not name v2 is read as v1.
- The ledger lives at `<ledger_dir>/<budget_id>` for both versions, under one
  lock. It keeps the basis and a hash of the exact declaration bytes.
- A wallet never changes its basis. A v1 ledger opened by a v2 file, or the
  reverse, is refused, and there is no migration: use a new budget ID. Any edit
  to the declaration of an existing wallet is refused too.
- A run the wallet cannot count is refused before it sends. Examples are a
  workflow, a model probe, request-pattern routing, and a run with the OS sandbox
  off.

### What the summary shows

`--summary-json` adds three fields next to `provider_cost_usd`. They sum the
amounts the provider reported as billed, one per provider call; they never come
from a price table.

| Field | Meaning |
| --- | --- |
| `provider_billed_cost_usd_total` | The sum as a decimal string, or `null` when no call reported an amount |
| `provider_billed_cost_complete` | `true` when every call reported an amount, `false` when some did not, `null` when the run made no provider call |
| `provider_billed_cost_calls` | How many calls reported an amount |

Each amount finer than 18 decimal places is rounded up to the 18th place, so the
total never undercounts. If the sum overflows, `provider_billed_cost_usd_total`
is `null` and `provider_billed_cost_complete` is `false`, even when every call
reported an amount, and `summary.md` has no line because there is no total.

`summary.md` gains one line after the provider usage table when at least one
amount was reported, for example
`Provider billed cost: $0.50154 (3/3 calls reported)`. A missing amount appends
` (incomplete)`. A run with no reported amount shows no line.

A **Responses run reports no billed amount**, so under a v2 wallet the three
fields are `null`, `false`, and `0`. This is not "free": it means the provider
sent no amount. The money the wallet counted (usage times your tariff) is a
different number, shown in the `run_budget` object described next. Providers
that report no amount, such as Ollama, give the same three values.

### The wallet and the stages in the summary

At its end, a run that selected a run budget records the wallet's state once, as
the `run_budget_summary` event. A run that declared structured stages records
them the same way, as `structured_stages_summary`. `--summary-json` reads these
events and adds two objects. Each object is left out when the run did not declare
that feature. The summary reads only the run's own events and never a ledger, so
a run recorded before this feature has neither object. Only the current run
counts: the events from the last `run_start` on. A run that reuses an events
path never shows an earlier run's objects.

This covers **direct-command runs only** (`--prompt`, `--plan-run`, and the other
direct commands). A REPL session shows neither the fields nor the section.

| Object | Field | Meaning |
| --- | --- | --- |
| `run_budget` | `record` | `observed` or `unavailable` (the causes are listed below). An `unavailable` record has no amount, never `0`; it keeps `accounting_basis` and `stop` |
| `run_budget` | `accounting_basis` | `provider_billed` (v1) or `usage_tariff` (v2) |
| `run_budget` | `limit_usd`, `spent_usd`, `held_usd` | The cost limit, what the wallet spent, and what it still holds, as decimal strings. v1 spends each settled send's consumed amount; v2 spends each send's tariff charge |
| `run_budget` | `declared_tariff_charge_usd_total` | v2 only: your declared tariff applied to the verified usage, summed. It is **not a provider bill**. Under v2 it equals `spent_usd` |
| `run_budget` | `sends`, `unknown`, `latch` | Send counts by state, each unknown send with its cause, and the latch with its cause, in the ledger's own words |
| `run_budget` | `stop` | `null`, or the wallet's own refusal in this command: `kind` `limit` with the declaration's `limit` name (such as `cost_usd`), or `refused`. A stop is never inferred from the amounts. The run holds it outside the ledger, so `unavailable` keeps it too |
| `structured_stages` | `record`, `complete`, `ledger_intact` | `observed` with the answer of the one stage verdict at the end of the run, or `unavailable` with nothing else. A later summary does not judge the files again |
| `structured_stages` | `stages` | Per target: `phase`, `sealed`, `attempts` (`used`, `limit`), `artifact` (`path`, `committed_sha256`), and `failed_checks` when an attempt's checks failed in this run |

A record is `unavailable` when nothing could be vouched for:

- the wallet's ledger lock, or a stage lock, was not free before the end-of-run
  deadline (1 s in all, shared by the wallet and the stages; for example a send
  still held the lock when `SIGINT` came);
- a lock was poisoned;
- the wallet's ledger file was no longer what the run wrote;
- a sum overflowed;
- the run never opened the stage record (it ended before its first stage call
  and any stage gate), or no session bound the declared stages;
- the recorded event cannot be read back: a record that says `observed` but whose
  body does not decode is reported as `unavailable`, in the JSON and in
  `summary.md` alike.

`summary.md` gains a short section for each object that is present, after the
billed line. `assurance` stays where it was: a sealed stage set is reported here,
but it does not raise `static`.

## Check an Operator Root Without a Run

`commandagent --operator-check` asks the run's own admission whether it accepts
what the operator root declares for this configuration, without a provider call
and leaving nothing behind. Pass the flags you would pass to the run
(`--profile`, `--extension-root`, `--pack`, `--provider`, `--api`, `--sandbox`,
`--allow`, `--request-pattern`, and so on) with the same environment
(`COMMANDAGENT_ADAPTER_CONFIG_ROOT`, `COMMANDAGENT_RUN_BUDGET`). The
configuration is resolved, and judged, as a non-interactive `--prompt` run's,
with an empty goal: pass `--profile` rather than relying on inference from a
goal. Add `--json` for the `commandagent.operator-check/v1` report.

```bash
commandagent --operator-check --json --profile my-profile --extension-root ./ext \
  --provider openai --api responses --model my-model
```

Nothing is made, opened for writing, or locked: no state directory, sandbox
temporary, wallet directory, lock file, ledger, or events file. Two things touch
the machine. When a proposal tool is declared, the isolation probe makes a
temporary area under the system temporary directory (`TMPDIR`), holds a loopback
socket, and starts a sandboxed child; the check removes the area and fails if it
is still there. An interrupt during the probe can leave that empty area behind,
as the run's own probe can. The OS sandbox is probed by starting the backend
once, as a run's start does.

Each check has an `id`, a `status` (`pass`, `fail`, `unverified`,
`not_declared`, or `not_checked`), a `code`, and a `detail`. The `code` is the
run's own word for the same refusal: a profile tool's kind (`invalid_pin`,
`unavailable`, ...), the stage contract's `stage_contract_refused`, or a run
budget refusal such as `PolicyV2(UnsupportedServiceTier)`, `Wallet(Corrupt)`,
`RouteNotCountable`, or `Unmetered(RequestPattern)`. The command exits `0` when
every check passes, `1` when one fails or is `unverified`, and `2` when the
configuration itself is refused, as the run would refuse it. The whole report
goes through the run's secret scrub before it is printed, in both forms.

| Check | What it runs |
| --- | --- |
| `run_budget.controls` | The sandbox mode and the key's variable against the run budget's controls |
| `pack` | The pack selection, resolved as the run resolves it |
| `profile_tools` | Every descriptor: decode, pins, dependencies, schema adoption, the schema and proposal preflight, and the isolation probe a proposal tool needs |
| `isolation.probe_cleanup` | That the isolation probe's temporary area was removed |
| `os_sandbox` | The OS sandbox the run would install, probed but not installed |
| `artifact_contract` | The artifact contract adapters the profile or pack declares |
| `structured_stages` | The stage contract, with the sandbox as probed |
| `run_budget.declaration` | The declaration, read and decoded as the run reads it |
| `run_budget.ledger` | The way to the wallet's directory, and a ledger already there, held to the declaration. It is read **without the wallet's lock**, and every form of this check says so, the one not reached included; a record that changes between two reads is `unverified`, never a pass |
| `run_budget.route.executor` | Whether the wallet can count the executor's route |
| `run_budget.route.planner` | Always `not_checked`: a `--prompt` run builds no planner client, so its route does not decide the result |
| `request_pattern` | The request-pattern setting: under a wallet, `jev` routing is refused as `Unmetered(RequestPattern)` |
| `secret_scope` | That the run's secret scope can be built from its credentials. A credential that collides with a reserved word, or a `.env` that cannot be read, is a configuration refusal (exit 2), not a failure of this check: resolving the configuration reads the same sources first. This check only reads them again after that |

The report also lists `pins`: for each selected tool, `schema_sha256`, each
dependency, the executable, each pinned directory, `runtime_sha256`, and the
descriptor's own digest, each shown as `declared` beside `calculated`. A pin
mismatch is shown even when it stops admission; the pass or fail stays
admission's. Nothing writes these values anywhere: copy them into the
descriptor yourself after you have checked what they pin.

A pass means that the current declarations, configuration, and probes are
accepted now. It does not check, and the report lists under
`not_checked_by_design`: provider connectivity and authentication, including
whether the provider key is set; the model, tier, and usage the provider
returns; the real request body, its token bound, and the reservation; late
responses and latches; creating the sandbox temporary, the wallet's directory
and lock, and saving the ledger at the real start; whether another process holds
the wallet's lock or the workspace lock; the existing state of the stage ledger;
a session to resume; the schema-tool preflight against the built client's own
wire (the check uses the configured executor's wire); and pins changed after the
check (a run compares them again before each process).
