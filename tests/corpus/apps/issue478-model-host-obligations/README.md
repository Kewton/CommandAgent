# Issue #478 formation corpus

`attempt-2.json` and `attempt-3.json` contain the exact saved `model_proposal`
objects from `reports/initial-contract-failure.json` under the read-only
`20260913-integrated-r0-evaluation-452-468-01` run. Their long goal is intentional:
the actual host sees the phase/scaffold context before the sanitizer bounds it.
The smoke instruction has 587 characters and SHA256
`75c69f8375ad4c8492158c3f65745a6532a82f26850cab51901015e8faa492e0`.

The Rust `issue478` tests feed these proposals to the product Runner using a
deterministic ChatClient. A temporary package manifest, node_modules directory,
and App Router entrypoint model the already-scaffolded formation context. These
are planner dependency-context markers; no installed Next.js or successful app
build is claimed. No live model regeneration is involved.

`refusals.json` drives model duty and ordering controls in that same Runner.
Additional actual formation/admission tests remove host guidance, result,
outputs or executable ownership, including fallback. A runtime-policy control
keeps the broad Profile owner out of the package-check short circuit. The
generic corpus acceptance marker only verifies fixture integrity; semantic
acceptance is asserted by the focused Rust tests.

`package-checks.json` records the pre-fix actual preset conversion's three
package checks. The formed Implement owner must retain these exact commands.
The runtime control executes them against a valid manifest, then verifies that
an incorrect requested port still fails.
