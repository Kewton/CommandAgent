# CLI Profile Contract

Status: **fixed (2026-07-24)**

## 1. Scope

Generate Python CLI tools. Deliverables are `cli/main.py`, `--help` output,
and usage examples. Runtime is `python3`, as for the data profile.

## 2. Full meaning

The generated CLI starts in representative normal and abnormal cases, obeys
declared exit-code rules (normal `0`, input error non-zero), its `--help`
output matches the implemented interface by machine comparison, and output
claims match observed output. Representative cases are mechanically derived
and bound before execution; shrinking or replacing the bound set is forbidden
(the same freeze rule as F3). The model cannot choose cases after the fact.

## 3. Required evidence

All evidence is anchored in execution observations, not prose or source text.

- C1 `cli_probe`: normal execution exits 0 with observed output; invalid input
  exits non-zero.
- C2 `help_binding`: compare actual `--help` output with actual accepted
  behavior and the declared options/arguments in both directions. Source
  analysis is only auxiliary; the execution observation is the anchor.
- C3 output-claim binding: README/examples and output examples match actual
  execution (the E2 claims-binding sibling).
- C4 rerun consistency (reuse E3).

## 4. Assurance

- `full`: C1–C4 all succeed.
- `partial`: C1 succeeds and C2/C3 were executed but are `claims_absent`.
- `static`: probes were not executed, including C1.
- `failed`: polarity violation (for example invalid input exits 0), C2/C3
  binding violation, or C4 rerun mismatch.

C4 not executed cannot earn `full`; assurance is earned only from the required
evidence.

## 5. Conformance negative requirements (six)

- reject help-listed but unimplemented options;
- reject an invalid input that exits 0;
- reject output examples absent from real output;
- reject assurance from an unexecuted probe;
- reject C4 rerun mismatch;
- reject shrinking or replacing the bound case set.

## 6. Permanent out of scope

UX quality, option-design judgment, complete TTY interaction comparison (the
catalog coverage report marks this 🔴), and performance.

## 7. Generation constraints

Generated CLIs must be deterministic: no dependence on time, randomness, or
network, as required for C4. `--help` must derive from a standard mechanism
such as `argparse`; separating hand-written help text from an `if` chain is a
C2 violation hazard.

This fixed contract governs the E-3 adjudication; fix behavior remains subject
to review.
