Recover this failed run by producing and executing a focused ultra plan.

Original goal:
友だちとの旅行のお金をあとで揉めないように割り勘できる小さなアプリを作って。誰が何を払ったかはざっくり入力できて、最後に誰が誰へいくら渡せばいいか見たい。

Profile: community-mini-app

Failure scope:
- phase: integration-and-verification
- step: unknown
- kind: final_acceptance_repair_exhausted

Failure evidence:
- community_schema_missing; final acceptance repair stopped: final_acceptance_repair_no_source_change

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- implementation

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
