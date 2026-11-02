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
  gated.
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

## What G1 does not provide

These are refused as `unavailable`. None has a stub that reports success.

- Effects other than `read`, including `propose-artifacts` (G3).
- `object` and `array` parameters (G2).
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

## For G2 and G3

`admit_selected` returns `AdmittedProfileTools`. Each `AdmittedToolBinding` holds the
run, profile, and tool identity, the schema and descriptor pins, the effect, and the
effective limits, with private fields and read-only accessors. Admission of a new tool
does not change a profile's admission or assurance: an extension profile stays a draft
with the `static` ceiling.
