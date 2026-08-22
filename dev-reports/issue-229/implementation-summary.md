# Implementation Summary: Issue #229

Implemented the shared state-path migration contract used by the combined #255/#229/#232 row.

- Added a `runtime_paths` leaf that defines `.commandagent` as the canonical workspace runtime namespace and exposes canonical runs, plans, repairs, and evidence paths alongside explicit `.anvil` legacy read paths.
- Moved default run-event writes to `.commandagent/runs` and default platform session writes from `anvilminimal` to `commandagent`.
- Preserved legacy compatibility: run inventory searches `.commandagent/runs` first and `.anvil/runs` second, while default sessions and workspace-scoped REPL history fall back to `anvilminimal` and write or copy forward into the canonical directory.
- Routed plans, repairs, evidence, workflow-node runs, panic recovery, and GUI session browsing through the canonical runtime-path contract. Workspace scanners and tool guards hide or reject both the canonical and legacy private namespaces.
- Kept explicit `--state-dir` exact, so user-selected directories do not acquire an implicit sibling fallback.
- Documented the new defaults and legacy-read behavior in the README and bilingual CLI/configuration references.
- Added migration tests for canonical precedence, legacy-only runs, legacy sessions and history, canonical writeback, GUI dual-root discovery, private-path filtering, and the absence of a newly created `.anvil` directory on the covered direct-run path.

Existing historical event payloads and legacy paths remain readable; no historical evidence was rewritten.
