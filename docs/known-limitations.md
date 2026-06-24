# Known Limitations

CommandAgent is still in MVP migration.

- Focused control-recovery eval now has an explicit matrix row and proof mode
  for adopted control paths. Real-LLM rows prove ordinary workflows; deterministic
  fixture rows prove hard-to-force failure classification and report projection.
  Fixture rows are eval-only evidence contracts, not proof of model task quality
  or runtime implementation success. Dry-run execution still intentionally skips
  expected assertions because no model/runtime evidence is produced.
- Broad migration sign-off now has an eval-only checker that reads normal and
  recheck summaries for smoke, focused, fixture, and large roots. The checker
  flags unknown/raw failures, focused assertion failures, generic source
  fallback, and missing large-case owner/action/target/evidence fields. It does
  not run CommandAgent, change runtime policy, retry cases, or prove visual/UI
  quality.
- Gold Plan large cases now exist to separate planner quality from
  worker/tool-interface behavior. They bypass plan generation with checked-in
  `/run-plan` inputs, but they still depend on the local model to execute the
  steps correctly and on local toolchains/dependencies for the verifier.
- Eval timeouts are now recorded as `provider_transport:eval_timeout` rows so
  broad runs can leave a root and report instead of crashing the harness. A
  timeout row is a blocker/evidence boundary, not a successful local LLM
  migration sign-off.
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
  while fixing a failed verifier. The correction action now narrows the allowed
  tools for same-tool schema/JSON correction, stale-edit read-before-edit, and
  prose-only repository-evidence recovery. Unsafe paths and provider-transport
  parse failures stop with structured protocol evidence when no safe shared
  action exists. Repeated malformed tool calls still stop as `tool_args_*`.
  This improves protocol recovery and attribution but does not guarantee
  app-quality completion.
- Repair packets now carry structured contract evidence for provider transport
  parse failures, tool protocol failures, read-only step-policy violations,
  verifier failures, and selected profile verification failures. Transport
  evidence is limited to shared parser diagnostics, not provider/model-specific
  behavior. Verifier/profile evidence can include a failure signature, repair
  target, candidate artifacts, related source excerpt, and bounded attempt
  ledger. This makes standalone repair inputs clearer, but it does not increase
  retry budgets, auto-resume failed phases, or score UI/game quality.
- Runtime and eval now share a `FailureObservation` taxonomy for terminal
  states, producers, contract layers, source of truth, diagnostics, and
  actionability. Unknown states remain explicit and eval reports flag raw
  `rc:1`-style failures without diagnostics as observation coverage defects.
  This improves attribution for later recovery work, but it is not an active
  job arbiter, target selector, or repair-action planner.
- Verifier failures now carry a bounded deterministic diagnostic payload when
  output can be classified, including diagnostic code, observed/expected pairs,
  affected cases, preferred repair role, weak verifier reason, and admitted
  cluster targets. This reduces generic `rc:1` attribution in repair packets
  and eval reports. It does not make the verifier pass, add semantic source
  repair, or guarantee that a weak model will apply the selected repair
  correctly.
- Repair prompts and saved repair packets may now render a `Recovery task`
  section derived from deterministic evidence. This clarifies what the next
  bounded repair turn should do, what is out of scope, and which original check
  remains authoritative. It is still not a recovery engine, semantic planner,
  hidden continuation mechanism, or guarantee of final artifact quality.
- Recovery tasks can carry a small execution envelope. The current read-only
  envelope keeps `step_policy:read_only_step_mutation` repair turns read-only
  and accepts only concrete repository read evidence. The setup/config envelope
  keeps `step_policy:setup_step_source_mutation` repair turns constrained to
  package, lockfile, and config paths instead of broad source mutation. This
  improves contract alignment, but it does not make weak plans, poor UI/game
  output, or unrelated verifier failures converge automatically.
- Phase-boundary profile verification is deterministic and read-only. It can
  catch structural contract drift such as Next.js app-root ambiguity, package
  script drift, and Tailwind dependency/config mismatch, but it does not score
  UI quality semantically and does not auto-resume the original ultra plan
  after a standalone repair command. Profile obligations can steer generated
  phase step plans away from known package contract drift and Next.js
  route-integration drift for explicit UI/game artifacts. Active step/repair
  prompts now carry refreshed profile facts, but these are narrow
  deterministic facts, not a full domain workflow. Shared ArtifactGraph
  projection is bounded to observed paths and contract artifacts; it is not a
  persistent workspace model or hidden profile workflow.
- Next.js route integration now uses a bounded static route graph from the
  selected route through relative imports. This improves attribution for
  route/component/hook integration, but it is still not a compiler, runtime
  execution engine, semantic UI checker, or guarantee that the generated game
  is visually good.
- Recovery Orchestration Contract now classifies deterministic failures into
  visible active jobs, admitted targets, prioritized targets, tool-policy
  projections, and repair actions for selected classes such as Next.js route
  integration, missing integration artifacts, manifest dependency repair,
  Tailwind contract repair, tsconfig alias repair, read-only step mutation, and
  future-artifact inspection. This improves the repair packet handoff, but it
  does not add hidden continuation, increase retry budgets, or guarantee that a
  weak model will perform the selected repair correctly.
- Active-job dispatch now records `loop_control_action`, `dispatch_status`,
  `dispatch_reason`, candidate jobs, and tie-break stops. This improves
  attribution when multiple recovery owners are possible, but it is still a
  bounded contract gate. Only safe single-target profile verification failures
  may start one create/edit-constrained minimal-loop repair before rerunning
  the same profile check; it does not retry different owners until one passes.
- Target admission and repair briefs now record proposed, admitted, and
  rejected targets, selected failure cluster, repair brief status, and action
  envelope status before ordinary repair prompt rendering. This reduces
  wrong-target repair, but it still depends on deterministic evidence and does
  not guarantee that a weak model will implement the admitted repair well.
- Legacy-control-stack recovery records now include completion evidence, evidence
  binding, deliverable obligations, recovery owner, repair action plan,
  semantic failure report, repair job state, attempt outcomes, patch
  validation, and eval report fields. Artifact ledger and completion authority
  now distinguish missing deliverables, missing evidence, failed completion
  evidence, failed evidence binding, and stale evidence in runtime/eval
  reports. Bounded pass-side producers now create file-layout and verifier
  evidence from current ledger facts, but richer docs/data/report/profile-wide
  producers are still partial. Bounded repair attempts now record before/after
  signatures and can exhaust target, role, or failure cluster for no-progress
  attribution. Persistent cross-command repair job lifecycle and
  verifier-proven rollback are still partial. Patch validation now has a
  common proposal/report boundary, rejects generated/cache/dependency/protected
  path mutations and test weakening before claiming progress, and eval reports
  patch validation, mechanical adapter, and rollback admission fields.
  Mechanical adapters currently produce bounded diagnostic hints/proposals for
  selected Rust/Python/Node/Next verifier classes; they do not apply edits,
  select targets, or run setup themselves.
- Task Contract projection now records task kind, behavior obligations, and
  artifact role projections from required artifacts and profile obligations.
  These facts reach plan prompts, active step contract facts, plan-lint
  evidence, and eval report fields. The projection is still deterministic and
  bounded; it does not replace profile verification, run setup, or decide
  hidden workflow continuation.
- Step-decomposition lint now rejects the observed high-confidence case where a
  `setup` step owns classified source/style, route, component, test, docs,
  generated, or build artifacts such as `app/globals.css`. Broader ownership
  rules for `verify`, `inspect`, and `report` remain compatibility-sensitive
  because some flows use `expected_paths` as read-only existence gates.
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
  Setup failure evidence recognizes the observed npm `ERESOLVE` peer dependency
  class, but CommandAgent is not a general dependency solver and does not query
  package registries or choose arbitrary latest versions. When manifest repair
  changes declared dependencies, setup recovery can detect stale package-lock
  evidence, record `setup_job_state=stale`, and select bounded `npm install`,
  but only under the existing setup policy.
- Setup lifecycle records now expose setup job kind, manifest kind/path,
  manifest validation status, readiness, command authority, setup result,
  failure signature, and verifier rerun result. Rust and Python verifier
  commands can surface manifest validation blockers as setup/manifest evidence,
  but automatic Rust/Python dependency installation remains unsupported.
- Runtime job reports now project lifecycle stage, active owner, selected
  action, target admission, repair action plan status, completion source,
  attempt outcome, evidence runner status, verifier rerun result, and explicit
  stop reason into eval artifacts and reports. This makes stop-point diagnosis
  clearer and distinguishes dry-run placeholder success, runtime success,
  existing success, evidence-only success, and recheck results. It remains
  observability only: it does not add hidden continuation, rerun verifiers, or
  improve generated app quality by itself.
- Profiles now render a common output schema for root hints, classified
  artifacts, setup/scaffold/integration artifacts, verifier commands,
  behavior obligations, verification failures, completion evidence
  requirements, failure mappings, adapter families, capability status, and
  recovery candidate hints. These facts improve attribution and make
  cross-profile parity gaps visible, but profiles still do not select final
  active jobs, execute setup, or materialize scaffold files by themselves.
- Requested-port Next.js launchability is now checked separately from
  `npm run build` when a generated plan has used the build verifier. The
  `dev_server_smoke` job validates `scripts.dev`, port availability, endpoint
  response, and cleanup. It is intentionally a bounded local smoke check, not a
  full browser or visual-quality evaluation, and it stops as `port_in_use` when
  the requested port is already occupied.
- Next.js Tailwind plan correction now treats omitted package literals such as
  `tailwindcss`, `postcss`, and `autoprefixer` as a manifest repair job. When a
  single package step is the deterministic target, CommandAgent can
  materialize the exact manifest obligation into that plan step before rerunning
  lint. Ambiguous target plans or repeated unchanged missing-literal sets still
  stop with explicit attempt-ledger evidence rather than weakening the Tailwind
  contract, increasing retry count, or adding provider-specific Gemini policy.
- `.env` loading is not implemented inside CommandAgent. Export provider API
  keys in the shell or use an external env loader.
- The eval runner has dry-run wiring, real binary execution paths, per-case
  `/plan-run` / `/ultra-plan-run` mode, and fixture seeding for modification
  cases. New eval runs also record terminal observation fields such as
  `terminal_state`, `failure_class`, `diagnostic_code`,
  `evidence_runner_status`, `artifact_ledger_status`, workspace scope,
  artifact ownership/source-of-truth, read/changed/verifier-mentioned/setup
  paths, rejected target reasons, and `port`; reports
  backfill conservative values for older run roots and prefer contract
  evidence diagnostic codes over raw process-code reasons when present. This
  improves attribution, including `port_in_use` for occupied dev-server ports,
  artifact evidence failures, and classified verifier failures. Runtime job
  fields also report setup, dev-server smoke state, selected failure clusters,
  preferred repair roles, weak verifier reasons, and admitted cluster targets,
  but eval remains observation-only and does not select repair
  actions. Large semantic checks are still intentionally explicit and
  conservative; the latest fresh large run is 0/6 and needs triage before it
  can be treated as a release-quality gate.
- Provider API support is MVP-level. Ollama, Gemini, and OpenAI share one thin
  chat contract. Ollama uses native tool calls, Gemini uses native function
  calling with XML fallback retained as a compatibility/downgrade path, and
  OpenAI still uses the shared XML fallback tool-call contract. Gemini native
  support covers request declarations, response `functionCall` parsing, and
  `functionResponse` history, but it does not add provider-specific repair
  policy or guarantee app-quality convergence.
- Smaller planner models may still fail plan schema or plan-lint requirements
  even after bounded correction. CommandAgent accepts ordinary block scalar
  strings for known long text fields, including `|`, `|-`, `|+`, `>`, `>-`,
  and `>+`, but it still rejects unsupported YAML features and invalid plan
  contracts. Use a stronger planner model for MVP workflows until frontier
  data is collected.
