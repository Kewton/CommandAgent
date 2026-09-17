# Issue #492 verification

- Status: `blocked`

The source and fixture checks are being finalized. The final committed-source
real Next.js run and its exact binary identity are required before this report
can be marked passed. The earlier real-05 core controls passed all five cases;
they are development observations, not the final committed-source identity.

Baseline: `d51cc6ed9e04f8d9747d015ff788845ea1b1e59b`.
All runtime output is below
`/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/runtime/`.

Every Cargo/test process uses:

```text
CARGO_TARGET_DIR=/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/target
CARGO_HOME=/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/cache/cargo
TMPDIR=/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/tmp
npm_config_cache=/Volumes/SSD_NX/tmp/commandagent-orchestrate-492-20260918-01/cache/npm
```

The real controls also use separate apps and runtime directories, unset NODE_ENV
and NODE_OPTIONS, retain devDependencies, and restore the selected SSD TMPDIR in
the observation wrapper's real npm child. No HOME or production allowlist change
is needed.

The new B0 preservation assertion failed on unchanged production source (exit
101, `runtime/pre-fix.log`), demonstrating the original instruction/check/path
replacement. Its current expectation preserves the same sanitized input. The
historical JSON SHA-256 remains
`e15f1024da83e208592cb9375fd76fad78a6b10041e4320b6f1d456960a6891e`.

Development attempts remain separated: real-01 stopped at dependency-copy
identity validation; real-02 stopped at Write authorization; real-03 failed in
the observer because custom child variables were filtered. None is an R1/R2
compiler verdict. real-04 observed real builds but used a non-preset enclosing
plan and is not conversion-path proof. real-05 used the actual preset/core
prepare/executor and passed every case. real-06's stricter input-hash assertion
detected package.json key-order normalization; values were identical. The final
fixture uses the product's canonical JSON ordering so byte hashes stay fixed.
Initial full-suite test audit failures were test harness issues (profile literals
and direct command validation); they were corrected without changing guardrail
baselines or command policy. `full-test-initial.log` retains that failed audit.
