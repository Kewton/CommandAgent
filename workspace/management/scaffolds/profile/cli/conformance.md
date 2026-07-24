# CLI conformance binding

Canonical contract: `docs/cli-profile-contract.md` (fixed).

Synthetic evidence cases:

- full C1–C4 pass: accept as earned `full` before the admission cap;
- help-listed but unimplemented option: reject;
- invalid input exiting zero: reject;
- usage output example absent from observed stdout: reject;
- C1 probe not executed: reject `full` and classify `static`;
- normal-case rerun mismatch: reject;
- frozen case set shrunk or replaced: reject.

Executable fixture:
`tests/corpus/apps/test0725_cli_profile_contract/fixtures/conformance.jsonl`.
The profile remains `admission = "off"`.
