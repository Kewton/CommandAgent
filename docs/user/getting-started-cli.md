# Getting started with the CLI

Learning path:
[README](../../README.md) → Getting started (this page) →
[Detailed tutorial](../guide/en/tutorial.md) →
[CLI reference](../guide/en/cli-reference.md)

日本語の順路:
[README](../../README.ja.md) → CLI 入門（このページ）→
[詳細チュートリアル](../guide/ja/tutorial.md) →
[CLI リファレンス](../guide/ja/cli-reference.md)

[Ingest four-gate walkthrough](first-loop.md) |
[Configuration](../guide/en/configuration.md)

This is the shortest path from a new checkout to one inspectable CommandAgent
run. Use it for the terminal product. GUI operators should instead begin with
[Getting started with the GUI](getting-started-gui.md).

## 1. Install or prepare the checkout

Choose one installation path:

- From a trusted checkout, `cargo install --path .` builds and installs the
  `commandagent` product binary.
- [`scripts/install.sh`](../../scripts/install.sh) downloads a release binary and verifies its SHA-256
  before installation. Download and inspect the script before running it.
- [`scripts/setup.sh`](../../scripts/setup.sh) prepares a source checkout, prerequisites, completions,
  and optional providers. `./scripts/setup.sh --check-only` changes nothing;
  `./scripts/setup.sh --yes` accepts its safe non-interactive defaults.

Confirm the binary you will run:

```bash
commandagent --version
commandagent --help
```

The similarly named `commandagentdev` is a development launcher and
`commandmatedev` is a separate CommandMate orchestration CLI. Neither is a
substitute for the product binary in this guide.

## 2. Choose a provider and exact model ID

For a local Ollama server:

```bash
ollama list
commandagent --provider ollama --model "<model-from-ollama-list>" --doctor
```

For LM Studio, start its server and use the exact model ID it exposes. Gemini
and OpenAI require their documented API key in the CommandAgent process
environment. Provider credentials must not be placed in goals, command-line
arguments, config files committed to the project, or run reports.

See the bilingual [provider guide](../guide/README.md) for all provider/host and
planner-role combinations.

## 3. Add configuration only when it removes repetition

CommandAgent works without a config file. A named preset belongs in either the
workspace or user `config.toml`:

```toml
[preset.local_cli]
provider = "ollama"
model = "<exact-model-id>"
planner_provider = "ollama"
planner_model = "<exact-model-id>"
profile = "python-cli"
```

The canonical search starts at `.commandagent/config.toml`; existing
`.anvil/config.toml` files remain supported fallbacks. Configuration is
field-by-field and explicit CLI flags win. Read
[configuration precedence](../guide/en/configuration.md#resolution-precedence)
before combining top-level values and presets.

## 4. Run the offline doctor

`--doctor` diagnoses configuration, provider readiness, local probes, external
draft profiles, and extension roots without making network requests:

```bash
commandagent --preset local_cli --doctor
commandagent --preset local_cli --doctor --json
```

Resolve each `ng` result before starting a mutating run. JSON is intended for
automation; it does not relax any readiness or acceptance gate.

## 5. Complete the first loop

Move to a trusted workspace, keep approval interactive, and start the REPL:

```bash
cd /path/to/trusted/project
commandagent --preset local_cli
```

Use the same sample goal as the recorded tutorial and the GUI first-run card:

```text
commandagent> Create a CLI --pattern filter command
commandagent> /confirm sha256:<card-hash>
```

Replace `<card-hash>` with the exact value shown after you read Gate 1. At the
end, distinguish the process exit status, the final verdict, and the earned
assurance level. Continue with the [detailed tutorial](../guide/en/tutorial.md)
for the recorded CLI/GUI flow or the Japanese
[ingest four-gate walkthrough](first-loop.md) for a profile-specific example.
For another process, add `--summary-json` and follow the stable
[headless handoff](headless.md).

## 6. Compare one pack variable at a time

List compatible, exact-version choices before selecting one:

```bash
commandagent --profile python-cli --intent create --packs
commandagent --extension-root /path/to/extensions \
  --profile python-cli --intent create --packs
```

For an A/B comparison, hold the goal, workspace fixture, profile, intent,
models, and verification suite fixed. Run A with one exact pack pin and run B
with the other:

```bash
commandagent --profile python-cli --intent create \
  --pack cli-assist@1.0.0 --plan-run "Add a bounded filter command."
commandagent --profile python-cli --intent create \
  --pack cli-assist@1.1.0 --plan-run "Add a bounded filter command."
```

Compare the persisted Gate 1 identity, pack hash, events, acceptance sheet, and
the same suite outcome. Do not call an unadmitted local pack approved merely
because conformance passed. The complete scaffold/pin experiment is documented
in [first-loop — pack A/B](first-loop.md#2-packを1パラメータだけ変えてabする).

## Glossary / 用語集

| Term | Meaning |
| --- | --- |
| Gate 1 | Pre-execution confirmation of the exact goal, identity, write boundary, and required checks. |
| Gate 2 | The delegated execution and file-backed monitoring period. It is not acceptance. |
| Gate 3 | Terminal evidence satisfied the fixed acceptance contract. Read the verdict and assurance separately. |
| Gate 4 | Required evidence is missing or failed; preserve evidence and choose an explicit recovery or close action. |
| profile | The domain contract that owns plan shape, checks, evidence vocabulary, and inference. |
| intent | The requested operation class: `create`, `fix`, or `investigate`. |
| pack | Exact-version, exact-byte pinned additive guidance/check knowledge compatible with a profile and intent. |
| assurance level / 保証水準 | The strongest level actually earned from evidence; `static`, `partial`, or `full` is not interchangeable with the verdict. |
