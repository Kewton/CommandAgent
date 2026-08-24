# Builder Plane operations

This runbook is the standard operator path for a headless Community Mini App
campaign. It keeps provider readiness, instrument identity, disposable
workspaces, delivery packaging, and later reverification as separate gates.

## 1. Bootstrap without leaking credentials

The repository `.env` is a local input and must never be committed. Sourcing it
without export does not make `OPENAI_API_KEY` visible to child processes. Use
shell auto-export only around the source operation, and never print the value.

```sh
test -f .env
chmod 600 .env
set -a
source .env
set +a
test -n "${OPENAI_API_KEY:-}"
```

That last check returns only a status code. `env`, `set`, shell tracing, or an
`echo` of the key must not be placed in logs. Doctor output is safe to retain
because credential values are redacted:

```sh
target/release/commandagent --doctor --json \
  --provider openai --model gpt-5.6-luna \
  --planner-provider ollama --planner-model qwen3.6:27b-coding-nvfp4 \
  --ollama-host http://127.0.0.1:11434
```

Doctor checks configuration shape. The bench preflight below is the spending
gate: it checks the OpenAI key plus `/v1/models`, the configured Ollama
`/api/tags` endpoint plus the exact planner model, and the release binary SHA.

## 2. Use a disposable campaign root

Create the root outside the repository. By default, the bench installs the
release binary under the campaign's own `bin/`; it does not write
`~/.local/bin` unless the operator explicitly selects that path.

```sh
CM_CAMPAIGN_ROOT=$(mktemp -d /private/tmp/community-builder.XXXXXX)
python3 workspace/management/scripts/bench.py run \
  --suite workspace/management/bench/suites/community-cm3-arm-a-baseline.toml \
  --workspace-root "$CM_CAMPAIGN_ROOT"
```

The qwen3.6 baseline remains the operationally sealed configuration. It omits
`think`, so no think field is sent. The qwen3.8 E/F candidates are comparison
results only; changing the operational default awaits the owner decision.

Preflight happens before the first campaign run directory and provider spend.
An unreachable OpenAI or Ollama endpoint stops with
`provider_unreachable_preflight`; a binary SHA mismatch also fails closed. In
either case, do not count an application run, and inspect the preflight record
before retrying. Environment interruptions are not silently relabeled as model
failures.

For a pinned series, add its already-issued `--bon-predeclaration` file. The
bench then requires built, installed, and executed binary SHA-256 equality
before spend. `--binary-dir` is available only when an explicit alternate
campaign-local location is required.

## 3. Observe and archive without cross-run state

Every headless process must receive unique workspace, state, events, and run
identifiers. Do not share one `.anvil` state directory between concurrent
workers. The CM-4 four-process probe observed four unique owner roots and zero
foreign path references; it does not justify sharing mutable paths.

After execution, scan each delivery candidate before archiving it:

```sh
python3 workspace/management/scripts/bench.py scrub \
  --path "$CM_CAMPAIGN_ROOT"
```

Retain the issued suite, suite SHA, binary SHA, model metadata, summary JSON,
events, acceptance evidence, and artifact hashes. Raw logs or campaign binaries
that fail the scrub/size policy remain outside the repository; retain their
hashes and curated excerpts instead.

The campaign root is disposable only after required evidence has been copied,
scrubbed, hashed, and independently opened. Deletion is an explicit operator
action against the exact campaign path, never a recursive repository-root or
home-directory cleanup.

## 4. Package one R2 delivery unit

For a successful run, create the portable bundle with the source campaign
metadata, the source verification event stream, and the exact campaign binary:

```sh
workspace/management/scripts/community_bundle.py create \
  --source-artifacts <campaign>/artifacts/<run> \
  --source-events <campaign>/artifacts/<run>/.anvil/runs/<verification-run>/events.jsonl \
  --source-run <run> \
  --campaign-summary <curated-campaign-summary.json> \
  --campaign-meta <campaign>/uat-meta.json \
  --binary <campaign>/bin/commandagent \
  --binary-sha256 <declared-series-sha256> \
  --output <new-delivery-bundle>
```

The generated manifest inventories every delivery file by relative path, size,
and SHA-256. An L2 bundle records promotion as `not_applicable_l2` with no
evidence claim. An L3/L4 bundle must carry actual `promotion_decision` evidence;
absence fails closed.

## 5. Reverify later

Use the same pinned product binary and the current reference verifier:

```sh
workspace/management/scripts/community_bundle.py reverify \
  --bundle <delivery-bundle> \
  --binary <pinned-commandagent> \
  --reference-validator workspace/management/scripts/community_profile.py
```

The command validates manifest hashes and the instrument hash before running
anything, then reruns product S/Z and the Python parity check. B is run for
L3/L4 and remains explicitly not applicable for L2. Success requires the
rederived verdict to equal the original manifest verdict. A changed artifact,
unlisted file, wrong binary, missing promotion record, or verdict difference is
a violation, not a warning.

## 6. Incident triage

- `provider_unreachable_preflight`: environment; no application run was
  consumed. Check the recorded host and probe kind without printing secrets.
- binary or suite SHA mismatch: instrument/input identity failure; do not spend.
- a Community `stop_class`: preserve generated artifacts and events, classify
  against `classes.toml`, and never weaken S/Z/B to turn it green.
- parallel failure with unique roots and zero foreign references: quality or
  provider behavior, not automatically an isolation defect.
- summary model ID or think mismatch: drift probe failure; stop the series.

The terminal `--summary-json` line is the caller contract. Consumers should use
its `verdict`, `stop_class`, paths, provider cost, and model metadata instead of
parsing prose output.
