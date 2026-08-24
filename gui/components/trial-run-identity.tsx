import type { ConfirmationIdentity } from "../lib/types";

type TrialRunIdentityProps = {
  identity: ConfirmationIdentity | undefined;
};

export function TrialRunIdentity({ identity }: TrialRunIdentityProps) {
  if (identity === undefined) {
    return (
      <div className="trial-identity-warning" data-testid="trial-identity-unavailable" role="status">
        <strong>この実行の確定内容を表示できません</strong>
        <p>
          古い gui_server 応答には identity がありません。画面はそのまま利用できます。
          同じ checkout で静的 GUI と gui_server を再ビルドし、再起動してください。
        </p>
      </div>
    );
  }
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
        <dt>プロファイル</dt>
        <dd><code data-testid="trial-run-identity-profile">{identity.profile}</code></dd>
      </div>
      <div>
        <dt>実行目的</dt>
        <dd><code data-testid="trial-run-identity-intent">{identity.intent}</code></dd>
      </div>
      <div>
        <dt>パック</dt>
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
      {identity.pins.think !== undefined && (
        <div>
          <dt>Ollama thinking</dt>
          <dd><code data-testid="trial-run-identity-think">{identity.pins.think}</code></dd>
        </div>
      )}
    </dl>
  );
}
