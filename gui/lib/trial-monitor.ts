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
        "Monitoring reached an upstream access sign-in redirect. Reload this page and re-authenticate with the proxy, then re-enter the runtime token.",
      summary: "Upstream access re-authentication required",
      terminal: false,
    };
  }

  const detail = await responseDetail(response);
  if (response.status === 401 || response.status === 403) {
    return {
      guidance: `Monitoring authorization failed (${response.status}). Re-enter the runtime Trial access token and verify that this origin is authorized.`,
      summary: detail,
      terminal: false,
    };
  }

  const invalidJsonl = /invalid[^.\n]*jsonl|jsonl[^.\n]*invalid/i.test(detail);
  return {
    guidance:
      response.status === 413
        ? "The session event stream exceeds the polling limit. Inspect the CLI artifacts directly."
        : invalidJsonl
          ? "The session event JSONL is invalid. Inspect and repair the existing artifacts before reconnecting."
          : `Monitoring request failed (${response.status || "unknown status"}). Retrying with capped backoff.`,
    summary: detail,
    terminal: response.status === 413 || invalidJsonl,
  };
}

export function thrownFailure(reason: unknown): MonitorFailure {
  const detail = reason instanceof Error ? reason.message : "The browser rejected the request.";
  return {
    guidance:
      "Monitoring could not reach the server. Check the proxy or network connection, reload or re-authenticate if required, then re-enter the runtime token.",
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
