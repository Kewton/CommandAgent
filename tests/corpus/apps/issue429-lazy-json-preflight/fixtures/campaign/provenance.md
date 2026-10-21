# Reopened Issue #429 campaign source fixture

The complete `src` trees are byte-for-byte copies from:

- `/Volumes/SSD_NX/tmp/commandagent-capability-0905.vkR3MS/S1/workspace/src`
- `/Volumes/SSD_NX/tmp/commandagent-capability-0905.vkR3MS/S3/workspace/src`
- `/Volumes/SSD_NX/tmp/commandagent-capability-0905.vkR3MS/E3/workspace/src`

Copied on 2026-09-07. `source-sha256.json` records every relative source path and
SHA-256. Tests verify all hashes when loading the corpus. No runtime data,
credentials, dependency tree, build output, or historical event log was copied.
The original source trees and evidence remain unchanged.

S1's staff routes contain a template and direct `writeFileSync` calls; its shifts
routes also contain regexes. S3's storage includes an ID template and passes
STAFF_FILE/SHIFTS_FILE through `writeFileSafe`. E3 uses the generic
`writeJSON<T>(filePath, data)` helper with DEPARTMENTS_FILE/EXPENSES_FILE.
Expected exact paths are staff/shifts for S1/S3 and departments/expenses for E3.

Static tests build a temporary project with this unchanged source and a synthetic
empty package manifest. Preflight tests replay synthetic JSON creation/update
and business-failure/source-mutation effects in the real isolated observer.
They do not execute or repair these Next.js applications. The original synthetic
plain-JS GET fixture still executes its own lazy writers. Neither test family is
a fresh campaign, production build, browser observation, domain integrity result,
or claim that these historically failing applications are now successful.

`../campaign-preflight.jsonl` contains expected policy event projections, not
historical measurements. `fixture_run` is a test-only selector and is not added
to the product event schema. Tests compare existing event fields and exercise the
existing Recovery driver emission for pass/fail/unavailable outcomes.
