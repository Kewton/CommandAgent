# Known Limitations

CommandAgent is still in MVP migration.

- REPL slash-command execution is wired for plan and ultra-plan commands.
  Simple docs, Python, Rust, Next.js file-set, planner/executor split, and
  repair fallback live UAT passes with local Ollama models, but complex
  `/ultra-plan-run` workflows still need release sign-off.
- `/ultra-plan-run` is intentionally phase-bounded. A failed phase stops the
  run and asks for explicit repair/replanning instead of continuing silently.
- Live LLM behavior depends on local model quality, quantization, provider
  reliability, and local toolchains.
- `Bash` is an offline policy guard, not a full OS sandbox. Commands run with
  the user's permissions.
- `.env` loading is not implemented inside CommandAgent. Export provider API
  keys in the shell or use an external env loader.
- The eval runner has dry-run wiring, real binary execution paths, per-case
  `/plan-run` / `/ultra-plan-run` mode, and fixture seeding for modification
  cases. Large semantic checks are still intentionally explicit and conservative;
  the updated large suite needs a fresh real run before it can be treated as a
  release-quality gate.
- Provider API support is MVP-level. Ollama, Gemini, and OpenAI share one thin
  chat contract, but provider-specific tool surfaces beyond Ollama native tools
  are not implemented.
- Smaller planner models may fail to follow the strict plan YAML schema even
  after one correction attempt. Use a stronger planner model for MVP workflows
  until frontier data is collected.
