# Team operating notes

Rules, pitfalls and standing user decisions for the PM, the development leader,
helpers and workers. They add to
[`parallel-dev-harness.md`](parallel-dev-harness.md); where this file records a
later user decision, this file wins until the harness document is updated.

These notes were moved from the PM's Claude Code auto memory on 2026-11-18.
From then on, work state, next steps, questions for the user and new user
decisions go to CommandMate `mem` (see "Memory and handoff" in `AGENTS.md`).
Lessons and procedures that every session must follow belong here.

## 1. Standing user decisions that update the harness

- **Workers are Claude Code sessions for now** (2026-10-17, 「しばらくはclaudeを
  使用して下さい」). Dispatch them with auto-yes and the harness stop pattern.
  Going back to Command Code is the user's call. If a worker's quality is low
  (repeated verification failures, scope or gate violations, a false report, a
  stall), the user allowed Codex `gpt-6.1-Sol` medium as the worker
  (2026-10-16). Antigravity is never used (harness section 1).
- **The PM closes Issues after their merge** (2026-10-30, 「あなたでもクローズ
  できると思います」). Close only after a merge under harness section 3, step 6,
  or when the user asks, with a comment naming the PR and merge commit, and
  record it in the ledger. Other Issue edits still need the user.
- **The PM starts and stops dev-team instances** (2026-11-11, 「開発体制の各
  セッションの停止と起動を許可します」). Record each in the ledger. Starting or
  stopping the CommandMate server stays with the user.
- **Reviewers** (2026-10-30, 「Issueのレビューはcodex-3で、Musubi の factory は
  codexにレビュー依頼して」). Issue and PR reviews go to `codex-3`. If `codex-3`
  stops midway (a hang, or the OpenAI filter "This content can't be shown"),
  move that task to `claude-2` without asking, then tell the user and record
  it (2026-10-04).
- **Red team with `claude-2`** (2026-10-31, 「Claude_Sub（fable 5.1）への依頼を
  上手く活用してください」). For a security-sensitive guard, have `claude-2`
  enumerate bypasses by family into cases the worker turns into tests before
  the next `codex-3` recheck, instead of looping fix → review.
- **Codex weekly limit** (2026-11-02, 「リセット可能なので、気にしなくて良いです。
  5%を切ったら教えてください。」). Do not narrow Codex briefs to save quota; tell
  the user when a Codex pane shows less than 5% left.
- **Review budget** (2026-09-29). An independent pre-merge review is only for
  secret- or security-related Issues, at most one full review per Issue, scoped
  and time-boxed. After rework, check only the fixed points. Fixes that touch
  only tests skip the helper check; the PM reads the diff to confirm no
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
  the PM sends `/compact` to `claude-3` (and `claude-2`) and checks the pane
  for "Compacted". Watchers also match "maximum context length" and
  "Type continue". Make every brief self-contained.
- When the PM session's auto-mode classifier denies an action (merge, starting
  a helper with auto-yes, permission edits), do not rephrase and retry. Offer
  the user `! <command>`; the user runs the PM in accept-edits mode.
- Start the leader with `--model sonnet` on the send that starts the session
  and confirm the pane header shows the model.

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
