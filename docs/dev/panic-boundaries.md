# Panic boundaries

Status: fixed design rule (2026-07-30)

CommandAgent treats model output, workspace artifacts, evidence, runtime
configuration, provider responses, and operating-system results as untrusted
run input. A failure while handling those values must become a typed error,
verification failure, violation, or honest terminal event. It must not unwind
the process.

## Layers where panic is permitted

Panic-producing `unwrap`/`expect` is permitted only when all input is owned by
the repository and failure proves a programmer invariant:

1. tests, fixtures, and snapshot construction;
2. shipped static definitions such as literal regular expressions;
3. embedded fixed manifests whose parsing and validation are exercised by CI;
4. operations whose type contract is infallible for the supplied
   repository-owned value, with the reason stated at the call site.

These failures belong to code review and CI. They must not be converted into a
model-attributed run class.

## Layers where panic is prohibited

The following production boundaries must return typed failure:

1. adjudication and terminal-reason selection;
2. completion and assurance projection;
3. profile registry resolution from runtime configuration;
4. comparators parsing or binding model/workspace/evidence values;
5. filesystem, process, network, environment, and provider operations.

A collection access guarded by a local predicate is lower risk, but it is not
a new permitted layer. Such sites should migrate to explicit match/typed
failure when touched. The E-5e baseline and the bounded remainder are recorded
in `workspace/management/runs/e5e-debt-audit.md`.
