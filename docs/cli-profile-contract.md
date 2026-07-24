# CLI Profile Contract

Status: **draft for review**

## 1. Scope

Generate Python CLI tools. Deliverables are `cli/main.py`, `--help` output,
and usage examples. Runtime is `python3`, as for the data profile.

## 2. Full meaning

Generated CLI starts in representative cases, obeys declared exit-code rules
(normal `0`, input error non-zero), its `--help` output matches the implemented
interface by machine comparison, and output claims match observed output.

## 3. Required evidence

- C1 `cli_probe`: normal execution exits 0 with observed output; invalid input
  exits non-zero.
- C2 `help_binding`: compare actual `--help` options/arguments with the
  declared interface in both directions.
- C3 output-claim binding: README/examples and output examples match actual
  execution (the E2 claims-binding sibling).
- C4 rerun consistency (reuse E3).

## 4. Assurance

`full` requires C1–C4. `partial` requires C1 while C2–C3 are
`claims_absent`/static. `failed` is an exit-polarity violation or a binding
violation.

## 5. Conformance negative requirements

- reject help-listed but unimplemented options;
- reject an invalid input that exits 0;
- reject output examples absent from real output;
- reject assurance from an unexecuted probe.

## 6. Permanent out of scope

UX quality, option-design judgment, complete TTY interaction comparison
(the catalog coverage report marks this 🔴), and performance.

The contract is a draft; admission and any fix behavior require review.
