import type { TrialRunState } from "../hooks/use-trial-run";

type TrialAccessPanelProps = {
  purpose: "status" | "history" | "detail";
  run: TrialRunState;
};

const purposeLabels = {
  status: "実行状況",
  history: "実行履歴",
  detail: "結果詳細",
} as const;

export function TrialAccessPanel({ purpose, run }: TrialAccessPanelProps) {
  const {
    busy, error, reconnectExisting, reconnectSessionId, trialAccessReady, trialToken,
    trialTokenAuthEnabled, updateTrialToken,
  } = run;
  const needsSession = purpose !== "history";

  return (
    <section className="panel trial-access-panel" data-testid="trial-access-panel">
      <div>
        <span className="panel-index">読み取り接続 / {purposeLabels[purpose]}</span>
        <h2>トライアルセッションへ接続</h2>
        {needsSession && (
          <code data-testid="trial-route-session">
            {reconnectSessionId.trim() === "" ? "session パラメーター未指定" : reconnectSessionId}
          </code>
        )}
      </div>
      {trialTokenAuthEnabled ? (
        <label htmlFor="trial-token">
          トライアルアクセストークン
          <input
            autoCapitalize="none"
            autoComplete="off"
            data-testid="trial-token"
            id="trial-token"
            onChange={(event) => updateTrialToken(event.target.value)}
            spellCheck={false}
            type="password"
            value={trialToken}
          />
        </label>
      ) : (
        <p className="source-note" data-testid="trial-token-auth-disabled">
          トライアルトークン認証はサーバー設定で無効です。
        </p>
      )}
      {needsSession && (
        <button
          className="secondary-action"
          data-testid="reconnect-session-button"
          disabled={busy || reconnectSessionId.trim() === "" || !trialAccessReady}
          onClick={() => void reconnectExisting()}
          type="button"
        >
          読み取り専用で再接続
        </button>
      )}
      {error !== null && <p className="trial-error" role="alert">{error}</p>}
      <small>アクセストークンはこの base path の sessionStorage だけに保持します。</small>
    </section>
  );
}
