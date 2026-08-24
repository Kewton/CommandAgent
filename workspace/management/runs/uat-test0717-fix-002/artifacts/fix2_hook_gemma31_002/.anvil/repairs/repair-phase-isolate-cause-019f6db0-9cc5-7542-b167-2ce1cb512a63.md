Recover this failed run by producing and executing a focused ultra plan.

Original goal:
このNext.jsアプリはリスタート操作の契約フック（data-anvil-action="restart"）が欠落しており検証に失敗します。原因を特定して修正してください。既存の検証が通ることを確認してください。

Profile: nextjs

Failure scope:
- phase: isolate-cause
- step: unknown
- kind: profile_invariant_failure

Failure evidence:
- missing relative imports: src/app/SpaceInvadersGame.tsx imports {useSpaceInvadersGame} from ./game-engine but src/app/game-engine.ts does not export useSpaceInvadersGame - export useSpaceInvadersGame or correct the import
- Missing relative imports: - src/app/SpaceInvadersGame.tsx imports {useSpaceInvadersGame} from ./game-engine but src/app/game-engine.ts does not export useSpaceInvadersGame - export useSpaceInvadersGame or correct the import

Missing paths:
- src/app/SpaceInvadersGame.tsx imports {useSpaceInvadersGame} from ./game-engine but src/app/game-engine.ts does not export useSpaceInvadersGame - export useSpaceInvadersGame or correct the import

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- profile_contract

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
