# Profile-selected operator tools

Issue #972 (G1). A selected profile or pack can reference opaque tool IDs. Only
what the operator pinned outside the workspace reaches the generation loop.

## Who owns what

| Owner | Owns | Cannot do |
| --- | --- | --- |
| Profile manifest (`schema_version = "v2"`) or eval pack | `profile_tools`: tool IDs, the builtin tools it keeps, the effect it expects, features it requires, narrower limits | Name an executable, argv, module URL, expected hash, or permission; add back a removed builtin; widen a limit |
| Operator | `<operator root>/profile-tools/<id>.json`: ID, name, version, parameters and their pin, runtime, dependency pins, effect, limits | Be replaced by anything inside the workspace |
| Host | Admission before any provider call, the published set, the allowlist at execution, the pin comparison right before an execution | Lift acceptance or assurance because a tool ran |

The operator root is the directory in `COMMANDAGENT_ADAPTER_CONFIG_ROOT`. It
shares that root with `artifact_contract_check`, not its protocol: the descriptors
are `profile-tool.descriptor/v1`, and the adapter protocol is not widened.

## Declaration

A v1 manifest, an overlay, and an assist pack cannot carry this table; their
strict decoders keep refusing it. The eval pack carries it for the same reason it
carries `artifact_contract_check`: both reference an operator registry, and assist
content stays prompt material. Absent means off.

```toml
[profile_tools]
schema_version = "profile-tool.declaration/v1"
builtin_tools = ["Read", "Grep"]
tools = [
  { id = "config-json-read", effect = "read", max_output_bytes = 4096 },
]
```

`builtin_tools` is required and lists what stays: a submission-only profile leaves
out Write, Edit, and Bash. An eval pack uses the same fields in YAML under
`profile_tools:`. When the profile and its selected pack both declare, the builtin
set is their intersection and a tool ID in both is refused.

Stages over `propose-artifacts` tools use `profile-tool.declaration/v2`: see
[Structured stages](structured-stages.md).

## Descriptor

```json
{
  "protocol": "profile-tool.descriptor/v1",
  "tool_id": "config-json-read",
  "name": "ReadConfigObject",
  "version": "1.0.0",
  "description": "Read one configuration object.",
  "effect": "read",
  "parameters": [
    {"name": "path", "kind": "workspace_path", "required": true, "description": "Workspace-relative file."}
  ],
  "schema_sha256": "<sha256 of the compact JSON of parameters>",
  "runtime": {"kind": "in_process", "id": "<registered runtime>"},
  "dependencies": [{"path": "/absolute/canonical/file", "sha256": "<sha256>"}],
  "requires": [],
  "limits": {"max_input_bytes": 1024, "max_output_bytes": 4096}
}
```

## When a selected set is refused

Every refusal below happens before the first provider call, with the message
`profile_tool_refused: <kind>: <reason>`.

| Entry | When | How it ends |
| --- | --- | --- |
| The CLI, any action | Right after the config resolves: before the request pattern settings, the OS sandbox, the artifact preflight, `run_start`, a jev call, and a provider call | Exit 2, as other pre-run rejections |
| A workflow child node | After the sandbox reconciles with its workspace: before its artifact preflight, `run_start`, and a provider call | The node fails, as with the artifact preflight's refusal |
| A TUI command that runs under the completion guard (`/run-plan`, `/plan-run`, `/plan-steps`, `/ultra-plan`, `/ultra-plan-run`, `/run-ultra-plan`, `/resume`, `/model-probe`, `/setup-interaction-probe`; also Gate 1 and Gate 4, which dispatch these) | First in `run_command`: before its preflight, a host verify command, a plan or recovery file, and a provider call | `TASK FAILED`; the REPL continues |
| A step plan run as a library call (`run_plan_file`, `run_step_plan`) | Before any step, including a verify-first step and a no-op report step that reach no session | The call returns the refusal |
| An UltraPlan run through `auto_recovery` as a library call (`generate_and_run_with_ui`, `run_file_with_ui`, `run_plan_with_ui`) | Before the first planner call and any recovery file | The call returns the refusal |
| An UltraPlan as a library call (`generate_ultra_plan`, `generate_and_run_ultra_plan`, `run_ultra_plan`, `run_ultra_plan_file`, and each `_with_ui`) | Before a prebuilt plan, a plan file, an event, and a provider call | The call returns the refusal |
| A direct prompt as a library call (`run_direct_prompt_with`, `run_direct_prompt_with_common`) | Before the classifier, `request_pattern_resolved`, and a provider call | The call returns the refusal |
| A model probe as a library call (`run_configured`, `run_with_output_dir`) | Before its scratch workspace, its output directory, and a provider call | The call returns the refusal |
| A workflow as a library call (`run_workflow`) | Before the origin reproducer, `evidence/workflow-events.jsonl`, and a provider call | The call returns the refusal |
| A session or planner call | Before that call's provider request | The call fails |

A pack that fails to resolve is not known to declare anything, so at the CLI gate its
refusal stays at its existing place, after the request pattern settings. After the pack
gates, a session or planner call refuses a selected pack that no longer decodes or
verifies as `selected_pack_invalid`: it must not turn a declared allowlist off.

A profile chosen inside the TUI (Gate 1 confirmation, `/profile`, `--profile` on a slash
command) is admitted by the TUI command's check, with the effective profile.

### Known limits

- **The verification, repair, process, and storage parts below an agent run** are not
  gated: admission is decided at the entry a caller uses, and some of these parts take a
  profile for other reasons (`planner/profile.rs`, `verifier_env.rs`). They are each
  `ChatClient` client, the tools (`execute`, Bash, Write, Edit), `provider_call`,
  `bounded_process`, the verify, probe, and save helpers, `doctor`, `preflight`,
  `repair_target`, `hook_snapshot`, `planner::profiles::python_cli::argv_probe::run`,
  `tui::history::prepare_workspace_history_path`,
  `BoundaryShell::prepare_confirmed_continuation`, and `workflow::runner::verify_origin` and
  `verify_origin_from_events`, which guarantee neither admission nor what their callback does.
- `request_pattern::resolve` and `JevClassifier` are not run APIs that have been through
  admission. A product run uses them only after its guard; a caller that uses one alone
  is responsible for that itself.
- `BoundaryShell::dispatch` and `dispatch_directive` are control parts that hand execution
  to a closure, and do not guarantee admission themselves. What is guarded is the REPL's
  path, whose closure uses `handle_command`.
- `repl::run_repl` and `tui::repl::run`: starting the binary is guarded at `src/lib.rs:139`.
  As library entries they have no guard up to initializing the UI and preparing the
  history directory; a command that executes something is refused by `handle_command`.
- `run_workflow` admits the profile in its own config, for its origin and for its
  workspace. A node that names another profile is admitted when that child runs, so a
  reproducer derived from the origin can run first if only the node's profile is refused.
- The TUI classifier for an Ambiguous route (`src/tui/boundary_shell/ambiguity.rs`) runs
  before a profile is chosen and cannot dispatch, so its classification call is not
  gated by these G1 refusals. A G2 schema tool's reasons stop it; see the G2 known limits.
- A TUI refusal is not exit 2. The command ends with `TASK FAILED` and the REPL goes on.
  Before the refusal the command has recorded `tui_command_start`, and nothing else of
  the command: no `ultra_plan_generation_attempt`, provider call, host verify command,
  report step, or change in the workspace (`tests/profile_tool_registration_tui.rs`).
- A refused workflow child is treated as the artifact preflight's refusal is: the outer
  run records `circle_failed` with `node_failed:<node>` and exits 0. It claims no success.
- The directory identities are compared through their own opens on both sides of the
  `dir_fd` walk, not on the descriptors the walk holds. A swap made during the walk and
  put back before it ends is not detected; #974 (G3) takes this up. The bytes read are
  still compared with their pin.
- Off Unix a directory identity cannot be read, so every declared tool is refused. This
  path has not been built: a Windows check stops at the C toolchain the `ring` dependency
  needs.
- `--pack` and a preset's pack are decoded once, by `cli_pack`, and admission uses that
  value for the exact selection and the pack's own profile. Three paths have no such
  value, so each admission reads the selected pack, also when it declares nothing: a
  selection set directly in the environment by a library caller, a binary started
  without `--pack` that inherits `COMMANDAGENT_PACK_*`, and a pack confirmed by the TUI's
  `/pack`. The output, events, and requests stay byte-identical. With no pack selected,
  nothing is read.

| Kind | Cause |
| --- | --- |
| `unknown_tool_id` | The operator did not register the ID |
| `name_collision` | The tool name equals a builtin, removed or not, or another admitted tool |
| `conflicting_declaration` | A tool in both profile and pack, an effect that differs from the registry, a limit wider than the registry |
| `invalid_pin` | The schema or a dependency does not match its pin |
| `workspace_descriptor` | The registry, a descriptor, or a dependency resolves into the workspace; a descriptor is a symlink. (After admission a registry reached otherwise is `invalid_pin`.) |
| `invalid_descriptor` | The descriptor does not decode or validate |
| `registry_unavailable` | The operator root is unset or cannot be resolved |
| `unavailable` | A required capability below is not available |
| `selected_pack_invalid` | The selected pack does not verify, or targets another profile |
| `schema_unsupported` | A schema outside the subset below, a client that cannot carry it, the text tool protocol, or a schema the provider sanitizer would change |
| `schema_limit_exceeded` | A schema over a limit below, or an operator limit wider than the opening one |
| `schema_scrubbed` | The secret scrub would change the schema |
| `schema_mismatch` | The compiler no longer gives the adopted schema from the adopted bytes, or a later admission finds another descriptor than the adopted one |
| `input_revision_unavailable` | A declared input cannot be read when it would be adopted |
| `input_revision_mismatch` | A declared input no longer holds the adopted revision |
| `schema_violation` | An argument failed the validator. The reason is bounded data, returned to the model |

## What G1 does not provide

These are refused as `unavailable`. None has a stub that reports success.

- Effects other than `read`. `propose-artifacts` runs only with the `pinned_process` runtime
  below (G3); with any other runtime it stays refused here.
- `object` and `array` flat parameters. A nested argument takes a schema, below.
- Any runtime kind other than `in_process`, and an in-process runtime nobody
  registered. The `commandagent` binary registers none in G1, so through the CLI a
  declaration can narrow builtin tools but a tool ID is refused until a later stage
  adds a runtime. Tests register fakes through `register_read_executor`.
- `requires = ["os_sandbox"]`, whether or not the OS sandbox is applied to this run, and
  any other required feature. The sandbox wraps the commands a run spawns; an
  `in_process` executor spawns none and keeps the host's authority, so the sandbox does
  not isolate it. A hash pin and a cleared environment are not isolation either.

## At execution

- A name outside the allowlist fails as `unknown tool:` before anything runs,
  including the verify step's split Bash path. The model gets the existing
  recoverable feedback.
- The existing mode, `--allow` policy, protected-path, and workspace rules still
  apply to what remains. The result is the intersection: a declaration only removes.
- The descriptor digest and dependency pins are compared again right before an
  execution. A mismatch ends the run with the tool not executed.
- Each read, at admission and right before an execution, walks from the admitted operator
  root to `profile-tools/<id>.json` through `src/tools/dir_fd.rs`, refusing any symlink on
  the way. The root and the `profile-tools` directory must keep the device and inode
  recorded at admission, so another directory holding the same bytes is refused. A
  dependency is read the same way from `/`, and its parent directory keeps its identity.
  The identities are read on both sides of the walk, from their own descriptors, because
  `dir_fd` does not expose the ones it holds.
- A tool's own failure, or an output past the limit, is `ok: false` in the existing
  `extension_tool_result` event and ends the run.
- A tool result never satisfies an artifact or a stage. The required-path and
  acceptance gates decide as before.
- `ToolExecutionReceipt.usage` stays `None` when a tool reports no usage, and a
  field it does not report stays `None`. Unknown is not free.

## Off

With no declaration on the selected profile or pack the session keeps
`ToolRegistry::default()`: requests, tool schemas, artifacts, and events stay
byte-identical, compared in `tests/profile_tool_registration.rs` with a recording
from the base commit. The registry root is not read. A profile's declaration is
remembered in memory under its manifest's exact-byte hash when the manifest
decodes, so no manifest is read again. The CLI resolves `--pack` earlier than it did and
reports a failure at the old place. A pack's declaration comes from the selected,
verified bytes, never from another decode of the same id and version; see the known
limit above for the paths where admission reads them itself.

No event name or field was added. The `transport` value `profile_tool` of the existing
`extension_tool_result` event, the refusal prefix `profile_tool_refused`, and each refusal
kind are reserved in `src/sensitive_data/fixed_identifiers.rs`, so a credential equal to
one or to a fragment of one is refused at registration. A later stage that adds an
identifier reserves it there too.

## Nested arguments: the G2 schema

Issue #973 (G2). A descriptor can declare a `schema` instead of flat `parameters`, which
must then be empty, and `schema_sha256` pins the compact JSON of `schema` with its keys
sorted. A profile or pack still names only a tool ID: it cannot supply a schema, a
compiler, an input list, or an expected hash. Nothing below applies to a tool without a
`schema`.

```json
"parameters": [],
"schema": {
  "protocol": "profile-tool.schema/v1",
  "root": {"type": "object", "additionalProperties": false, "required": ["input_revision", "id"], "properties": {"input_revision": {"type": "string"}}},
  "inputs": ["config/allowed-ids.txt"],
  "compiler": "<registered compiler>",
  "revision_argument": "input_revision",
  "limits": {"max_argument_bytes": 4096}
}
```

### The subset

The vocabulary is closed. A keyword outside it, a keyword on a type it does not apply to,
any `$ref` or `$schema`, and any combination not listed here are `schema_unsupported`,
never ignored.

| `type` | Keywords allowed next to it |
| --- | --- |
| `object` | `properties` and `additionalProperties: false` (both required), `required` |
| `array` | `items` (required), `minItems`, `maxItems` |
| `string` | `enum`, or any of `minLength`, `maxLength`, `pattern` |
| `integer`, `number` | `enum`, or any of `minimum`, `maximum` |
| `boolean` | `enum` |
| `null` | None |

- `anyOf` is a node of its own, a non-empty array, with no other keyword beside it.
- `type` is one name, so a nullable value is an `anyOf`. The root is a `type: object`.
- `required` names only declared keys, once each. A minimum is at most its maximum.
- `enum` holds distinct scalars of the node's type, and excludes a length, bound, or pattern.
- An `integer` takes only an integer token: `1.0` is a `number`. A string length counts
  Unicode scalar values.
- A `number` is compared as an `f64`. Its `minimum`, `maximum`, and `enum` values must stay within
  2^53, and an integer argument past 2^53 on a `number` is `lossy_number`, so nothing rounds into a
  bound. An `integer` compares exactly, across the whole `i64` and `u64` range.
- A fractional token is rounded by the standard library and must equal the shortest decimal of that
  `f64`, which is the value kept. `1.0000000000000001` would become `1.0`, so it is `lossy_number`.
- A `pattern` is unanchored and runs on the `regex` crate, with no backtracking. It may not
  use `\d`, `\w`, `\s`, `\b`, a flag, a lookaround, or a backreference, because those read
  differently on a provider's engine. `(?:…)`, classes, and `\n`, `\t`, `\r`, and an escaped
  punctuation mark are allowed.

### Limits

These are this protocol's own. The artifact-contract v1 limits are unchanged. An operator's
`limits` can only narrow: a value above an opening limit or of 0 is `schema_limit_exceeded`.

| Limit | Opening value | How it counts |
| --- | --- | --- |
| Argument bytes | 64 KiB | The raw text received; the compact JSON for a value without raw text. The tool's input limit also narrows this |
| Depth | 32 | Containers nested in a document: `{}` is 1 and a scalar adds none. In a schema, the document depth it describes: `anyOf` adds none |
| Elements in arguments | 512 | Every JSON value, including the root and each container. A key is part of its member |
| Elements in a schema | 512 | Every schema node, each `enum` value, each `required` name |
| Schema bytes | 256 KiB | The compact JSON of the effective schema |

| Computation limit | Value |
| --- | --- |
| Branches of one `anyOf` | 16 |
| Product of the `anyOf` branches along any path | 4096. With 512 elements, validation visits at most 512 × 4096 schema nodes. It is checked at admission, so a schema over it never starts |
| A schema's own JSON nesting | 96 levels |
| `pattern` source | 512 bytes |
| Compiled `pattern` | 1 MiB, the `regex` crate's size limit |
| An `enum` string | 1024 bytes |
| Diagnostics of one refusal | 8, each with a path of at most 128 bytes and a fixed code: at most about 2 KiB |

### Decisions the issue body left open

The body of #973 does not define where a dynamic schema comes from, what a revision is, or
which providers qualify. These were decided with the project manager's approval.

- **Providers.** Reception and sending cover native tool calls on the Chat Completions wire
  (OpenAI chat, LM Studio, OpenAI-compatible) and, since #1005, on the Responses API wire
  (direct OpenAI, LM Studio, OpenAI-compatible; see "The Responses API" below). This was
  narrowed from the first decision,
  which also covered explicit text (XML): the XML parser reads on from text it has closed up,
  so it cannot show that a call's arguments are what the reply held. The user approved the
  narrowing, and XML support moved to #978. The text tool protocol, both
  Gemini APIs, and Ollama are refused as `schema_unsupported` before any provider call, in
  `schema/invocation.rs` and `src/providers/profile_tool_schema.rs`, with those clients and the
  XML parser unchanged. Support is declared by the client
  (`ChatClient::profile_tool_schema_wire`). An entry that holds no session client (a planner
  call, a workflow) asks the configuration what the executor role would build, including the
  `openai_compatible` override. Nothing reads a label. An injected client declares nothing and
  is refused; it is not assumed to qualify. A role that never carries a schema tool, such as a
  classifier, does not decide; a call whose client is replaced for that call by a classifier
  override is compared right before it is sent.
- **Where a dynamic schema comes from.** The descriptor declares a static `root` and `inputs`,
  workspace-relative paths read without following a symlink: at most 64, 1 MiB each, 8 MiB in
  all. A compiler the operator registered in the process (`register_schema_compiler`, by ID,
  as `register_read_executor`) returns the schema from the adopted bytes. It must be
  deterministic and must not call a model. The host admits the result like any other schema
  and hashes it.
- **The revision.** The input revision is a SHA-256 over the declared inputs in order: path, a
  NUL, the length in decimal, a NUL, and the bytes. The first admission in a process adopts
  it, which is before the first provider call, because every entry admits first. Each receipt
  and execution compares with it. A difference is refused and never adopted: not in a repair,
  not in a later session, and not on `/resume` in the same process. Restoring the bytes
  restores the run.
- **The expectation is the host's.** `revision_argument` names a required top-level property
  that is exactly `{"type": "string"}`, and the host narrows it to the adopted revision, so a
  call echoing another revision fails the same validator. A revision or hash a model sends, or
  a history holds, is compared and never taken as the expectation. A property named `accepted`
  or `schema_sha256` is data if the schema declares it and does nothing.

### Before the start

Each entry in the table above also refuses, before its first provider, classifier, or Jev
call: the text tool protocol (use the native protocol instead), a client that cannot carry the
tool, a schema a shared step on the way out would change, a schema over a limit, and an input
that cannot be adopted. The provider sanitizer and the secret scrub are not changed for this.
Each adopted schema is compared with its copy after the sanitizer and after the scrub, and
again right before a request leaves; a difference is a refusal. A secret equal to a property
name, an enum value, or a pattern is `schema_scrubbed` with the value not repeated. There is no
fallback to plain text or Bash. A selected schema tool also keeps a session from starting the
native-to-XML fallback, whatever caused the parse error.

### On receipt

The raw arguments are decoded by `schema/raw_json.rs`, only for a schema tool. A repeated key,
malformed JSON, trailing data, or a number an `f64` cannot hold is refused. This happens where
the arguments are still text, and a key repeated around them in the same body is refused too. A
refusal does not fail the parse, because a parse error is what starts a fallback.

A value is never changed to pass: no string conversion, no `null` filled in, no number
coerced, no ID corrected, no key removed. A schema tool's arguments skip the shared recovery,
which reshapes `name`, `tool`, and wrapper keys. Tag-like text in a string of native arguments is
part of the string: it is delivered as it is and creates no call.

A session with a schema tool never reads a reply as XML. If such a session reaches the XML
parser anyway, the whole reply is refused as `schema_unsupported` before anything is parsed, so
no call exists and nothing runs.

An argument the validator refuses returns to the model as data, in the existing recoverable
feedback: `{"stage": …, "violations": [{"path": …, "code": …}]}`. A path can hold a key the
model sent, so it is bounded and is not read as an instruction. A pin, revision, or schema
that no longer holds ends the run, because retrying cannot correct it.

### Right before an execution

`PinnedReadExecutor` runs the same validator and compares the binding, on top of the descriptor,
dependency, directory, and input-size checks of G1, which stay. This holds for a provider that
ignored the schema and for a direct call of the public `ToolRegistry::execute`. A direct call
gives a value, so it cannot show that raw text had no repeated key. In a session, a value with
no matching receipt is refused.

A valid schema does not make a tool available: an effect, feature, or runtime that is not
available stays `unavailable`, including `requires = ["os_sandbox"]`. Passing the validator
does not satisfy an artifact or a stage, and an unreported usage stays `None`.

### Known limits

- The text (XML) tool protocol does not carry a schema tool. A run that selects both is refused
  as `schema_unsupported` before its first provider call; use the native protocol. XML support
  is a separate issue, #978. A session without a schema tool keeps the XML parser and repair as
  they were.
- The adoption lasts for the process, keyed by workspace, profile, and tool, and nothing persists
  it, because a record in the workspace would not be trusted and `.anvil/` is not widened. This has
  two approved consequences:
  - **A long-lived REPL whose inputs were edited** refuses every later command and free text with
    `input_revision_mismatch` (`schema_mismatch` for a replaced descriptor or compiler). To recover,
    put the input bytes back, or restart the process, which adopts the current bytes.
  - **A `/resume` in a new process** adopts the current bytes and compares with nothing from the old
    process. No refusal appears if the inputs changed in between. Before resuming, check yourself
    that the inputs are the ones the earlier run used.
- A REPL's free text is checked before its route is proposed, against the REPL's current
  profile, executor client, and inputs, and refused with the `TASK FAILED` block; the REPL goes on.
  It records no event at this point. When no profile was named, the classifier may still propose a
  different profile, which is admitted when its command is dispatched. A schema tool of a pack chosen
  by `--pack` in the free text is also judged at dispatch, not before the classifier. Another refusal
  of a selected set, such as `unknown_tool_id`, keeps its place at the dispatched command.
- The binary registers no runtime and no compiler, so through the CLI a schema tool with an
  `in_process` runtime or a compiler is refused as `unavailable`. Success runs go through the
  library entries. A `pinned_process` tool with a static schema needs neither, and runs through
  the binary (G3, below).
- A provider's own acceptance of this subset is not measured. Nothing here calls a live service.
- The `dir_fd` limit above still applies to G1 and G2. G3 closes it for its own reads (below),
  and a schema check does not resolve it.
- The receiving context is a thread-local, which `provider_call` hands to its worker, as with the
  secret scope. A custom client that parses on another thread produces no receipt, so its calls
  are refused rather than trusted.

### The Responses API

Issue #1005. The Responses wire carries a schema tool with the same host contract as Chat
Completions: the same admission, receipts, validator, refusal feedback, G4 counts, and history
rules. Its own code is `src/providers/openai_responses/profile_schema.rs`, and everything in it
applies only in a session with a schema tool; a session without one keeps the Responses request,
parser, and replay as they were, byte for byte against `responses-off-golden.json`.

- **Sending.** A function definition is the flat `type`, `name`, `description`, `parameters`.
  An admitted schema tool says `strict: false`, so the service does not tighten the schema (no
  optional key made required, no `null` added); its `parameters` are the adopted schema as a
  value. No other definition and no request without a schema tool gains a field. Direct OpenAI
  keeps `store: false`, its encrypted-reasoning `include`, its effort, and `max_output_tokens`;
  a compatible server gets none of the OpenAI-only fields. A server that rejects `strict` fails
  as an HTTP error does today; nothing is resent without it.
- **Receiving.** Before any call of a reply is received, every `function_call` must have a
  `call_id` that is there, is a non-empty string, and is new to the reply and to the
  conversation (the client's stored turns and the history being sent, as one set). Otherwise the
  whole reply fails the parse with a fixed text naming `function_call` (`OpenAI function_call
  call_id missing`, `is not a string`, `empty`, `duplicate`), with no ID, argument, or secret in
  it: nothing of it runs, leaves a receipt, or is stored, and the loop retries it as a malformed
  native call up to its existing limit, then ends the run with that error, never on XML. A
  schema tool's raw `arguments` go to G2 as text, with the whole body checked for a repeated key;
  any other call keeps the parser it had, with the checked `call_id`. `item.id` is never used
  for the call.
- **The history.** `function_call_output.output` is the tool message's text, which the history
  rule already fitted (a stage answer up to 5,120 bytes); the wire does not fit it again. A
  history whose calls and outputs do not correlate one to one by call ID is refused before the
  request leaves, with `OpenAI function_call call_id correlation invalid`, and the same retry and
  end: a call with an empty or repeated ID, a tool output with no call ID (which the parent's
  request would send as user text), one that names no call before it or answers a call twice,
  and a call no output answers.
- **Replay.** A stored turn is found by its calls' IDs in order, never by its text. It goes back
  raw, every item once in its order (reasoning, a preamble message, an unknown item, the calls),
  only when (a) G2 refused none of its calls at parse time, (b) each raw call has the host's call
  ID, name, and decoded arguments, in the same number and order, and (c) the run's secret scrub
  leaves every raw item unchanged, including a message's text, a reasoning summary, any field of
  an unknown item, and the opaque encrypted content, which is compared and never read or shown.
  The scrub reads a string as written, so a string of an item that starts as JSON (a call's
  `arguments`, of any tool, or a field of an unknown item) is read as JSON too, with repeated
  keys seen, up to 1 MiB, depth 64, and four levels of JSON inside JSON: a repeated key, a
  string past those limits, or content the scrub would change (an escaped secret, say) fails
  (c). Otherwise the whole turn is rebuilt from the host's values, as the fallback builds it: the
  host's text and its calls with `arguments.to_string()` (`"{}"` for a refused call). That turn's
  reasoning, preamble, and unknown items are dropped. Unlike Chat Completions, a safe raw turn
  sends its preamble message again. An assistant turn with no call is never found by its text,
  so in a session with a schema tool it is always rebuilt from the host's text and its
  reasoning is dropped, where a session without one replays it by its text.

Known limits of the Responses wire:

- A turn that fails a condition loses its reasoning on purpose. Whether a service accepts a call
  turn without its reasoning, or a turn with or without its preamble, is not shown by a fake.
- A change of tools or prefix (a G4 schema update, a compaction) may make the service drop
  reasoning; the host's decisions and counts do not depend on it.
- Whether LM Studio or an OpenAI-compatible server accepts `strict` is not measured.
- A server that sends the same `call_id` in every reply fails from its second call turn on, as a
  duplicate, and the run ends with the error; Chat Completions would go on. No such server is
  known.
- A history that does not correlate cannot be fixed by a retry; the run ends after the retries
  without calling the provider.
- The loop's compaction (`src/minimal_loop/compact.rs`) removes messages one by one, oldest
  first, and keeps recent Read outputs, but does not remove a call together with its output. When
  it removes a call and keeps the output, the history no longer correlates, and a session with a
  schema tool on the Responses API ends with the correlation error, where Chat Completions sends
  the same history as it is. A compaction that removes only text, or a call with its output,
  leaves the session going. Removing calls and outputs as pairs is the fix, in the compaction,
  and is left to another Issue.
- When one reply has several calls and a write-required rejection stops one before the last,
  the loop leaves the batch there and the calls after it get no output; a session with a schema
  tool on the Responses API then ends with the correlation error before its next request, where
  Chat Completions sends the same history as it is. The fix is in the loop, and is left to
  another Issue.
- The reasoning is kept in the client's memory only, as before; nothing new is persisted, and a
  resume in another process sends none.

## Bindings

`admit_selected` returns `AdmittedProfileTools`. Each `AdmittedToolBinding` holds the
run, profile, and tool identity, the schema and descriptor pins, the effect, the effective
limits, for a schema tool the input revision and the adopted schema's digest, and for a G3 tool
the digest of its process declaration, with private fields and read-only accessors. Admission of
a new tool does not change a profile's admission or assurance: an extension profile stays a
draft with the `static` ceiling. `commit_receipts()` returns what the commit leaf decided.

## The propose-artifacts effect: G3

Issue #974. A process the operator pinned proposes bytes for one declared output. Only the host,
after checking them, adopts them. The process is not given a workspace to write in: its input
arrives on stdin, its candidate leaves on stdout, and the operating system refuses everything
else. A tool with this effect always has a G2 schema, so it runs on native tool calls only (Chat
Completions, or the Responses API since #1005), and is refused on the text tool protocol before
the start.

### Who owns what

A profile or pack still says only `{ id = "...", effect = "propose-artifacts" }` and may narrow a
limit. The executable, argv, environment, directories, and destination are the operator's, in the
descriptor outside the workspace. A model chooses none of them, and the process never sees the
destination.

```json
"effect": "propose-artifacts",
"schema": {"protocol": "profile-tool.schema/v1", "root": {"...": "..."}, "inputs": ["config/source.txt"]},
"runtime": {"kind": "pinned_process", "id": "<operator's label>"},
"process": {
  "protocol": "profile-tool.process/v1",
  "executable": {"path": "/absolute/canonical/tool", "sha256": "<sha256 of the file>"},
  "dependency_directories": [{"path": "/absolute/canonical/dir", "sha256": "<directory pin>"}],
  "link_directories": [{"path": "/absolute/canonical/links", "sha256": "<directory pin>"}],
  "argv": ["/absolute/canonical/dir/serializer.js"],
  "cwd": "/absolute/canonical/dir",
  "env": {"TOOL_MODE": "strict"},
  "limits": {"timeout_ms": 30000, "max_result_bytes": 65536},
  "outputs": [{"id": "artifact", "path": "config/settings.json"}]
}
```

The binary resolves this with no registered runtime, so the CLI runs these tools.

- A **directory pin** (`directory_pin`) is the SHA-256 over one line per entry below the
  directory, sorted: `d\0path`, `f\0path\0sha256`, or `l\0path\0target`. A symlink is recorded and
  never followed; anything else is refused. At most 200,000 entries and 4 GiB.
- `dependency_directories` are readable. There can be several, because a loader's libraries need
  not live beside the executable. `link_directories` are metadata-only: a directory of links a
  loader walks through. `cwd` lies inside a dependency directory.
- Every pinned path is canonical, outside the workspace, and at most 32 directories.
- `outputs` holds exactly one entry. More than one is refused before the start
  (`effect_output_refused`): separate renames are not one atomic adoption, and this first version
  has no staged generation. `id` is opaque to the host.

### The fixed environment

The final `Command` clears its environment, and then holds the declared keys and nothing else. The
host adds **no key of its own**. Bash, a loader, or the OS may set some of their own after `exec`
(`PWD`, `__CF_USER_TEXT_ENCODING`); no value comes from the parent.

- Keys and values are literal: nothing is expanded, referenced, or inherited.
- At most 16 keys, each key at most 128 bytes and each value at most 1024. Over that is
  `effect_limit_exceeded`. A key is a plain identifier, and a NUL in a value is refused.
- The declaration is part of the descriptor's bytes, so a changed value is a pin mismatch
  (`invalid_pin`), at admission and before each stage.
- Refused without regard to case (`effect_env_refused`), because a loader, a shell, or a runtime
  reads them as code or as a search path:
  `PATH`, `HOME`, `IFS`, `ENV`, `BASH_ENV`, `BASHOPTS`, `SHELLOPTS`, `CLASSPATH`, `GCONV_PATH`,
  `JAVA_TOOL_OPTIONS`, `JDK_JAVA_OPTIONS`, `_JAVA_OPTIONS`, `NODE_OPTIONS`, `NODE_PATH`,
  `PERL5LIB`, `PERL5OPT`, `PERLLIB`, `PYTHONHOME`, `PYTHONPATH`, `PYTHONSTARTUP`, `RUBYLIB`,
  `RUBYOPT`; any key beginning with `BASH_FUNC_`, `COMMANDAGENT_`, `DYLD_`, or `LD_`; any key
  ending in `_PROXY`.
- A value or an argument the existing secret scrub would change is refused with the value not
  repeated (`effect_env_refused`). There is no second, looser rule.
- The Rust core names no key or value specific to one executable. Something like
  `OPENSSL_CONF=/dev/null` for a tool that needs it is the operator's to declare.

### Isolation

| OS | Result |
| --- | --- |
| macOS, where the probe below passes | A `deny default` Seatbelt policy, through `/usr/bin/sandbox-exec` |
| macOS, where it fails (for example under another sandbox) | `unavailable` before the start |
| Linux, Windows, anything else | `unavailable` before the start |

There is no fallback to `auto`, to no sandbox, or to `os_sandbox::wrap_command`, which returns an
unconfined command when a backend is missing. `src/tools/os_sandbox/` is not used and is unchanged.

The policy is `src/tools/profile_tools/isolation.rs`. Every location is a `-D` parameter:

- `deny default`; `deny network*`, including loopback and unix sockets.
- `process-exec` of the pinned executable alone. There is no `process-fork` and no `mach-lookup`.
- Read: the executable, each dependency directory, `/usr/lib`, `/System`, `/private/var/db/dyld`,
  `/Library/Apple/System`, `/dev/null`, `/dev/urandom`, `/dev/random`, and the root directory
  itself. `sysctl-read`.
- Metadata: each pinned path and each of its ancestors, and each link directory. Nothing else can
  be `stat`ed, so the existence and size of other files stay hidden.
- Write: `/dev/null` only.

A policy judges an open, not a read or a write on a descriptor that is already open, so one left open
by whatever started the host would reach the child. Between fork and exec, the launch marks every
descriptor from 3 up close-on-exec: the host's own, the probe's, and any inherited one. The exec
happens only when every open descriptor is known to be marked. A descriptor table larger than the
fixed list, a list that fails or fills, or a mark that fails with anything but `EBADF` stops the launch
as `unavailable`, with nothing started. A resource limit is never taken for the highest descriptor,
because lowering one leaves a higher descriptor open. The runner takes only a `StrictCommand`, which
this leaf alone can make, so there is no other way to start one.

**The runtime's writers.** At admission and again right before each process, the executable, the
directory that holds it, and each dependency directory with everything in it must be owned by this user
or root, with no group or other write. Otherwise the tool is `unavailable`. A symlink's own mode means nothing and it is not followed.

**The probe.** Before the first provider call, every entry starts `/bin/bash` under this very
policy: one round trip on stdin and stdout, then a write and two connections (TCP and UDP to a
local socket the host holds open). It passes only if the round trip completes, the operating
system refuses each with `Operation not permitted`, and the file and the connection did not
happen. A timeout or an unreachable address is not evidence, and the policy is never loosened to
pass. Nothing in production can ask for a different result.

### The envelopes

`profile-tool.request/v1`, one line of compact JSON on stdin: `request_id` (new per call), the tool
ID, version, descriptor digest, `runtime_sha256` (the digest of the process declaration),
`input_revision`, `schema_sha256`, the validated `arguments`, each adopted input as `path`,
`sha256`, and `bytes_base64`, and `outputs` (the opaque ID). Inputs are the bytes G2 adopted, never
read again from a path. A declared input is judged by the credential, workspace-policy, and
protected-path rules before it is read. G2's read for a `read` tool stays as it was.

`profile-tool.result/v1`, one JSON document on stdout, read by the strict decoder of G2:

```json
{"protocol": "profile-tool.result/v1", "request_id": "...", "tool_id": "...",
 "runtime_sha256": "...", "input_revision": "...", "schema_sha256": "...",
 "checks": [{"id": "shape", "status": "passed"}],
 "outputs": [{"id": "artifact", "sha256": "...", "bytes_base64": "..."}],
 "diagnostics": [{"code": "note", "message": "bounded data"}]}
```

Accepted only if: exit 0 on its own, the whole request taken, every stream read to its end, nothing
left behind; no field unknown, none missing, no key repeated; each binding field equal to the
request; at least one check, with distinct IDs, each exactly `passed` (`failed`, `not_run`,
`unknown`, and anything else refuse); exactly the declared output, once; canonical base64; and the
digest beside the bytes equal to the bytes. Exit 0, a success sentence, or a self-declared
`accepted` is not evidence: an `accepted` key is an unknown field.

A check that did not pass ends the run. It is not retried or returned as feedback in this version;
a second submission is G4's.

**A secret in the output.** Before anything keeps, cuts, or reports a process's output, its raw stdout
and stderr, the strings of stdout if it decodes as a document, and what each of those strings decodes to
as base64, which is how the candidate travels, are compared as bytes with the values this run holds, and
this comes before how the process ended or what its result says is looked at. A string that does not
decode has nothing more to compare, and the result's own refusal stands. The values are: the sources of the run's catalog, this workspace's scope, and what
the start received. The forms are the value as it is, JSON-escaped, base64 in the standard and URL-safe
alphabets at each of three alignments, with or without padding and with other bytes around it, and hex in
either case. A match ends the call as `result_rejected` with the fixed code `secret_value_in_output`,
however the process ended. Nothing of the output is kept: no diagnostics and no `stderr_tail`, nothing
saved, and nothing returned to the model. A source of those values that cannot be read refuses the call
before the process, as `effect_env_refused`. A value shorter than the catalog's minimum is not compared.

What a process said is data, kept apart from whether it is accepted. On a refusal, the
`profile_tool_effect_rejected` event holds the kind, the diagnostics of a document that decodes
within the limits above (whatever else is wrong with it, including a binding that does not match),
and the end of stderr as `stderr_tail`, at most 2 KiB of text, each through the existing secret scrub. On a success,
the diagnostics are on the `profile_tool_commit` event and in `commit_receipts()`. Neither is in an
error message, which later steps classify by its words, and neither is in the tool result the model
gets. Nothing runs them.

### Limits

| Limit | Value | How it counts |
| --- | --- | --- |
| Arguments | 64 KiB, depth 32, 512 elements | As in G2, on the raw text received |
| Schema | 256 KiB | As in G2 |
| The whole result | 64 KiB | Stdout plus stderr, as read, one shared budget stopped at its edge. Reading past it stops the process. It covers the diagnostics, which are inside stdout |
| Expanded candidate | Within the same budget | The bytes after decoding are shorter than their base64 inside stdout, so they cannot exceed it |
| One process | 30 s | Wall clock from spawn, through the feed of stdin. Stopping and reaping take at most a further 2.5 s, plus the process group's existing grace |
| Diagnostics | 8, code 64 bytes, message 512 bytes | Inside the result |
| Checks | 64 | Inside the result |
| Env | 16 keys, 128 and 1024 bytes | Above |
| Argv | 64 entries, 4096 bytes each | |

`limits` in `process` can only narrow: a value of 0 or above an opening value is
`effect_limit_exceeded`. `max_output_bytes` of the descriptor or the declaration narrows the result
too. The request is not limited by these: it holds the adopted inputs, already bounded by G2 at
1 MiB each and 8 MiB in all, and a process that does not take it all is refused. The artifact-contract
v1 limits are unchanged.

### Permissions

The authority is the Write target of the existing allow policy (`AllowTarget::Write`), not the Read
branch of `ExtensionTool`. A proposal tool is not an extension: it has its own entry in the registry,
behind the same allowlist. It does not record `access=read_only`. The same judgement runs three
times, with the values in force each time: before the start, right before the process, and right
before the rename.

Refused before the start as `effect_read_only`: an `ExecutionMode::Plan` context, a step of kind
`inspect`, a recovery inspection with scoped reads, an allow policy without `write`, a legacy run
without `--yes` (it could not write anyway), and an action that only plans (`Action::PlanSteps`,
`Action::UltraPlan`). Refused as `effect_path_refused`: a destination or input that is outside the
workspace, a credential, a hidden or engine path, or a protected path of the completion contract.
A declared input must also be one regular file with one name: a hard link could carry a credential's
bytes under an allowed name. G2's read for a `read` tool is unchanged. `ToolContext.mode` is `Act` in
every session, so the session leaf reads the step's kind instead.

A refusal from this admission keeps that origin, also for a word it shares with G1 (`unavailable`,
`invalid_pin`, and so on), so it stops a REPL's free text before the classifier. A G1 tool's refusal
keeps its place at the dispatched command.

A name equal to a builtin, removed or not, is `name_collision`: it is checked against every builtin
before any is removed, as in G1. Write, Edit, and Bash stay removed unless declared.

| Kind | Cause |
| --- | --- |
| `unavailable` | The strict isolation is missing, on this OS or by the probe |
| `effect_read_only` | One of the read-only contexts above |
| `effect_limit_exceeded` | A limit above, widened or passed |
| `effect_output_refused` | Not exactly one output, or an output ID that is not a slug |
| `effect_path_refused` | A destination or input the rules refuse, at any stage, including what the commit finds there |
| `effect_env_refused` | A refused key or value |
| `invalid_pin` | The executable, a directory, the working directory, or the descriptor differs from its pin |
| `process_failed` | The process did not end cleanly, including a cancel or a timeout |
| `result_rejected` | The result failed a check above. The reason holds a fixed code |
| `commit_failed` | The temporary could not be written, synced, or renamed. Nothing changed |

### The commit

`src/tools/profile_tools/commit.rs`, on a parent directory descriptor reached by a no-follow walk
from a root descriptor held since admission:

1. Open the leaf without following a link and `fstat` it. If anything is there, it must be a regular
   file with one link, and not the same device and inode as an input or a protected path. A protected
   path that exists but cannot be identified, such as a symlink, is refused rather than left out.
2. `openat(O_CREAT|O_EXCL)` a temporary, mode `0600`, in the same directory, and keep its descriptor.
   Write. Move the old file's permission bits (`& 0o777`, never setuid, setgid, or sticky) if one was
   there; a new file stays `0600`. `fsync`.
3. The same `fstat`, the full judgement again, the cancel.
4. `fstatat` the temporary's name: it must be the held descriptor's device and inode, a regular file
   with one link. Read the bytes back from the descriptor: their digest must be the candidate's.
5. `renameat`. **Its success is the linearization point.**
6. `fstatat` the destination's name and read back again, the same way.
7. `fsync` the directory.

Everything is checked before step 5: the runtime pin, request ID, input revision and digest, schema
hash, the result's completeness, the candidate's digest, the limits, the cancel, the authority, the
path, and that the entry about to be adopted is the file that holds the checked bytes. A failure or a
cancel before it leaves the old bytes in place and removes the temporary.

After it the commit stands. A later cancel does not undo it: the receipt says
`cancelled_after_commit: true`. A failed directory `fsync` is `durable: false` on a receipt that
still says `committed: true`, not a failed commit. If step 6 finds something else at the destination,
there is no receipt: the call is `commit_failed` with a reason that says the rename stood, because the
old bytes are gone and what is there was never checked.

The receipt is `profile-tool.commit-receipt/v1`: the IDs, the path, `committed`, `durable`, the runtime,
input, and schema digests, `result_sha256` (the raw stdout), `committed_sha256`, the byte count, and the
diagnostics. Without the diagnostics it is the tool's result. Whole, it is on the `profile_tool_commit`
event written immediately, and it is available from `commit_receipts()`. A session records a changed path from this receipt
(`ToolRegistry::take_committed_paths`), never from a tool name or arguments. A receipt satisfies no
artifact or stage. Dependence between stages is G4's.

New events: `profile_tool_effect_call`, `profile_tool_effect_rejected`, and `profile_tool_commit`. The
existing `tool_call_raw` and `tool_execute` events still surround a call. Their words are reserved in
`src/sensitive_data/fixed_identifiers.rs`; an output ID, a path, and a diagnostic are not.

### Decisions the issue body left open

Decided with the project manager's approval. The issue body is not edited.

- **Which OS.** macOS only, with its own strict launch. Everything else is unavailable before the start.
- **Where the runtime comes from.** The operator's descriptor, with no registered runtime, so the binary
  runs it too.
- **Files.** The process writes nothing: bytes in on stdin, bytes out on stdout.
- **Forks.** Denied by the policy, so a descendant cannot exist. `process.rs` still does not trust that:
  a run stops its group, tracks descendants by their parent chain with their start time, stops the
  group of one that left through the existing guarded helper, and refuses a run that left any alive.
  It signals no group this run did not make: a descendant that joined another group stays counted
  as alive, and the run is refused.
- **Fixed environment.** Declared by the operator, literal, with the host adding no key. Measured on a
  real interpreter under this policy: it needs one for its own configuration and the core does not
  know which.
- **A failed check ends the run.** "Do not hide it by retrying" is read strictly here. Returning it to
  the model as feedback for a correction is a later decision.
- **A step plan with an inspect step under such a profile** is refused at that step's session, before
  its provider call, rather than withholding the tool.
- **An action that only plans** means `Action::PlanSteps` and `Action::UltraPlan`. A library call to
  `generate_step_plan` under another action, and the TUI's `/plan-steps`, publish no tool and start no
  process; each still meets every other refusal, and a later run of that plan meets this one.

### Known limits

- The process runs only on macOS. No Linux backend exists here. Landlock alone does not close UDP or
  writes this needs.
- **A parent directory moved during the commit.** The walk holds the parent's descriptor, so swapping
  its name for a symlink redirects nothing. But the held directory itself can be renamed out of the
  workspace by something else while the commit runs, and the file lands where that directory went.
  This is not closed.
- **Case and Unicode aliases.** On a file system that folds them, `CONFIG/a.json` and `config/a.json`
  are one file, and the string rules treat them as two. The commit's identity check catches a
  destination that is the same inode as a protected file that already exists. Nothing folds case or
  normalizes Unicode here, on purpose: a looser rule of its own would be a new way around the
  protected paths. A protected path that does not yet exist is not covered.
- **The G1 directory race.** G3's descriptor read compares the identities of the descriptors its walk
  really held, so a registry directory swapped during the walk and put back is seen. This is closed for
  the descriptor of a G3 tool. It stays open for G1 and G2 reads, which are unchanged. A dependency
  directory's tree digest walks by path below the pinned directory.
- **A runtime swapped and restored.** The pins are compared before the process and again after it,
  before the commit, so a runtime still changed then stops the commit. The process is still started by
  path: a writer running as this same user that swaps the executable or a dependency after the first
  comparison and restores it before the second is not detected. A writer of this same user is outside
  what this guards. Running a verified snapshot is a later issue. A swapped runtime still runs under the
  strict policy, so it can propose bytes only for the one declared output, and the host still checks
  the envelope.
- **Who else can write a runtime.** The check reads the mode and owner of the executable, of the
  directory that holds it, and of each dependency directory and everything in it. Any of them writable
  by group or other, or owned by neither this user nor root, is `unavailable`. That is the whole
  guarantee. Directories above those are not read, so a user who can rename in one of them can still
  swap the name of a runtime below it. Access control lists are not read, and neither are link
  directories, which the policy lets a process see only as metadata.
- **A writer of this same user and the adopted file.** Like any file, the destination can be rewritten
  by this user's other processes after the commit. The checks close the moment around the rename, up
  to step 6, and no further.
- **Other processes of this user.** Under the strict policy a tool can read the exec-time environment
  and the argv of every process of this same user through `KERN_PROCARGS2`: an ancestor shell or
  launcher, another session, a GUI, and a child that the host let inherit its environment. Measured on
  macOS 27.2 (26B123) with a parent that is not a platform binary. An earlier measurement said the
  environment could not be read; its parent was `/bin/zsh`, and that conclusion was wrong. Five Seatbelt
  rules were tried and none closes this: `sysctl-name`, `sysctl-name-prefix`, removing `sysctl-read`,
  and two forms of `process-info*`.
- **What this assumes.** The operator pinned the executable and trusts it. The sandbox is defense in
  depth against a tool that misbehaves. It does not keep this user's secrets from an executable that
  means harm. Running a tool as another user is a later issue.
- **Where a secret may be.** Do not put a secret in the exec-time environment of any process that stays
  running while a tool runs:
  - Fine: a shell that was itself started without the secret runs `set -a; source ./.env; set +a` and
    then `commandagent`. The shell's own exec-time area never held it, and the host removes it from its
    own at the start. A shell started with the secret keeps it there, and sourcing again does not help.
  - Not fine: `OPENAI_API_KEY=... wrapper` where `wrapper` execs or waits for `commandagent`. The wrapper's
    exec-time area holds the value for as long as it runs.
  - Not fine: a secret in any process's argv.
- **The start's own window.** The host holds the value in its exec-time area from its first exec until
  its second, a moment at the very start. No tool of this run has started then, and another process of
  this user that reads in that moment is not stopped.
- **What the start covers.** The command-line binary and the GUI server. A library caller, an embedding
  host, and a test harness do not exec themselves: their exec-time area is whatever started them, and a
  caller that wants this calls `startup_credentials::bootstrap_cli` first in its own `main`. The protected
  names are the four built-in provider keys and the compatible provider's selected key name. Any other
  secret-looking variable stays in the environment.
- **The output check.** It refuses a finite list of forms of a value this run holds. Encryption, splitting,
  compression, and any other encoding are not recognized.
- **GUI delegation.** The server keeps the keys it received in its store. Only for a child it delegates
  to, `restore_allowed_environment` puts the existing allowed keys in that child's environment. From the
  child's exec until its own start execs again, the key is in the child's exec-time area, and this same
  user can read it there. A later issue hands it over without an environment. A version check gets no key.
- **No second submission.** A failed check ends the run. G4 takes this up.
- **Crash.** A crash between the temporary's creation and the rename can leave a `.profile-tool-*.tmp`
  file. It is not an artifact, nothing adopts it, and nothing removes it after a restart.
- **A directory the commit made** stays if a later step fails.
- **Descendants if the policy ever allowed a fork.** One that leaves its parent within one scan tick and
  closes its streams would not be seen. The policy is what closes this today.
- **REPL.** G2's limits carry over: a `--pack` in free text, a route the classifier proposes, and the
  library `tui::repl::run` start before any admission. Each is admitted at its command and at its
  session, so nothing is saved on those paths.
- A provider's and a real tool's behavior is not measured here. Nothing here calls a live service.
