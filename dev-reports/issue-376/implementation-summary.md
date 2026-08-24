# Issue #376 implementation summary

- Added a read-only, bounded task projection to GUI Trial session detail. It
  consumes only Issue #375 schema-version-1 `plan_step_*` events and never
  derives task success from phase or stream ordering.
- Kept each `plan_execution_id` as a separate execution interval and each
  `step_execution_id` as a separate task, including repeated Step IDs across an
  initial request and later continuation.
- Exposed current phase, current task, and task position while running, plus
  completed, short-circuited, failed, and interrupted terminal states with both
  text and symbols.
- Added a shared status/detail task view with native keyboard disclosures,
  ordered headings, synchronized `aria-expanded`, failed-task auto-expansion,
  bounded failure diagnostics, and navigation to `events.jsonl` evidence.
- Kept Trial history rows compact. Legacy, malformed, oversized, and incomplete
  terminal streams now show an explicit unsupported state without fabricated
  task totals or success counts.
- Preserved conditional ETag polling and omitted raw event records. Focused
  tests cover roughly 100 tasks and both smoke payloads remain below 128 KiB
  while rendering only the bounded projection.
- Updated the GUI Trial guide, shell design, mechanism ledger, changelog,
  read-only guards, GUI-server contract tests, and dual-base-path browser smoke.
