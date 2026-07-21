Recover this failed run by producing and executing a focused ultra plan.

Original goal:
あなたが考える最高に面白くかっこいいブロック崩しゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。

Profile: nextjs

Failure scope:
- phase: build-verification
- step: unknown
- kind: final_acceptance_repair_failed

Failure evidence:
- keyboard or pointer input must visibly change game state (player position, projectiles, score/health, or state transitions); wire input handlers into the render/update loop.
- render-loop checklist: ref attached -> effect runs -> rAF loop starts -> draw calls
- input-wiring checklist: keyboard or pointer input must visibly change game state (player position, projectiles, score/health, or state transitions); wire input handlers into the render/update loop.
- input operations must visibly change actual application state, and that change must be reflected in the data-anvil-state JSON snapshot; wire input handlers to state updates.
- stateful_update_evidence: update visible state over time or directly from user input, and expose the updated snapshot in data-anvil-state
- restart_or_recoverable_state_evidence: add data-anvil-action="restart" to every restart/retry/new-game affordance (game-over, victory, and in-play when present) and ensure it resets observable state; the initial primary action alone is not restart evidence
- model_stagnation:read_only_loop: write_required exhausted for src/app/page.tsx; objective: Repair the final acceptance failure for the current ultra run. Original ultra goal: あなたが考える最高に面白くかっこいいブロック崩しゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。 Profile: nextjs Intent: create Final acceptance failure: - primary reason: missing_required_evidence:stateful_update_evidence - repair target: implementation - attempt: 1/2 Pending capability evidence remedies: - stateful_update_evidence: update visible state P

Missing paths:
- none

Missing capabilities:
- stateful_update_evidence
- restart_or_recoverable_state_evidence

Verification commands:
- none

Changed paths:
- none

Repair targets:
- input_state_render_wiring

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
