# E-3 gate: elapsed adjudication

## Primary record

The archived human-run log contains the two shell timestamps:

- start: `1784769464`
- end: `1784769482`
- calculation: `1784769482 - 1784769464 = 18 seconds`

This is the end-to-end circle measurement (the command that produced both
investigate and fix nodes, through the final acceptance line). The archived
`workflow-events.jsonl` and node event files do not contain epoch fields, so
they cannot independently provide a finer epoch subtraction. The investigate
node event stream has no numeric epoch; the fix node event stream contains
numeric values 1 through 7, yielding `7 - 1 = 6 seconds`.

## Current generator

`workspace/management/scripts/acceptance_sheet.py:161-181` first collects
numeric `epoch` values from the selected run's events and uses
`max(epochs) - min(epochs)`. For a circle, it then checks the sibling
`run1.log` and overwrites the value when two numeric shell timestamps exist.
The current archived run1 directory has no sibling log in the sheet fixture
layout, so its selected node events produce `7 - 1 = 6` seconds.

## Change point and interpretation

Commit `b62f964` introduced the epoch calculation and log fallback; the later
circle branch retained the node-event calculation while allowing the log path
to override when present. The earlier C-1a sheet's `18 seconds` came from the
outer run log timestamps above. The current `6 seconds` comes from the fix
node's event range. Both are real measurements of different scopes: 18s is
the circle end-to-end duration, while 6s is the observed node-event span.
The fixture expectation must not be changed until review chooses and labels
the contract definition; this report records the ambiguity without changing
the fixture.
