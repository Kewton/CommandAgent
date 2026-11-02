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

Every refusal below happens before the first provider call, in the minimal loop
and in the planner, with the message `profile_tool_refused: <kind>: <reason>`.

| Kind | Cause |
| --- | --- |
| `unknown_tool_id` | The operator did not register the ID |
| `name_collision` | The tool name equals a builtin, removed or not, or another admitted tool |
| `conflicting_declaration` | A tool in both profile and pack, an effect that differs from the registry, a limit wider than the registry |
| `invalid_pin` | The schema or a dependency does not match its pin |
| `workspace_descriptor` | The registry, a descriptor, or a dependency resolves into the workspace; a descriptor is a symlink |
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
- `requires = ["os_sandbox"]` while the OS sandbox is not applied to this run, and
  any other required feature. A hash pin and a cleared environment are not isolation
  and do not stand in for it.

## At execution

- A name outside the allowlist fails as `unknown tool:` before anything runs,
  including the verify step's split Bash path. The model gets the existing
  recoverable feedback.
- The existing mode, `--allow` policy, protected-path, and workspace rules still
  apply to what remains. The result is the intersection: a declaration only removes.
- The descriptor digest and dependency pins are compared again right before an
  execution. A mismatch ends the run with the tool not executed.
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
decodes, so no manifest is read again. A selected pack decoded earlier in the process
without a declaration is not read again. A pack selection the process has not decoded
yet, which only a direct library caller can reach, is read once to find out.

No event name, field, or fixed identifier was added; a later stage that adds any
reserves it in `src/sensitive_data/fixed_identifiers.rs`.

## For G2 and G3

`admit_selected` returns `AdmittedProfileTools`. Each `AdmittedToolBinding` holds the
run, profile, and tool identity, the schema and descriptor pins, the effect, and the
effective limits, with private fields and read-only accessors. Admission of a new tool
does not change a profile's admission or assurance: an extension profile stays a draft
with the `static` ceiling.
