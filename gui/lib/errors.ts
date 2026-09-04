export class GuiRequestError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    readonly serverMessage: string,
    readonly report: unknown = null,
    readonly sessionId: string | null = null,
  ) {
    super(serverMessage);
    this.name = "GuiRequestError";
  }
}

type ErrorPayload = {
  code?: unknown;
  error?: unknown;
  report?: unknown;
  session_id?: unknown;
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
  const sessionId = typeof payload.session_id === "string" ? payload.session_id : null;
  return new GuiRequestError(
    response.status,
    code,
    serverMessage,
    payload.report ?? null,
    sessionId,
  );
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
    case "recovery_run_confirmation_required":
      return withDetail(
        "Recovery Run の確認が必要です。表示中の path、hash、フェーズ、許可ポリシー、自動実行予算を確認し、チェックを選択してください。",
        detail,
      );
    case "recovery_run_stale":
      return withDetail(
        "Recovery Run の確認内容が古くなりました。現在の Gate 4 から Recovery Plan の確認カードを作り直してください。",
        detail,
      );
    case "recovery_run_drift":
      return withDetail(
        "確認後に Recovery Plan が変化したため実行を拒否しました。plan とイベントを確認し、現在の内容で確認カードを作り直してください。",
        detail,
      );
    case "recovery_treatment_rejected":
      return withDetail(
        "自動 Recovery の treatment が拒否されているため、この plan は GUI から再実行できません。拒否理由と保持された control を確認してください。",
        detail,
      );
    case "recovery_treatment_pending":
      return withDetail(
        "自動 Recovery の treatment 判定が未解決のため実行できません。promotion または control 保持の記録を確認してください。",
        detail,
      );
    case "recovery_run_invalid":
      return withDetail(
        "現在の Gate 4 には安全に実行できる Recovery Plan がありません。記録済み path と plan の構文を確認してください。",
        detail,
      );
    case "trial_execution_disabled":
    case "trial_authentication_disabled":
      return withDetail(
        "Trial 実行が無効です。GUI サーバーを --execution-root 付きで再起動してください。トークン認証を on にする場合は GUI_TRIAL_TOKEN も設定してください。",
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
    case "trial_intent_ambiguous":
      return withDetail(
        "実行目的を自動判定できませんでした。新しいアプリを開発する場合は「実行目的」で「作成」を選択し、再試行してください。既存アプリを修正する場合は「修正」を選択し、再試行してください。",
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
    case "extensions_disabled":
      return withDetail(
        "拡張供給が無効です。GUI サーバーを所有者専用の --extension-root 付きで再起動してください。",
        detail,
      );
    case "extension_invalid_request":
      return withDetail(
        "拡張 pack の入力を受け付けられませんでした。ID、version、ファイル名、サイズ、JSON を確認してください。",
        detail,
      );
    case "extension_conflict":
      return withDetail(
        "この pack は固定済みまたは退役済みのため変更できません。新しい version を staged にしてください。",
        detail,
      );
    case "extension_verification_failed":
      return withDetail(
        "拡張 pack の検証に失敗しました。検証レポートに従って staged ファイルを修正し、再検証してください。",
        detail,
      );
    case "extension_supply_failed":
      return withDetail(
        "拡張ルートの処理に失敗しました。所有権、0700 権限、空き容量、journal の状態を確認してください。",
        detail,
      );
    case "profile_auth_failed":
      return withDetail(
        "プロファイル供給の認証に失敗しました。現在の GUI Trial トークンを入力し直してください。",
        detail,
      );
    case "profile_origin_not_allowed":
      return withDetail(
        "この Origin からプロファイルを供給できません。GUI_TRIAL_ALLOWED_ORIGINS と現在の URL を確認してください。",
        detail,
      );
    case "profile_invalid_request":
      return withDetail(
        "プロファイル要求を解釈できません。相対 path、文書本文、確認済み hash を確認してください。",
        detail,
      );
    case "profile_body_too_large":
      return withDetail(
        "プロファイル文書が上限を超えています。manifest / overlay を 256 KiB 以下にしてください。",
        detail,
      );
    case "profile_validation_failed":
      return withDetail(
        "プロファイルを検証できません。compact manifest v2、additive overlay、closed capability、path を確認してください。",
        detail,
      );
    case "profile_confirmation_stale":
      return withDetail(
        "確認後に path または本文が変わりました。preview をやり直して exact hash を再確認してください。",
        detail,
      );
    case "profile_conflict":
      return withDetail(
        "既存の built-in／外部 ID または保存済みファイルと競合しています。別の ID を使うか、同一内容で再試行してください。",
        detail,
      );
    case "profile_io_failed":
      return withDetail(
        "プロファイルの保存または journal 記録に失敗しました。extension root の所有権、0700 権限、symlink、空き容量を確認してください。",
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

export function isTrialTokenRejected(reason: unknown): boolean {
  if (typeof reason !== "object" || reason === null) return false;
  return (reason as { code?: unknown }).code === "trial_token_invalid";
}

export function reconnectSessionId(reason: unknown): string | null {
  if (
    !(reason instanceof GuiRequestError) ||
    ![
      "trial_workspace_running",
      "trial_workspace_recovery_required",
      "trial_workspace_conflict",
    ].includes(reason.code)
  ) {
    return null;
  }
  if (reason.sessionId !== null && SESSION_ID_PATTERN.test(reason.sessionId)) {
    return reason.sessionId;
  }
  return (
    reason.serverMessage.match(
      /(?:already running session|non-terminal session) ([0-9a-f]{8}-[0-9a-f-]{27})/i,
    )?.[1] ?? null
  );
}

const SESSION_ID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function withDetail(guidance: string, detail: string): string {
  return `${guidance} 詳細: ${detail}`;
}
