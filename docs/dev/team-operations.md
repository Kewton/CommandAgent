# Team operating notes

Rules, pitfalls and standing user decisions for the PM, the development leader,
the reviewer, the judge and workers. They add to
[`parallel-dev-harness.md`](parallel-dev-harness.md); where this file records a
later user decision, this file wins until the harness document is updated.

These notes were moved from the PM's Claude Code auto memory on 2026-11-18.
From then on, work state, next steps, questions for the user and new user
decisions go to CommandMate `mem` (see "Memory and handoff" in `AGENTS.md`).
Lessons and procedures that every session must follow belong here.

## 1. Standing user decisions that update the harness

- **Who answers open prompts** (2026-11-21, 「毎回私が選択する訳にはいかないので
  ルールを見直して下さい」). The user no longer answers every prompt. The leader
  answers a worker's prompt inside the Issue's scope and any refusal or
  cancel; the PM answers any team session's prompt with a refusal or cancel,
  or with an in-brief yes that has no outward effect; the user keeps real
  provider sends and spending, push, PR creation and merge, deletes outside
  the worktree or a scratch root, `.env` and keys, CommandMate start and stop,
  Issue edits, and gate changes. First use: the PM answered "No" to a
  musubi-ca-pack #6 worker probe that would have posted to api.openai.com.
  See `CLAUDE.md` (exit-10 rule) and harness section 4.
- **Team reorganized to the CommandMate team-setup layout** (2026-11-21,
  「ガイドの標準構成（claude-2 を PM にするなど）に合わせ」「推奨で良い」). The PM is
  `claude-2`, the development leader `claude-3`, the reviewer `codex`, the
  judge `claude-4`, and workers are Command Code (`worker` role is the
  repository default for Command Code). The helpers (`codex-3` / Codex_Sub,
  `claude-4` / Claude 4) are retired: investigation goes to the reviewer
  `codex`. The PR-creation and merge delegations now belong to `claude-2`.
  Entries below that name the old instances are marked as replaced.
- **Workers are Claude Code sessions for now** (2026-10-17, 「しばらくはclaudeを
  使用して下さい」; replaced on 2026-11-21: workers are Command Code again, see
  the first entry). Dispatch them with auto-yes and the harness stop pattern.
  Going back to Command Code is the user's call. If a worker's quality is low
  (repeated verification failures, scope or gate violations, a false report, a
  stall), the user allowed Codex `gpt-6.2-sol` with reasoning high as the
  worker (2026-10-16; the model was replaced on 2026-11-18, 「Codexのモデルを
  gpt6.2-sol highに変えてください」, and was Codex `gpt-6.1-Sol` medium before).
  Antigravity is never used (harness section 1).
- **Claude Code sessions run Opus 5.5 with effort medium** (2026-11-18,
  「なるべく opus5.5 medium として」). This covers every Claude Code
  session in the team: the PM (`claude-2`), the development leader
  (`claude-3`) and the judge (`claude-4`).
- **Codex runs `gpt-6.2-sol` with reasoning high** (2026-11-18, 「Codexのモデルを
  gpt6.2-sol highに変えてください」). This replaces Codex `gpt-6.1-Sol` medium
  everywhere it was named, including the worker fallback above.
- **The PM closes Issues after their merge** (2026-10-30, 「あなたでもクローズ
  できると思います」). Close only after a merge under harness section 3, step 6,
  or when the user asks, with a comment naming the PR and merge commit, and
  record it in the ledger. Other Issue edits still need the user.
- **The PM starts and stops dev-team instances** (2026-11-11, 「開発体制の各
  セッションの停止と起動を許可します」). Record each in the ledger. Starting or
  stopping the CommandMate server stays with the user.
- **Reviewers** (2026-10-30, 「Issueのレビューはcodex-3で、Musubi の factory は
  codexにレビュー依頼して」; since 2026-11-21 the reviewer is `codex` and the
  `claude-2` fallback below is withdrawn because `claude-2` is the PM: when
  `codex` stops midway, the PM tells the user). Issue and PR reviews go to `codex-3`. If `codex-3`
  stops midway (a hang, or the OpenAI filter "This content can't be shown"),
  move that task to `claude-2` without asking, then tell the user and record
  it (2026-10-04).
- **Red team with `claude-2`** (2026-10-31, 「Claude_Sub（fable 5.1）への依頼を
  上手く活用してください」; replaced on 2026-11-21: `claude-2` is the PM, so this
  practice is retired until the user assigns another session). For a security-sensitive guard, have `claude-2`
  enumerate bypasses by family into cases the worker turns into tests before
  the next `codex-3` recheck, instead of looping fix → review.
- **Codex weekly limit** (2026-11-02, 「リセット可能なので、気にしなくて良いです。
  5%を切ったら教えてください。」). Do not narrow Codex briefs to save quota; tell
  the user when a Codex pane shows less than 5% left.
- **Review budget** (2026-09-29). An independent pre-merge review is only for
  secret- or security-related Issues, at most one full review per Issue, scoped
  and time-boxed. After rework, check only the fixed points. Fixes that touch
  only tests skip the reviewer check; the PM reads the diff to confirm no
  assertion was removed.
- **Reporting** (2026-10-09, 「私の判断が必要になったら教えてください。」). Do not
  answer relayed prompt notices one by one. Speak to the user only for a
  decision, a prompt stuck for minutes, a failure that needs direction, or an
  anomaly; handle routine notifications silently and record them in the
  ledger. Poll about every 5 minutes with one combined condition per stage.

## 2. Leader and PM procedures

- When a fix's scope includes something the PM decides (how to treat a review
  blocker or a flaky gate), the leader sends it to the worker only after the
  PM's approval, not together with the report (#974, 2026-11-03).
- The leader raises a stopped or stalled worker to the PM within 15 minutes.
  Wait with `commandmate wait <worktree>` (returns when the worker goes idle),
  not with a loop on `git rev-parse HEAD`; a stop without a commit never moves
  HEAD (#972, 2026-11-02).
- A Claude Code session cannot run `/compact` on itself. At Issue boundaries
  the PM sends `/compact` to `claude-3` (and `claude-4`) and checks the pane
  for "Compacted". Watchers also match "maximum context length" and
  "Type continue". Make every brief self-contained.
- When the PM session's auto-mode classifier denies an action (merge, starting
  a session with auto-yes, permission edits), do not rephrase and retry. Offer
  the user `! <command>`; the user runs the PM in accept-edits mode.
- Start the leader on Opus 5.5 with effort medium (section 1) on the send
  that starts the session, and confirm both in the pane header.
- Pitfalls from the 2026-11-18 and 2026-11-19 runs (leader and PM operating
  errors), each with the step that prevents it:
  - A brief's verify rule named a gate id that does not exist, and
    `commandmate verify` exited 99 without reaching a verdict. Copy gate ids
    from `.commandmate/verify.yaml`, never from memory. Exit 99 is not a pass:
    count it as no verdict and rerun with valid ids.
  - A wait loop passed wrong arguments to `capture`, matched nothing, and the
    run sat idle for about 2.5 hours. Wait on both an anchored regular
    expression (for example `^DONE:` / `^STOP:`) and a `STALL` condition, so a
    loop that never matches still reports.
  - A PM brief claimed that mode `0o700` does not hit R3 of the
    isolation ratchet. It does: R3 matches `0o[0-7]?[157][0-7][0-7]`
    (`docs/dev/dev-guardrails.md`). When a test changes permissions, save the
    original mode and restore it, and check a mode against the R3 pattern
    instead of asserting it in a brief.
  - A monitor started with `&` never delivered its notification. Start
    monitors with the harness's background execution (for example the Bash
    tool's `run_in_background`), not with a shell `&`.
- Pitfalls from the 2026-11-26 musubi-ca-pack run (leader and worker
  operating errors), each with the step that prevents it:
  - A worker read measurement records, team-setup reports, briefs and
    reviews it had been told not to read, in every dispatch of one Issue. Fix
    requests had gone as nudges while the task contract still held the old
    goal, so the worker looked for the request on its own. Write every fix
    into the Issue fixture and re-plan instead of nudging. The task contract
    takes only the fixture's first line (the objective), its completion
    conditions and its target files, so put the fix in the first line and
    the bans in the completion conditions, and confirm that they reached the
    contract before dispatching. Word the bans without the literal strings
    of the stop pattern (for example, write "no deletion with `rm`" rather
    than the command itself): the goal is shown on the worker's screen, the
    stop pattern matches it, and auto-yes disarms itself. Put the read bans
    in bold in every worker instruction: measurement records, team-setup
    files, the base checkout's `.commandmate/`, other sessions' conversation
    records, and evaluation material. Limit searches to the worker's own
    worktree. Start a new worker session after a deviation. When a worker
    reads a banned path, stop the work and report to the PM.
  - A leader's watcher answered permission prompts without reading the
    screen (four times), and twice typed `1` into the input box. Read the
    pane with `capture --pane`, act only on `prompt_detected`, and match only
    the command part of the prompt. When the command cannot be read, do not
    answer: raise it instead.
  - A dispatch reached the worktree's default `claude` instance instead of
    `command-code`. Before every dispatch or fix, confirm that the target
    instance is `command-code` and that its auto-yes is on.

## 3. Machine resources

- Put bulky output (cargo target dirs, audit worktrees, mutation output, raw
  logs, E2E runs) under `/Volumes/SSD_NX/tmp/<topic>-<YYYYMMDD>-NN/`, not in
  the repository; small reports and ledgers may stay in `workspace/tmp/<MMDD>/`.
  A 2026-09-27 audit left 179 GB in `workspace/temp/`.
- Heavy work runs one at a time machine-wide, enforced by
  `/Volumes/SSD_NX/tmp/heavy-lock/with-heavy-lock` (exclusive class for
  live-LLM and evaluation) and `with-cargo-slot` (two cargo slots), not by brief
  wording. The process tree is stopped above 12 GB (exit 137). The README in
  that directory is the reference.
  - The PM installs each worktree's `.cargo/config.toml` with
    `/Volumes/SSD_NX/tmp/heavy-lock/install-worktree-config <worktree>` before
    dispatch; each worktree gets its own target dir under
    `/Volumes/SSD_NX/tmp/cargo-target/`, deleted at cleanup.
  - Pitfall (2026-11-18/19): the worktree path reached the setup script
    without being split from the other arguments, and the script wrote
    `~/.cargo/config.toml` instead of the worktree's file. After creating a
    worktree and installing its config, confirm that `~/.cargo/config.toml`
    does not exist and that `<worktree>/.cargo/config.toml` does.
  - Never share one target dir across checkouts (same unit hashes let cargo
    reuse another checkout's binary). Never set `CARGO_TARGET_DIR` or another
    runner in briefs. At most 3 concurrent workers.
  - `cargo mutants` always with an explicit `--timeout` (about 2x baseline) and
    `--jobs 1`; a memory-stopped mutant counts as a timeout, not as caught.
    Harness section 3a covers isolation.
- When testing a mutant in a copy of the tree, run cargo from inside the copy,
  never with `--manifest-path` from the worktree (#976, 2026-11-06).
- Docker: remove each check image after its run and prune the build cache
  (`docker builder prune`); start Linux toolchain checks from the deps-first
  `ci199.sh` of L-133. Check `docker system df` and `df -h /` before long
  campaigns. In 2026-10 leftover images and cache filled the internal disk and
  made containerd read-only.
- Shell discipline for delegated sessions: one command per Bash call (no `;`,
  `&&`, `||`, `|`), and write files only with the editor tools, never with `>`
  or `>>`.

## 4. Engineering and product principles

- **Product direction** (2026-11-02): CommandAgent is a host for building
  services with 「安いモデルで、用途を絞り、検査済みの成果物を、費用を記録しながら
  作る」. Prefer work on per-run cost visibility and caps, easy external packs,
  host-side verification and completion judgement, and a small entry surface.
- **No overfitting** (2026-10-29, 「generalでの利用とし、過学習にならないように
  してください」): no fixes keyed to benchmark wording, file names or hidden
  tests; judge on held-out tasks; one fixed entry for every task; never weaken
  gates. Domain specifics stay in packs.
- **Working first, refine later** (2026-11-07, 「まずは動くものを作ることを優先し、
  使いながら洗練させていきましょう」): size verification to the stakes, keep the
  hard limits the user set, record non-critical findings instead of blocking.
- **Issue reviews**: verify every claim against the code with `file:line`
  evidence, analyze the root cause, sweep for the same class, update the Issue
  body itself (a dated review section; fix wrong lines in place) in Japanese,
  and file separately scoped bugs as new cross-linked Issues.
- **Demo assets are real recordings** (2026-08-20): never hand-drawn. Regenerate
  with `scripts/demo/` in a throwaway workspace with an isolated `--state-dir`;
  keep `tests/doc_drift.rs` in sync.

## 5. Generic-bench measurement conventions (2026-10, inactive)

Kept for reference if that work resumes; the current focus is the external
pack use.

- Prices per 1M tokens given by the user (2026-10-30): gemma4:31b-cloud on
  Ollama Pro 0.14 / cached 0.05 / output 0.40 USD; gpt-6-luna 0.10 / 0.01 /
  0.50 (reasoning billed as output); glm-5.3-flash:cloud 0.15 / 0.03 / 0.50.
- Every bench summary has a per-arm cost column. Tokens come from the
  `time_profile` event only. Recompute the OpenAI reference arm from `usage`
  at current prices, not from the recorded `cost_usd`. The cost goal compares
  face-value credits to 1.5x the reference arm, without dividing by 3.
- TypeSafe Jev may classify request text (never code or file contents); the
  key is `JEV_API_KEY` in `.env` and is never printed (2026-10-30).
