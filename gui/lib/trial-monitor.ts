export const POLL_INTERVAL_MS = 750;
export const TERMINAL_FAILURE_LIMIT = 4;

const MAX_BACKOFF_MS = 12_000;

export type MonitorStatus = "connected" | "degraded" | "lost";

export type MonitorFailure = {
  guidance: string;
  summary: string;
  terminal: boolean;
};

export function retryDelay(attempt: number): number {
  const exponent = Math.max(0, Math.min(attempt - 1, 10));
  return Math.min(POLL_INTERVAL_MS * 2 ** exponent, MAX_BACKOFF_MS);
}

export async function responseFailure(response: Response): Promise<MonitorFailure> {
  if (response.type === "opaqueredirect") {
    return {
      guidance:
        "監視が上流アクセスのサインインへ転送されました。ページを再読み込みしてプロキシで再認証し、実行時トークンを再入力してください。",
      summary: "上流アクセスの再認証が必要です",
      terminal: false,
    };
  }

  const detail = await responseDetail(response);
  if (response.status === 401 || response.status === 403) {
    return {
      guidance: `監視の認証に失敗しました (${response.status})。実行時の Trial アクセストークンを再入力し、このオリジンが許可されているか確認してください。`,
      summary: detail,
      terminal: false,
    };
  }

  const invalidJsonl = /invalid[^.\n]*jsonl|jsonl[^.\n]*invalid/i.test(detail);
  return {
    guidance:
      response.status === 413
        ? "セッションのイベントストリームがポーリング上限を超えました。CLI の成果物を直接確認してください。"
        : invalidJsonl
          ? "セッションのイベント JSONL が不正です。再接続する前に既存の成果物を確認し、修復してください。"
          : `監視リクエストに失敗しました (${response.status || "状態不明"})。上限付きバックオフで再試行します。`,
    summary: detail,
    terminal: response.status === 413 || invalidJsonl,
  };
}

export function thrownFailure(reason: unknown): MonitorFailure {
  const detail = reason instanceof Error ? reason.message : "ブラウザがリクエストを拒否しました。";
  return {
    guidance:
      "監視がサーバーへ接続できません。プロキシまたはネットワーク接続を確認し、必要なら再読み込みまたは再認証して、実行時トークンを再入力してください。",
    summary: detail,
    terminal: false,
  };
}

async function responseDetail(response: Response): Promise<string> {
  const text = await response.text();
  try {
    const parsed = JSON.parse(text) as { error?: string };
    return `${response.status}: ${parsed.error ?? text}`;
  } catch {
    return `${response.status}: ${text}`;
  }
}
