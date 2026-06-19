# Known Limitations

CommandAgent is still in MVP migration.

- REPL slash-command execution is wired for plan and ultra-plan commands.
  Simple docs, Python, Rust, Next.js file-set, planner/executor split, and
  repair fallback live UAT passes with local Ollama models, but complex
  `/ultra-plan-run` workflows still need release sign-off.
- `/ultra-plan-run` is intentionally phase-bounded. A failed phase stops the
  run and asks for explicit repair/replanning instead of continuing silently.
  If the failure is only `dependency_missing`, approved online runs may perform
  one setup attempt and verifier retry. Without approval, in offline mode, or
  after exhausted setup recovery, the run stops as a setup blocker instead of
  suggesting a repair replan.
- Tool-call schema failures are now classified separately from verifier and
  profile failures. A missing required field such as `Write.path` can receive
  one strict current-step protocol correction for eligible file-changing initial
  steps, and repair turns may also correct malformed `Write` or `Edit` calls
  while fixing a failed verifier. Repeated malformed tool calls still stop as
  `tool_args_*`. This improves protocol recovery and attribution but does not
  guarantee app-quality completion.
- Phase-boundary profile verification is deterministic and read-only. It can
  catch structural contract drift such as Next.js app-root ambiguity, package
  script drift, and Tailwind dependency/config mismatch, but it does not score
  UI quality semantically and does not auto-resume the original ultra plan
  after a standalone repair command. Profile obligations can steer generated
  phase step plans away from known package contract drift and Next.js
  route-integration drift for explicit UI/game artifacts. Active step/repair
  prompts now carry refreshed profile facts, but these are narrow
  deterministic facts, not a full domain workflow or generic artifact graph.
- Terminal progress is best-effort and TTY-aware. It improves visibility into
  plans, blocking planner/model/tool waits, steps, verifiers, artifact status,
  and repair packets, and includes an interactive startup logo. It is
  presentation-only and does not provide a fixed footer, readline history and
  completion, or signal-aware Ctrl-C resume messaging yet.
- Live LLM behavior depends on local model quality, quantization, provider
  reliability, and local toolchains.
- `Bash` is an offline policy guard, not a full OS sandbox. Commands run with
  the user's permissions.
- Dependency setup recovery currently supports only npm/pnpm lockfile evidence
  for `npm run build` style Next.js verification. It does not support Yarn,
  `npx`, arbitrary package-manager commands, or model-issued install steps.
- `.env` loading is not implemented inside CommandAgent. Export provider API
  keys in the shell or use an external env loader.
- The eval runner has dry-run wiring, real binary execution paths, per-case
  `/plan-run` / `/ultra-plan-run` mode, and fixture seeding for modification
  cases. Large semantic checks are still intentionally explicit and conservative;
  the latest fresh large run is 0/6 and needs triage before it can be treated as
  a release-quality gate.
- Provider API support is MVP-level. Ollama, Gemini, and OpenAI share one thin
  chat contract. Gemini and OpenAI use the shared XML fallback tool-call
  contract; provider-specific native tool surfaces beyond Ollama native tools
  are not implemented.
- Smaller planner models may fail to follow the strict plan YAML schema even
  after one correction attempt. Use a stronger planner model for MVP workflows
  until frontier data is collected.
