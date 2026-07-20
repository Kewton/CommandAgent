# CommandAgent Dev Container

Open the repository in a Dev Container and run the same local, non-networked
checks as CI:

```bash
just ci
```

The image contains Rust 1.94.1, Node.js LTS, Python 3.12, `just`, `shellcheck`,
and the Python packages pinned by CI. `just ci` runs local tests and fixtures;
it does not contact a model provider. Dependency installation during the
initial container creation still requires network access.

## Connecting to host Ollama

Ollama is deliberately not installed in this container. Run Ollama on the
host, make it reachable from Docker, and point CommandAgent at the host gateway:

```bash
COMMANDAGENT_OLLAMA_HOST=http://host.docker.internal:11434 just run
```

The equivalent direct invocation is:

```bash
cargo run -- --provider ollama \
  --model "qwen3.6:27b-coding-nvfp4" \
  --ollama-host http://host.docker.internal:11434
```

Docker Desktop provides `host.docker.internal`; the Dev Container also adds
the standard `host-gateway` mapping for Linux Docker. If Ollama listens only on
the host loopback interface, configure its `OLLAMA_HOST` setting to listen on
an interface reachable by Docker and apply appropriate firewall restrictions.
