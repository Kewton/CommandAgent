# Known Limitations

CommandAgent is still in MVP migration.

- Full REPL slash-command execution for plan/ultra-plan commands is not fully
  wired yet. The parser, schemas, verifier, repair artifacts, profile contracts,
  and ultra execution core are present.
- `/ultra-plan-run` is intentionally phase-bounded. A failed phase stops the
  run and asks for explicit repair/replanning instead of continuing silently.
- Live LLM behavior depends on local model quality, quantization, provider
  reliability, and local toolchains.
- `Bash` is an offline policy guard, not a full OS sandbox. Commands run with
  the user's permissions.
- `.env` loading is not implemented inside CommandAgent. Export provider API
  keys in the shell or use an external env loader.
- The eval runner has dry-run wiring and real binary execution paths, but large
  semantic checks are still intentionally explicit and conservative.
- Provider API support is MVP-level. Ollama, Gemini, and OpenAI share one thin
  chat contract, but provider-specific tool surfaces beyond Ollama native tools
  are not implemented.
