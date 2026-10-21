# Production Recovery finish contract

`cases.json` drives `cargo test --lib issue458`. The harness delegates preflight,
YAML/resume validation, start/snapshot/treatment binding, finish, control retention
and promotion to `RunnerRecoveryDriver`. Only execution outcomes and attempted
file edits are injected. Registered commands execute normally in isolated
observations; rejection must keep the control source hash and begin the next
treatment from those original bytes.

The corpus covers limits 0/1/2, two eligible failures, execution/verification to
success, repeated plans, interruption and unsupported error kinds. It fixes the
ordered event boundary for rejection, continuation publication and later success.
The existing legacy-path regression separately asserts that configured 0 emits
no automatic events. Additional tests execute a noisy failing Python checker:
varying elapsed time and observation cwd must still cycle-stop, while changed
semantic command output allows another attempt. Compiler-code and profile-reason
controls ensure the identity does not collapse distinct diagnoses.

The readiness matrix exercises the production classifier with typed observations.
A separate real readiness probe uses an explicitly scripted local npm fixture to
return build success and then exit on startup; its unavailable classification
does not establish a Next.js build result or model repair success.

These are new deterministic fixtures, not edits or reclassifications of historical
#452 evidence. Live-model efficacy is unmeasured.
