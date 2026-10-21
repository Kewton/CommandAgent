# Saved failed-core scaffold boundary (#485)

The original R0 session is `01a09b8b-f70e-7aa1-84f1-40a18d62da17`, historical
HEAD `b7b65f8fd75793f2075491889c66372b02a06d14`. The regression is implemented
against #484-integrated parent `0a8222643683c764fa5f9ac8bd3e5b2b47ef7728`.
`source-manifest.json` records the original paths, SHA-256 hashes, central event
positions (one-based), line hashes including newline, and reduction mappings.
Those historical absolute paths are provenance only; execution needs this corpus
and the current test binary. No historical files are changed.

`scaffold/` contains the eight required source/configuration files plus
`next-env.d.ts`, copied byte-for-byte. Package locks, dependencies, build outputs,
runtime metadata, screenshots, timing and unrelated logs are omitted. The exact
original completion contract and UltraPlan retain their goal, required paths,
capabilities, evidence, obligations, commands and phases. The failed-core
boundary records setup completion, core planning exhaustion after three proposals,
no accepted core steps, and the remaining phases. It is a frozen input boundary;
the corrected #484 planner is not required to reject its now-supported proposal.

`expected-events.jsonl` is a stable projection of the recorded events. The first
two records describe the frozen boundary; the subsequent records are matched in
order against newly emitted production events. Tests do not inject these events
or compare random session IDs/timestamps. Control hashes are compared to freshly
measured source hashes rather than the historical workspace's hash.

`observations/saved-scaffold-interaction.json` retains the original successful
generic interaction fields (including its heuristic mode and missing hooks).
This observation cannot prove project-management behavior. `business/` is a
small, synthetic route-bound implementation of project creation, dated tasks,
member assignment and status filtering. Its observation input is explicitly
scripted and carries usable contract hooks; it is a controlled acceptance input,
not a measured browser result or an external R0 success.

`runtime.sh` supplies deterministic npm build exits, a disposable HTTP child and
the existing `cfg(test)` interaction input seam. Every scripted build compares
the source bytes to the case's frozen source snapshot. These are transport
responses, not replacement acceptance decisions or a real compiler run. There
is no dependency download, live model, browser/GUI evaluation or shared server
operation. Production code still performs classification, verification dispatch,
runtime acceptance, isolated preflight, control audit, bounded Recovery
suppression, terminal projection and normal final acceptance.

Controls are independent:

- Saved scaffold: registered build and generic interaction pass; implementation
  evidence is missing. The real driver suppresses Recovery at 0/2, control is
  unchanged, no restore occurs, and the process ends failed with final acceptance
  not checked.
- Business implementation with all scripted contract observations: evaluate
  CurrentSuccess and ordinary final acceptance separately; require every gate.
- Same business source with missing interaction, and separately failed build:
  keep the unrelated failure even though implementation evidence exists.
- Scaffold plus an unrelated health API and missing interaction: observe the API
  as an implementation artifact independently of the still incomplete goal.

This corpus does not establish persistence, live business operations, a completed
historical core phase, external R0 acceptance, CI or exact-HEAD UAT. Those remain
separate observations owned by the parent workflow.

Transport isolation and terminal layers

The original contract, package scripts and registered observer still require
60302. Each test uses an OS-assigned loopback port for its HTTP child via the
existing `browser-probe-command.json` test override (`require_build=true`). A
`cfg(test)` leaf hook carries only that override into the disposable observation.
This avoids both intra-test competition and collisions with other tests or the
user's service at 60302. The sandbox cannot bind even these test-only sockets;
the required tests therefore run outside it. No shared processes are stopped.

The saved lifecycle includes a TUI stop before process stop. The test invokes
the real TUI emitter with the actual failed drive result, then the real process
emitter; it does not inject the saved event. Final controls separately record
VerificationReport, runtime acceptance, release/final fields and task projection.
Missing interaction may retain a passing VerificationReport and partial release
gate/task. A failed build may retain a release-derived `full_success` field when
that gate is not applicable, while external verification and the task fail.
Neither one field nor command completion substitutes for full task acceptance.
The negative controls assert those remaining gates explicitly.
