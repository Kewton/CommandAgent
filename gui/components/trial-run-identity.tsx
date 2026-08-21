import type { ConfirmationIdentity } from "../lib/types";

type TrialRunIdentityProps = {
  identity: ConfirmationIdentity;
};

export function TrialRunIdentity({ identity }: TrialRunIdentityProps) {
  const pack = identity.pack.selection === "pinned"
    ? `${identity.pack.id}@${identity.pack.version}`
    : "選択なし";

  return (
    <dl
      aria-label="この実行の確定内容"
      className="trial-run-identity"
      data-testid="trial-run-identity"
    >
      <div className="trial-run-identity-goal">
        <dt>目標</dt>
        <dd data-testid="trial-run-identity-goal">{identity.request}</dd>
      </div>
      <div>
        <dt>profile</dt>
        <dd><code data-testid="trial-run-identity-profile">{identity.profile}</code></dd>
      </div>
      <div>
        <dt>pack</dt>
        <dd><code data-testid="trial-run-identity-pack">{pack}</code></dd>
      </div>
      <div>
        <dt>実行モデル</dt>
        <dd>
          <code data-testid="trial-run-identity-executor-model">
            {identity.pins.executor_provider} / {identity.pins.executor_model}
          </code>
        </dd>
      </div>
      <div>
        <dt>計画モデル</dt>
        <dd>
          <code data-testid="trial-run-identity-planner-model">
            {identity.pins.planner_provider} / {identity.pins.planner_model}
          </code>
        </dd>
      </div>
    </dl>
  );
}
