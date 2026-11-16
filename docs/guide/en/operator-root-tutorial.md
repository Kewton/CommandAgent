# Build a Two-Stage Extension

[日本語](../ja/operator-root-tutorial.md) | [Guide index](../README.md)

This page walks through the sample in
[`examples/operator-root/`](../../../examples/operator-root/README.md): a profile
with two structured stages, each stage one process you pin in the operator root,
and a run budget that counts every provider call. You run it against a fake
model, so no key and no real provider are needed. At the end you have what you
need to build your own two stages in the same shape.

The reference for every file used here is
[Operator Root, Profile Tools, and Run Budget](profile-tools.md). This page does
not repeat its tables; it shows them working together.

## Before You Start

| You need | Why |
| --- | --- |
| macOS | A pinned process runs only under the macOS Seatbelt sandbox, after its probe passes. Elsewhere the tool is refused as `unavailable` before the run starts |
| `commandagent` on your `PATH` | Every command below runs it |
| `/bin/bash` | The only executable that works as a pinned process on macOS today. `/bin/sh`, `/usr/bin/python3`, and `/usr/bin/perl` are launchers that exec another file, which the sandbox refuses. The tools use bash builtins only |
| `python3` | Only for the rehearsal's fake model, `rehearse/fake_model.py` (standard library only). The tools never use it |

What the sample does with its toy data:

- Stage `parse` reads `input/items.txt` (lines `name,quantity`) and proposes
  `work/items.json`.
- Stage `check` reads `work/items.json`, checks that each name is unique and each
  quantity is zero or more, and proposes the one final artifact,
  `out/report.json`, as `{"items":<count>,"total":<sum>}`.

## 1. Lay Out the Root

Every pinned path must be absolute and canonical, so the sample ships templates
that say `@ROOT@` and a script that fills them in. Run it from a checkout of the
repository and give it a new directory:

```bash
bash examples/operator-root/setup.sh "$HOME/two-stage-example"
ROOT=$(cd "$HOME/two-stage-example" && pwd -P)
```

The directory's path may hold only letters, digits, `/`, `.`, `_`, and `-`; any
other character is refused.

`setup.sh` copies five directories, writes the canonical path wherever a
template says `@ROOT@`, makes the empty directories below, and removes group and
other write permission, because the host refuses a pinned tool directory that
someone else can write.

| Path under `$ROOT` | Holds | Who owns it |
| --- | --- | --- |
| `extension/profiles/two-stage-example/manifest.toml` | The profile: two tools, two stages, the required artifact | The extension author |
| `operator/profile-tools/example-parse.json`, `example-check.json` | One descriptor per tool: what runs, its pins, its stage floor | The operator |
| `operator/run-budgets/example-wallet.json` | The v1 run budget | The operator |
| `tools/lib.sh`, `parse.sh`, `check.sh` | The two processes and their shared helpers | The operator |
| `workspace/` | The workspace: `input/items.txt` and `.commandagent/config.toml` with the preset | The user of the run |
| `rehearse/fake_model.py`, `replies.json` | The fake model and its script | Only for the rehearsal |
| `ledgers/` | The run budget's ledger, made by the first run | The host |
| `home/`, `tmp/` | An empty `HOME` and `TMPDIR` for `commandagent` (step 5) | Only for the rehearsal |

The operator root (`$ROOT/operator`) and the tools are outside the workspace, as
they must be: a model, a profile, or a workspace file cannot change them.

## 2. Read the Manifest

The profile declares its tools by opaque IDs and orders them as stages. It names
no executable, path, or hash.

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

- `builtin_tools = ["Read"]` leaves the model no `Write`, `Edit`, or `Bash`: the
  only way to change the workspace is to call a stage tool.
- The edge from `parse` to `check` comes from the descriptors: `example-check`
  reads `work/items.json`, which is `example-parse`'s output.
- `[artifacts] required = ["out/report.json"]` names the final artifact.

The fields of the declaration are described under
[Structured stages](profile-tools.md#structured-stages).

## 3. Read a Descriptor

Open `$ROOT/operator/profile-tools/example-parse.json`. The parts that matter:

```json
"schema": {
  "protocol": "profile-tool.schema/v1",
  "root": {
    "type": "object",
    "additionalProperties": false,
    "required": ["expected_revision"],
    "properties": {"expected_revision": {"type": "string"}}
  },
  "inputs": ["input/items.txt"]
},
"process": {
  "executable": {"path": "/bin/bash", "sha256": "0000…"},
  "dependency_directories": [{"path": "<ROOT>/tools", "sha256": "0000…"}],
  "argv": ["<ROOT>/tools/parse.sh"],
  "cwd": "<ROOT>/tools",
  "env": {"LC_ALL": "C"},
  "outputs": [{"id": "items", "path": "work/items.json"}]
},
"stage": {"required_checks": ["lines"], "attempts_per_target": 3, "attempts_per_run": 8, "deadline_seconds": 600}
```

- `schema.inputs` lists the workspace files the host reads and hands to the
  process. The process never opens a workspace path itself.
- The schema takes one required string, `expected_revision`. A stage tool must
  have it; the host keeps it out of what the process is asked.
- `schema_sha256` ships with its real value, because the schema is the same on
  every machine. The executable pin and the directory pin depend on your
  machine, so they ship as zeros and you copy them in at step 5.
- `outputs` holds exactly one entry. Its `path` is the operator's choice; the
  process never sees it.
- `stage` is the operator's floor. Without it the stage tool is refused.

## 4. Write a Stage Process

The host starts `/bin/bash <ROOT>/tools/parse.sh`, writes one request line to its
standard input, and reads one result document from its standard output. The
fields of both are listed under
[The process envelopes](profile-tools.md#the-process-envelopes).

### The Request

One line of compact JSON. The sample reads the values it must repeat and the one
input:

```bash
source "${BASH_SOURCE[0]%/*}/lib.sh"
read_request || exit 3          # REQ_ID, REQ_TOOL, REQ_RUNTIME, REQ_REVISION, REQ_SCHEMA, REQ_OUTPUT
b64_decode "$REQ_INPUT" || exit 4   # the input's bytes, as integers in BYTES
bytes_to_text                       # the same bytes as TEXT
```

### The Result

One JSON document. It repeats the binding fields, lists the checks, and carries
the candidate's SHA-256 and canonical base64:

```bash
add_check lines "$status"         # passed or failed
emit_result "{\"items\":[$items]}"$'\n'
```

- A failed check is reported with **exit 0**. The model is told
  (`status: resubmit`, `code: check_failed`, with your diagnostics) and may call
  the tool again. A non-zero exit ends the run.
- The host adopts the bytes only when every check is `passed` and every check in
  `stage.required_checks` is present.
- What the process says is data. Exit 0 or a sentence in the result is not
  evidence; the host checks the digest against the bytes itself.

### What Bash May Do in the Sandbox

The process runs under a `deny default` policy. It may not fork, exec anything
but the pinned executable, write a file, or connect anywhere. It may read only
the executable, its dependency directories, and system libraries. So the
sample's tools use bash builtins only:

| Do | Do not |
| --- | --- |
| `[[ =~ ]]`, `${var#…}`, `printf -v`, arithmetic, arrays, `source` of a file in a dependency directory | `$(…)` or backticks (they fork), a pipe, `<<<` (it writes a temporary file), any external program such as `sed` or `sha256sum` |

That is why `tools/lib.sh` computes base64 and SHA-256 in bash arithmetic. Copy
it as it is; the repository tests it against a reference implementation.

`lib.sh` makes two assumptions to stay small:

- It reads only the first input of the request. A stage that reads two inputs
  needs its own `read_request`.
- It writes check IDs and diagnostics into the result's JSON as they are, without
  escaping. Keep them to letters, digits, spaces, and `._-`; a `"` or `\` breaks
  the result, and the run ends with `result_rejected: … result_malformed`.

## 5. Check the Root and Copy the Pins

Define the environment once, then ask the host whether a run would admit what
you declared. The check makes no provider call and writes nothing.

```bash
ca() {
  env -i PATH="$PATH" \
    HOME="$ROOT/home" \
    TMPDIR="$ROOT/tmp" \
    COMMANDAGENT_ADAPTER_CONFIG_ROOT="$ROOT/operator" \
    COMMANDAGENT_RUN_BUDGET="example-wallet" \
    COMMANDAGENT_COMPATIBLE_REASONING_EFFORT="low" \
    commandagent "$@"
}
FLAGS=(--cwd "$ROOT/workspace" --extension-root "$ROOT/extension"
  --profile two-stage-example --preset example
  --base-url http://127.0.0.1:8765/v1 --allow read,write)
ca --operator-check --json "${FLAGS[@]}"
```

- `ca` runs `commandagent` with these variables only and with `HOME` at the
  empty `$ROOT/home`. Your own `~/.commandagent/config.toml` and any other
  `COMMANDAGENT_*` variable in your shell do not apply, so the rehearsal behaves
  as it does in the repository's test, which runs the same way.
- `--allow read,write` is the run's tool ceiling. `read` lets the model use
  `Read`. `write` is the authority a `propose-artifacts` tool needs for the host
  to write the bytes it adopts; without it the run is refused before it starts
  with `profile_tool_refused: effect_read_only`. The model still has only `Read`,
  because the profile lists no other built-in tool.

The first check exits `1`: `profile_tools` fails as `invalid_pin`, because the
pins are zeros. The report lists every pin as `declared` beside `calculated`:

```json
{"tool_id": "example-parse", "pin": "executable", "path": "/bin/bash",
 "declared": "0000000000000000000000000000000000000000000000000000000000000000",
 "calculated": "<64 hex digits>"}
```

For each tool, copy the `calculated` value of `executable` into
`process.executable.sha256`, and that of `dependency_directory` into
`process.dependency_directories[0].sha256`. Check what each value pins before you
copy it; nothing writes them for you. Run the check again: it exits `0` with
`status: pass`.

- `schema_sha256` already matches.
- Any later edit of a file under `tools/` changes the directory pin. Run the
  check again and copy the new value.

## 6. Rehearse with the Fake Model

In a second terminal, start the fake model. It answers each request with the next
reply in `replies.json` and reports a synthetic billed cost:

```bash
python3 -I "$ROOT/rehearse/fake_model.py" --replies "$ROOT/rehearse/replies.json" \
  --port 8765 --record "$ROOT/requests.jsonl"
```

Then run the prompt with the same environment and flags:

```bash
ca "${FLAGS[@]}" --summary-json --prompt "Produce out/report.json."
cat "$ROOT/workspace/out/report.json"
```

The run exits `0` and `out/report.json` holds `{"items":3,"total":15}`.

- The script calls `ExampleParse`, then `ExampleCheck` with the revision the host
  named, then answers in text. `{revision}` in `replies.json` is filled from the
  host's last answer, and a `stale` answer is repeated with the revision it names.
- After a `resubmit` the fake does not repeat the call; it moves on in the
  script. To rehearse a failed check, write the same call into `replies.json`
  once more.
- `requests.jsonl` holds every connection and request the fake received.
- To rehearse again in the same directory, stop the fake (Ctrl-C) and run this
  step again from the start: the run exits `0` again. A fake that is still
  running has used up its script and answers only text, so a second run against
  it ends `1` with `stage_incomplete`.

Why the preset carries `openrouter_provider`: a v1 run budget counts only the
generic OpenAI-compatible Chat Completions route, the one route where the
provider reports the billed amount of each response, and it admits a send only
when the request names the routing table the wallet declares. `openrouter_provider`
is CommandAgent's configuration key for that table; the value
`example-upstream` is fictitious. `--operator-check` does not see the request
body, so a missing or different table still passes the check; the run then
stops at its first send with `the run budget refused the send:
Wallet(Budget(RouteNotAdmitted))`. Keep the table equal to the wallet's
`upstream_only` and `allow_fallbacks`, the preset's `model` equal to its
`route.model`, and `COMMANDAGENT_COMPATIBLE_REASONING_EFFORT` equal to its
`route.effort`.

### Read the Summary

`--summary-json` prints one JSON line last. Two objects show what the host
decided:

| Field | Expected value |
| --- | --- |
| `structured_stages.complete` | `true` |
| `structured_stages.stages[].sealed` | `true` for `parse` and for `check`, each with the `committed_sha256` of its file |
| `run_budget.record` | `observed` |
| `run_budget.stop` | `null` |
| `run_budget.sends.settled` | The number of requests the fake answered, counted over the wallet's life |

The wallet is one wallet for its whole life: its sends, money, and wall clock
count across runs from its first send. After its limits, lay out a fresh copy
with `setup.sh` or declare a new budget ID; an edited declaration of an existing
wallet is refused.

## 7. A Refused Pin

Append one byte to a pinned script and check again:

```bash
printf '\n' >> "$ROOT/tools/check.sh"
ca --operator-check --json "${FLAGS[@]}"
ca "${FLAGS[@]}" --prompt "Produce out/report.json."
```

The check exits `1` with `profile_tools` failing as `invalid_pin`. The run exits
`2` with `error: profile_tool_refused: invalid_pin: …` before it starts:
`requests.jsonl` gains no line. To recover, check what changed, then copy the new
directory pin as in step 5.

## 8. Adapt It to Your Own Two Stages

1. Copy the sample with `setup.sh` into a new directory, and rename the profile
   directory and its `[metadata] id`.
2. In the manifest, give your tools and stages their IDs, and set
   `[artifacts] required`, the `files` of `[[checks.final]]`, and the phase
   prompt under `[plan]` to your final file. Rewrite the
   `[guidance.variants.contract]` message with your tools' `name`s.
3. For each tool, write a descriptor named `<tool_id>.json`: its `tool_id` and
   `name`, the `schema.inputs` it reads (stage 2 reads stage 1's output), one
   `outputs` entry, `argv` naming your script, and a `stage` floor whose
   `required_checks` are the check IDs your script reports. The `name` is what
   the model calls; use it in the guidance and in `replies.json`. Recompute
   `schema_sha256` whenever you change `schema`: it is the SHA-256 of the
   compact JSON of `schema` with keys sorted, and `--operator-check` shows the
   `calculated` value.
4. Write each script with `lib.sh`: read the request, decode the input, decide
   your checks, and call `emit_result` with the output's text.
5. Point `replies.json` at your tool names, change `--profile` in `FLAGS` to
   your profile ID, then run steps 5 and 6.

Keep the limits of this version in mind: macOS only, one output per tool, and
for a fake rehearsal a v1 run budget only (a v2 wallet counts only direct
OpenAI Responses calls).
