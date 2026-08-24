export const POLL_INTERVAL_MS = 750;
export const CHANGED_POLL_INTERVAL_MS = 1_000;
export const MAX_UNCHANGED_POLL_INTERVAL_MS = 10_000;
export const TERMINAL_FAILURE_LIMIT = 4;

const MAX_BACKOFF_MS = 12_000;

export type MonitorStatus = "connected" | "degraded" | "lost";

export type MonitorFailure = {
  status: number;
  code: string | null;
  guidance: string;
  summary: string;
  terminal: boolean;
};

export function retryDelay(attempt: number): number {
  const exponent = Math.max(0, Math.min(attempt - 1, 10));
  return Math.min(POLL_INTERVAL_MS * 2 ** exponent, MAX_BACKOFF_MS);
}

export function unchangedPollDelay(unchangedResponses: number): number {
  const exponent = Math.max(0, Math.min(unchangedResponses, 10));
  return Math.min(CHANGED_POLL_INTERVAL_MS * 2 ** exponent, MAX_UNCHANGED_POLL_INTERVAL_MS);
}

export async function responseFailure(response: Response): Promise<MonitorFailure> {
  if (response.type === "opaqueredirect") {
    return {
      status: response.status,
      code: null,
      guidance:
        "監視が上流アクセスのサインインへ転送されました。ページを再読み込みしてプロキシで再認証し、実行時トークンを再入力してください。",
      summary: "上流アクセスの再認証が必要です",
      terminal: false,
    };
  }

  const detail = await responseDetail(response);
  if (response.status === 401 || response.status === 403) {
    return {
      status: response.status,
      code: detail.code,
      guidance: `監視の認証に失敗しました (${response.status})。実行時の Trial アクセストークンを再入力し、このオリジンが許可されているか確認してください。`,
      summary: detail.summary,
      terminal: false,
    };
  }

  const sessionMissing = response.status === 404;
  const invalidJsonl =
    detail.code === "trial_session_events_invalid" ||
    /invalid[^.\n]*jsonl|jsonl[^.\n]*invalid/i.test(detail.summary);
  return {
    status: response.status,
    code: detail.code,
    guidance:
      response.status === 413
        ? "セッションのイベントストリームがポーリング上限を超えました。CLI の成果物を直接確認してください。"
        : sessionMissing
          ? "監視対象のセッションが見つかりません (HTTP 404)。セッション ID と実行ルートを確認して再接続するか、新しい実行を開始してください。"
          : invalidJsonl
            ? "セッションのイベント JSONL が不正です。既存のイベントと成果物を確認して修復してから再接続してください。"
            : `監視リクエストに失敗しました (${response.status || "状態不明"})。上限付きバックオフで再試行します。`,
    summary: detail.summary,
    terminal: response.status === 413 || invalidJsonl || sessionMissing,
  };
}

export function thrownFailure(reason: unknown): MonitorFailure {
  const detail = reason instanceof Error ? reason.message : "ブラウザがリクエストを拒否しました。";
  return {
    status: 0,
    code: null,
    guidance:
      "監視がサーバーへ接続できません。プロキシまたはネットワーク接続を確認し、必要なら再読み込みまたは再認証して、実行時トークンを再入力してください。",
    summary: detail,
    terminal: false,
  };
}

export function monitorFailure(reason: unknown): MonitorFailure {
  if (isMonitorFailure(reason)) return reason;
  return thrownFailure(reason);
}

export function isMonitorFailure(reason: unknown): reason is MonitorFailure {
  if (typeof reason !== "object" || reason === null) return false;
  const candidate = reason as Partial<MonitorFailure>;
  return (
    typeof candidate.status === "number" &&
    (typeof candidate.code === "string" || candidate.code === null) &&
    typeof candidate.guidance === "string" &&
    typeof candidate.summary === "string" &&
    typeof candidate.terminal === "boolean"
  );
}

async function responseDetail(
  response: Response,
): Promise<{ code: string | null; summary: string }> {
  const text = await response.text();
  try {
    const parsed = JSON.parse(text) as { code?: unknown; error?: string };
    return {
      code: typeof parsed.code === "string" ? parsed.code : null,
      summary: `${response.status}: ${parsed.error ?? text}`,
    };
  } catch {
    return { code: null, summary: `${response.status}: ${text}` };
  }
}
