# Install

CommandAgent ships release binaries from GitHub Releases. The release workflow
builds compressed binaries for Linux and macOS and publishes SHA-256 checksum
files next to each asset.

## Supported Assets

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `commandagent-linux-amd64.gz` |
| Linux arm64 | `commandagent-linux-arm64.gz` |
| macOS Intel | `commandagent-darwin-amd64.gz` |
| macOS Apple Silicon | `commandagent-darwin-arm64.gz` |

Windows binaries are not part of the current MVP release scope.

## Install Script

Install the latest release:

```bash
curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh | bash
```

Install a specific tag:

```bash
curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh \
  | COMMANDAGENT_VERSION=v0.1.0 bash
```

Install to a custom directory:

```bash
curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh \
  | COMMANDAGENT_INSTALL_DIR="$HOME/bin" bash
```

The installer downloads the platform asset and `.sha256` file, verifies the
checksum, expands the binary, and installs it as `commandagent`.

## Manual Install

Download the asset and checksum for your platform from the release page, then:

```bash
shasum -a 256 -c commandagent-linux-amd64.gz.sha256
gzip -dc commandagent-linux-amd64.gz > commandagent
chmod +x commandagent
mkdir -p "$HOME/.local/bin"
mv commandagent "$HOME/.local/bin/commandagent"
```

Use the asset name that matches your platform.

## Build From Source

```bash
cargo build --release
target/release/commandagent --help
```

MSRV is not fixed yet. Until Issue #1 is resolved, source builds should use the
current stable Rust toolchain.

### Local Development Alias

For a checkout-specific development binary, create a symlink from a directory on
`PATH` to the release build output:

```bash
cd /Users/maenokota/share/work/github_kewton/CommandAgent-develop
cargo build --release
mkdir -p "$HOME/.local/bin"
ln -sfn "$PWD/target/release/commandagent" "$HOME/.local/bin/commandagentdev"
commandagentdev --version
```

This matches the local `anvildev` setup pattern. The symlink keeps pointing at
the same checkout's `target/release/commandagent`, so later
`cargo build --release` runs update what `commandagentdev` executes without
recreating the symlink.

## Provider Setup

Ollama uses the local `OLLAMA_HOST` endpoint:

```bash
OLLAMA_HOST=http://127.0.0.1:11434 commandagent --provider ollama --model <model>
```

Gemini and OpenAI API keys must be supplied by the environment or by an
external env loader. CommandAgent does not load `.env` internally.

```bash
export GEMINI_API_KEY=...
export OPENAI_API_KEY=...
```
