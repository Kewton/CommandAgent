# Issue #441 review resolution design

Review target: `7c4940b07011d36a99700b530a3d4a8565e6164b`.
Read the independent `relative-prefix-diagnostic.log` under the read-only
`20260908-0058-issue441-review-preparation` run and the development guardrails.
The original design, implementation summary, and verification report remain
historical records and will not be edited.

The current suffix check ends at the first whitespace, including whitespace
inside quotes or escaped as part of the path. Reproduce through the actual
`display_text` function with a new Issue-specific fixture before changing the
implementation. Include single/double quotes, escaped spaces and characters,
quote concatenation, and safe paths containing spaces.

Replace this display-only suffix boundary heuristic with a conservative shell
word scan that tracks quoting and escapes through the whole candidate suffix.
Check decoded parent-directory components before replacing a root prefix; keep
ambiguous or incomplete syntax unnormalized. Require a component boundary in
the decoded suffix as well. Preserve the original spelling in successful output
and leave runtime path confinement and execution authority unchanged.

Run the reproduction first, then focused repair/Issue #441 and existing #428
tests, corpus/guardrail/conformance checks, fmt, clippy, the full all-target Rust
suite, and doctests. Add a separate review-resolution verification record and
commit only this correction, its fixture/tests, and the new reports.
