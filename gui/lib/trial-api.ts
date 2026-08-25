import { apiPath } from "./base-path";
import { responseError } from "./errors";
import { responseFailure, thrownFailure, type MonitorFailure } from "./trial-monitor";
import type {
  CreatedSession,
  DirectiveProposal,
  DocumentRecord,
  DocumentSummary,
  PolledSession,
  PackOptions,
  SessionProposal,
  SessionPathProjection,
  SessionSpec,
  TrialIntent,
  TrialOptions,
  TrialSessionIndex,
  TrialWorkspaceLease,
} from "./types";

export type SessionPollResult = {
  etag: string | null;
  value: PolledSession | null;
};

export function trialAuthorizationHeaders(
  token: string,
  json = false,
): Record<string, string> {
  return {
    ...(token.trim() === ""
      ? {}
      : { "x-commandagent-trial-authorization": `Bearer ${token.trim()}` }),
    ...(json ? { "content-type": "application/json" } : {}),
  };
}

export async function fetchTrialOptions(): Promise<TrialOptions> {
  return fetchJson<TrialOptions>(apiPath("trial-options"));
}

export async function fetchPackOptions(): Promise<PackOptions> {
  return fetchJson<PackOptions>(apiPath("pack-options"));
}

export async function fetchWorkspaceLease(token: string): Promise<TrialWorkspaceLease> {
  return fetchJson<TrialWorkspaceLease>(apiPath("trial-workspace"), {
    headers: trialAuthorizationHeaders(token),
  });
}

export async function proposeSession(
  token: string,
  spec: SessionSpec,
): Promise<SessionProposal> {
  return fetchJson<SessionProposal>(apiPath("session-proposals"), {
    method: "POST",
    headers: trialAuthorizationHeaders(token, true),
    body: JSON.stringify(sessionRequestSpec(spec)),
  });
}

export async function createSession(
  token: string,
  spec: SessionSpec,
  confirmationHash: string,
): Promise<CreatedSession> {
  return fetchJson<CreatedSession>(apiPath("sessions"), {
    method: "POST",
    headers: trialAuthorizationHeaders(token, true),
    body: JSON.stringify({ ...sessionRequestSpec(spec), confirmation_hash: confirmationHash }),
  });
}

function sessionRequestSpec(spec: SessionSpec): Omit<SessionSpec, "intent" | "working_directory"> & {
  intent?: TrialIntent;
  working_directory?: string;
} {
  const { intent, working_directory, ...request } = spec;
  return {
    ...request,
    ...(intent === null ? {} : { intent }),
    ...(working_directory.trim() === "" ? {} : { working_directory }),
  };
}

export async function createDirective(
  token: string,
  sessionId: string,
  directive: string,
): Promise<DirectiveProposal> {
  return fetchJson<DirectiveProposal>(
    apiPath(`sessions/${encodeURIComponent(sessionId)}/directives`),
    {
      method: "POST",
      headers: trialAuthorizationHeaders(token, true),
      body: JSON.stringify({ directive }),
    },
  );
}

export async function confirmDirective(
  token: string,
  sessionId: string,
  directiveHash: string,
): Promise<void> {
  await fetchOk(
    apiPath(
      `sessions/${encodeURIComponent(sessionId)}/directives/${encodeURIComponent(directiveHash)}`,
    ),
    {
      method: "POST",
      headers: trialAuthorizationHeaders(token, true),
      body: "{}",
    },
  );
}

export async function fetchSessionArtifacts(
  token: string,
  sessionId: string,
): Promise<DocumentSummary[]> {
  return fetchJson<DocumentSummary[]>(
    apiPath(`sessions/${encodeURIComponent(sessionId)}/artifacts`),
    { headers: trialAuthorizationHeaders(token) },
  );
}

export async function fetchSessionPaths(
  token: string,
  sessionId: string,
): Promise<SessionPathProjection> {
  return fetchJson<SessionPathProjection>(
    apiPath(`sessions/${encodeURIComponent(sessionId)}/paths`),
    {
      cache: "no-store",
      headers: trialAuthorizationHeaders(token),
    },
  );
}

export async function fetchSessionEvents(
  token: string,
  sessionId: string,
): Promise<DocumentRecord> {
  return fetchTrialDocument(
    token,
    apiPath(
      `sessions/${encodeURIComponent(sessionId)}/events`,
      new URLSearchParams({ tail: "200" }),
    ),
  );
}

export async function fetchSessionArtifact(
  token: string,
  sessionId: string,
  path: string,
): Promise<DocumentRecord> {
  return fetchTrialDocument(
    token,
    apiPath(
      `sessions/${encodeURIComponent(sessionId)}/artifacts`,
      new URLSearchParams({ path }),
    ),
  );
}

export async function fetchSessionRecoveryDocument(
  token: string,
  sessionId: string,
  path: string,
): Promise<DocumentRecord> {
  return fetchTrialDocument(
    token,
    apiPath(
      `sessions/${encodeURIComponent(sessionId)}/recovery-document`,
      new URLSearchParams({ path }),
    ),
  );
}

export async function fetchSessionIndex(token: string): Promise<TrialSessionIndex> {
  return fetchJson<TrialSessionIndex>(apiPath("sessions"), {
    cache: "no-store",
    headers: trialAuthorizationHeaders(token),
  });
}

export async function fetchSession(id: string, token: string): Promise<PolledSession> {
  const result = await fetchSessionPoll(id, token, null);
  if (result.value !== null) return result.value;
  throw {
    status: 304,
    code: "unexpected_not_modified",
    guidance:
      "監視が初回取得で304応答を受信しました。プロキシのキャッシュ設定を確認してから再接続してください。",
    summary: "初回のセッション状態がありません",
    terminal: true,
  } satisfies MonitorFailure;
}

export async function fetchSessionPoll(
  id: string,
  token: string,
  etag: string | null,
): Promise<SessionPollResult> {
  let response: Response;
  try {
    const headers = trialAuthorizationHeaders(token);
    if (etag !== null) headers["if-none-match"] = etag;
    response = await fetch(apiPath(`sessions/${encodeURIComponent(id)}`), {
      headers,
      redirect: "manual",
    });
  } catch (reason) {
    throw thrownFailure(reason);
  }
  if (response.status === 304) {
    return { etag: response.headers.get("etag") ?? etag, value: null };
  }
  if (response.type === "opaqueredirect" || !response.ok) {
    throw await responseFailure(response);
  }
  try {
    return {
      etag: response.headers.get("etag"),
      value: (await response.json()) as PolledSession,
    };
  } catch (reason) {
    throw {
      status: response.status,
      code: "invalid_session_response",
      guidance:
        "監視が不正な状態応答を受信しました。再接続する前に、プロキシ応答と既存セッションの成果物を確認してください。",
      summary: reason instanceof Error ? reason.message : "Trial リクエストに失敗しました。",
      terminal: true,
    } satisfies MonitorFailure;
  }
}

async function fetchTrialDocument(token: string, url: string): Promise<DocumentRecord> {
  return fetchJson<DocumentRecord>(url, { headers: trialAuthorizationHeaders(token) });
}

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, init);
  if (!response.ok) throw await responseError(response);
  return (await response.json()) as T;
}

async function fetchOk(url: string, init?: RequestInit): Promise<void> {
  const response = await fetch(url, init);
  if (!response.ok) throw await responseError(response);
}
