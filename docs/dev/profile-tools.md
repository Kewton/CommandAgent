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
| `schema_unsupported` | A schema outside the subset below, a client that cannot carry it, or a schema the provider sanitizer would change |
| `schema_limit_exceeded` | A schema over a limit below, or an operator limit wider than the opening one |
| `schema_scrubbed` | The secret scrub would change the schema |
| `schema_mismatch` | The compiler no longer gives the adopted schema from the adopted bytes, or a later admission finds another descriptor than the adopted one |
| `input_revision_unavailable` | A declared input cannot be read when it would be adopted |
| `input_revision_mismatch` | A declared input no longer holds the adopted revision |
| `schema_violation` | An argument failed the validator. The reason is bounded data, returned to the model |

## What G1 does not provide

These are refused as `unavailable`. None has a stub that reports success.

- Effects other than `read`, including `propose-artifacts` (G3).
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
  (OpenAI chat, LM Studio, OpenAI-compatible). This was narrowed from the first decision,
  which also covered explicit text (XML): the XML parser reads on from text it has closed up,
  so it cannot show that a call's arguments are what the reply held. The user approved the
  narrowing, and XML support moved to #978. The text tool protocol, the Responses API, both
  Gemini APIs, and Ollama are refused as `schema_unsupported` before any provider call, in
  `schema/invocation.rs` and `src/providers/profile_tool_schema.rs`, with those clients and the
  XML parser unchanged. Support is
  declared by the client (`ChatClient::profile_tool_schema_wire`). An entry that holds no
  session client (a planner call, a workflow) asks the configuration what the executor role
  would build, including the `openai_compatible` override. Nothing reads a label. An injected
  client declares nothing and is refused; it is not assumed to qualify. A role that never
  carries a schema tool, such as a classifier, does not decide; a call whose client is
  replaced for that call by a classifier override is compared right before it is sent.
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
tool, a schema a shared step on the way out would change,
a schema over a limit, and an input that cannot be adopted. The provider sanitizer and the
secret scrub are not changed for this. Each adopted schema is compared with its copy after the
sanitizer and after the scrub, and again right before a request leaves; a difference is a
refusal. A secret equal to a property name, an enum value, or a pattern is `schema_scrubbed`
with the value not repeated. There is no fallback to plain text or Bash. A selected schema tool
also keeps a session from starting the native-to-XML fallback, whatever caused the parse error.

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
- The binary registers no runtime and no compiler, so through the CLI a schema tool is refused
  as `unavailable`. Success runs go through the library entries.
- A provider's own acceptance of this subset is not measured. Nothing here calls a live service.
- The `dir_fd` limit above still applies. G3 takes it up, and a schema check does not resolve it.
- The receiving context is a thread-local, which `provider_call` hands to its worker, as with the
  secret scope. A custom client that parses on another thread produces no receipt, so its calls
  are refused rather than trusted.

## For G3

`admit_selected` returns `AdmittedProfileTools`. Each `AdmittedToolBinding` holds the
run, profile, and tool identity, the schema and descriptor pins, the effect, the effective
limits, and for a schema tool the input revision and the adopted schema's digest, with private
fields and read-only accessors. Admission of a new tool does not change a profile's admission
or assurance: an extension profile stays a draft with the `static` ceiling.
