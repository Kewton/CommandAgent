export class GuiRequestError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    readonly serverMessage: string,
  ) {
    super(serverMessage);
    this.name = "GuiRequestError";
  }
}

type ErrorPayload = {
  code?: unknown;
  error?: unknown;
};

export async function responseError(response: Response): Promise<GuiRequestError> {
  const text = await response.text();
  let payload: ErrorPayload = {};
  try {
    payload = JSON.parse(text) as ErrorPayload;
  } catch {
    // Non-JSON proxy responses still receive status-based recovery guidance.
  }
  const code = typeof payload.code === "string" ? payload.code : `http_${response.status}`;
  const serverMessage =
    typeof payload.error === "string" && payload.error.trim() !== ""
      ? payload.error
      : text.trim() || response.statusText || "empty error response";
  return new GuiRequestError(response.status, code, serverMessage);
}

export function describeError(reason: unknown): string {
  if (!(reason instanceof GuiRequestError)) {
    if (reason instanceof TypeError) {
      return "GUI サーバーに接続できません。ページを再読み込みし、プロキシとサーバーの稼働を確認してから再接続してください。";
    }
    const detail = reason instanceof Error ? reason.message : "unknown client error";
    return withDetail(
      "要求を完了できませんでした。ページを再読み込みして再試行し、続く場合は GUI サーバーのログを確認してください。",
      detail,
    );
  }

  const detail = reason.serverMessage;
  switch (reason.code) {
    case "trial_token_invalid":
      return withDetail(
        "Trial トークンが無効です。ページを再読み込みして再認証し、実行時に発行されたトークンを入力してください。",
        detail,
      );
    case "trial_origin_not_allowed":
      return withDetail(
        "この Origin から Trial を実行できません。GUI_TRIAL_ALLOWED_ORIGINS に現在の Origin を追加して GUI サーバーを再起動してください。",
        detail,
      );
    case "trial_workspace_running": {
      const sessionId = reconnectSessionId(reason);
      if (sessionId !== null) {
        return withDetail(
          `Trial ワークスペースはセッション ${sessionId} が使用中です。下の「再接続」リンクから既存セッションの監視へ戻ってください。`,
          detail,
        );
      }
      return withDetail(
        "Trial ワークスペースは別のセッションが使用中です。セッション一覧を更新し、実行中セッションへ再接続してください。",
        detail,
      );
    }
    case "trial_workspace_recovery_required":
      return withDetail(
        "Trial ワークスペースの復旧が必要です。表示されたセッションのイベントと成果物を確認し、既存 CLI の復旧手順を完了してから再接続してください。",
        detail,
      );
    case "trial_workspace_conflict":
      return withDetail(
        "Trial ワークスペースを使用できません。実行ルートが起動時と同じ場所にあるか、ディレクトリ権限と分離境界を確認してください。",
        detail,
      );
    case "trial_session_conflict":
      return withDetail(
        "現在のセッション状態ではこの操作を実行できません。最新状態を再取得し、Gateまたは保留中の指示を確認してください。",
        detail,
      );
    case "trial_confirmation_stale":
      return withDetail(
        "Gate 1 の内容が変わりました。「契約と価格を確認」をやり直し、現在のカードを確認してから起動してください。",
        detail,
      );
    case "trial_confirmation_required":
      return withDetail(
        "Gate 1 の確認が必要です。契約と価格を確認し、確認チェックを選択してから起動してください。",
        detail,
      );
    case "trial_execution_disabled":
    case "trial_authentication_disabled":
      return withDetail(
        "Trial 実行が無効です。GUI サーバーを --execution-root と GUI_TRIAL_TOKEN 付きで再起動してください。",
        detail,
      );
    case "trial_internal_error":
      return withDetail(
        "CLI を起動または監視できませんでした。GUI サーバーの --commandagent-bin が実在する実行可能ファイルを指すか確認し、既存セッションがあれば再接続してください。",
        detail,
      );
    case "trial_request_invalid":
      return withDetail(
        "入力を受け付けられませんでした。表示中の入力条件を確認し、Gate 1 を再確認してから再試行してください。",
        detail,
      );
    case "trial_events_too_large":
    case "resource_too_large":
      return withDetail(
        "読み取り対象が GUI の上限を超えています。イベント末尾の行数または対象ファイルを小さくして再試行してください。",
        detail,
      );
    case "trial_session_not_found":
    case "trial_session_file_not_found":
      return withDetail(
        "セッションを見つけられません。セッション ID と実行ルートを確認してから再接続してください。",
        detail,
      );
    case "resource_not_found":
      return withDetail(
        "記録を見つけられません。選択した実行やファイルを確認し、一覧を再読み込みしてください。",
        detail,
      );
    case "repository_read_failed":
    case "trial_session_file_read_failed":
      return withDetail(
        "リポジトリ記録を読み込めません。ページを再読み込みし、GUI サーバーの --repository-root とファイル権限を確認してください。",
        detail,
      );
    default:
      if ([401, 403, 407].includes(reason.status)) {
        return withDetail(
          "上流プロキシまたはアクセス認証が必要です。ページを再読み込みして再認証し、Trial トークンを入力し直してください。",
          detail,
        );
      }
      if ([502, 504].includes(reason.status)) {
        return withDetail(
          "GUI サーバーへ到達できません。プロキシ経路と GUI サーバーの稼働を確認してから再接続してください。",
          detail,
        );
      }
      return withDetail(
        `要求に失敗しました（HTTP ${reason.status} / ${reason.code}）。ページを再読み込みして再試行し、続く場合は GUI サーバーのログを確認してください。`,
        detail,
      );
  }
}

export function reconnectSessionId(reason: unknown): string | null {
  if (
    !(reason instanceof GuiRequestError) ||
    !["trial_workspace_running", "trial_workspace_conflict"].includes(reason.code)
  ) {
    return null;
  }
  return (
    reason.serverMessage.match(
      /(?:already running session|non-terminal session) ([0-9a-f]{8}-[0-9a-f-]{27})/i,
    )?.[1] ?? null
  );
}

function withDetail(guidance: string, detail: string): string {
  return `${guidance} 詳細: ${detail}`;
}
