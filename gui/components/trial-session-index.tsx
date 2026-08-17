"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { apiPath } from "../lib/base-path";
import { describeError, responseError } from "../lib/errors";
import type {
  TrialSessionIndex,
  TrialSessionSummary,
  TrialWorkspaceLease,
} from "../lib/types";
import { useShellRuntimeStatus } from "./shell";

const COMPLETE_TOKEN_LENGTH = 32;

type ObservedSession = Pick<TrialSessionSummary, "gate" | "id" | "status">;

type TrialSessionIndexProps = {
  accessToken: string;
  observedSession: ObservedSession | null;
  onAccessTokenRejected: (reason: unknown, rejectedValue: string) => void;
  onLeaseChange: (lease: TrialWorkspaceLease | null) => void;
  revalidationKey: number;
};

export function TrialSessionIndexPanel({
  accessToken,
  observedSession,
  onAccessTokenRejected,
  onLeaseChange,
  revalidationKey,
}: TrialSessionIndexProps) {
  const [sessionIndex, setSessionIndex] = useState<TrialSessionIndex | null>(null);
  const [busy, setBusy] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastSuccessAt, setLastSuccessAt] = useState<string | null>(null);
  const requestSequence = useRef(0);
  const previousRevalidationKey = useRef(revalidationKey);
  const runtime = useShellRuntimeStatus();
  const previousRuntimeLease = useRef<string | null>(null);
  const trimmedToken = accessToken.trim();
  const tokenAuthEnabled = runtime?.data?.trial_token_auth_enabled !== false;
  const authenticated = !tokenAuthEnabled || trimmedToken.length >= COMPLETE_TOKEN_LENGTH;

  const revalidate = useCallback(
    async (token: string, manual = false) => {
      const requestId = ++requestSequence.current;
      setRefreshing(true);
      if (manual) setBusy(true);
      try {
        const value = await fetchSessionIndex(token);
        if (requestId !== requestSequence.current) return;
        setSessionIndex(value);
        onLeaseChange(value.lease);
        setLastSuccessAt(new Date().toISOString());
        setError(null);
      } catch (reason) {
        if (requestId !== requestSequence.current) return;
        onAccessTokenRejected(reason, token);
        setError(describeError(reason));
      } finally {
        if (requestId === requestSequence.current) {
          setRefreshing(false);
          if (manual) setBusy(false);
        }
      }
    },
    [onAccessTokenRejected, onLeaseChange],
  );

  useEffect(() => {
    requestSequence.current += 1;
    if (!authenticated) {
      setSessionIndex(null);
      setLastSuccessAt(null);
      setError(null);
      setRefreshing(false);
      setBusy(false);
      onLeaseChange(null);
      return;
    }

    setSessionIndex(null);
    setLastSuccessAt(null);
    setError(null);
    void revalidate(trimmedToken);

    const refresh = () => void revalidate(trimmedToken);
    const refreshWhenVisible = () => {
      if (document.visibilityState === "visible") refresh();
    };
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refreshWhenVisible);
    return () => {
      requestSequence.current += 1;
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refreshWhenVisible);
    };
  }, [authenticated, onLeaseChange, revalidate, trimmedToken]);

  useEffect(() => {
    if (previousRevalidationKey.current === revalidationKey) return;
    previousRevalidationKey.current = revalidationKey;
    if (authenticated) void revalidate(trimmedToken);
  }, [authenticated, revalidate, revalidationKey, trimmedToken]);

  const runtimeLease = runtime?.data === null
    ? null
    : runtime?.data.session?.state ?? "idle";
  useEffect(() => {
    const previous = previousRuntimeLease.current;
    previousRuntimeLease.current = runtimeLease;
    if (
      authenticated &&
      previous === "running" &&
      (runtimeLease === "idle" || runtimeLease === "recovery_required")
    ) {
      void revalidate(trimmedToken);
    }
  }, [authenticated, revalidate, runtimeLease, trimmedToken]);

  const sessions = useMemo(
    () => mergeObservedSession(sessionIndex?.sessions ?? [], observedSession),
    [observedSession, sessionIndex],
  );

  return (
    <section
      className="panel session-index"
      data-authenticated={authenticated}
      data-refreshing={refreshing}
      data-testid="trial-session-index"
      id="trial-session-history"
    >
      <header className="panel-heading">
        <div>
          <span className="panel-index">EXECUTION ROOT / .anvil/runs</span>
          <h2>GUI Trial 実行履歴</h2>
          <p className="source-note">設定された execution root 内の Trial セッションです。</p>
        </div>
        <button
          className="secondary-action"
          data-testid="refresh-trial-sessions"
          disabled={busy || !authenticated}
          onClick={() => void revalidate(trimmedToken, true)}
          type="button"
        >
          セッションを更新
        </button>
      </header>
      {!authenticated && (
        <p className="session-index-empty" data-testid="trial-session-auth-required">
          Trial 履歴は認証待ちです。完全な Trial アクセストークンを入力してください。
        </p>
      )}
      {authenticated && (
        <p className="session-index-freshness" data-testid="trial-session-freshness">
          {lastSuccessAt === null
            ? refreshing ? "実行ルートを確認中…" : "取得成功記録なし"
            : `最終取得: ${lastSuccessAt}${refreshing ? " · 再検証中" : ""}`}
        </p>
      )}
      {error !== null && (
        <div className="trial-error session-index-error" role="alert">
          <strong>Trial 履歴の更新エラー</strong>
          <p>{error}</p>
          {sessionIndex !== null && <small>最後に取得できた一覧を表示しています。</small>}
        </div>
      )}
      {authenticated && sessionIndex !== null && sessions.length === 0 && (
        <p className="session-index-empty">確認済み GUI Trial セッションはありません。</p>
      )}
      {authenticated && sessions.length > 0 && (
        <ol className="session-list">
          {sessions.map((session) => (
            <li id={sessionAnchor(session.id)} key={session.id}>
              <div>
                <code>{session.id}</code>
                <time>開始: {sessionTime(session.started_epoch_seconds)}</time>
                <time>最終更新: {sessionTime(session.modified_epoch_seconds)}</time>
              </div>
              <span className={`session-status ${session.status}`}>
                {session.gate ?? "unknown"} / {session.status}
              </span>
              <a data-testid="session-reconnect-link" href={sessionLink(session.id)}>
                再接続
              </a>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

async function fetchSessionIndex(token: string): Promise<TrialSessionIndex> {
  const response = await fetch(apiPath("sessions"), {
    cache: "no-store",
    headers: token.trim() === ""
      ? {}
      : { "x-commandagent-trial-authorization": `Bearer ${token.trim()}` },
  });
  if (!response.ok) throw await responseError(response);
  return (await response.json()) as TrialSessionIndex;
}

function mergeObservedSession(
  sessions: TrialSessionSummary[],
  observed: ObservedSession | null,
): TrialSessionSummary[] {
  if (observed === null) return sessions;
  const projected = sessions.find((session) => session.id === observed.id);
  if (projected !== undefined) {
    const projectedIsTerminal = projected.gate === "gate_3" || projected.gate === "gate_4";
    const observedIsTerminal = observed.gate === "gate_3" || observed.gate === "gate_4";
    const current = observedIsTerminal
      ? { ...projected, gate: observed.gate, status: observed.status }
      : projectedIsTerminal || observed.status === "starting"
        ? projected
        : { ...projected, gate: observed.gate, status: observed.status };
    return [current, ...sessions.filter((session) => session.id !== observed.id)];
  }
  return [
    {
      ...observed,
      modified_epoch_seconds: 0,
      started_epoch_seconds: 0,
    },
    ...sessions,
  ];
}

function sessionAnchor(id: string): string {
  return `trial-session-${id}`;
}

function sessionLink(id: string): string {
  return `?session=${encodeURIComponent(id)}`;
}

function sessionTime(epochSeconds: number): string {
  if (epochSeconds <= 0) return "反映待ち";
  return new Date(epochSeconds * 1_000).toISOString();
}
