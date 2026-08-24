"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { describeError } from "../lib/errors";
import { dateTimeLabel, trialGateLabel, trialStatusLabel } from "../lib/format";
import { fetchSessionIndex } from "../lib/trial-api";
import type {
  TrialSessionIndex,
  TrialSessionSummary,
  TrialWorkspaceLease,
} from "../lib/types";
import { useShellRuntimeStatus } from "./shell";
import { TrialFailureDiagnostics } from "./trial-failure-diagnostics";

const COMPLETE_TOKEN_LENGTH = 32;

type ObservedSession = Pick<TrialSessionSummary, "gate" | "id" | "status">;

type TrialSessionIndexProps = {
  accessToken: string;
  deferAutomaticRevalidation: boolean;
  highlight: string | null;
  observedSession: ObservedSession | null;
  onAccessTokenRejected: (reason: unknown, rejectedValue: string) => void;
  onLeaseChange: (lease: TrialWorkspaceLease | null) => void;
  revalidationKey: number;
};

export function TrialSessionIndexPanel({
  accessToken,
  deferAutomaticRevalidation,
  highlight,
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
    if (deferAutomaticRevalidation) {
      setRefreshing(false);
      setBusy(false);
      onLeaseChange(null);
      return;
    }
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
  }, [authenticated, deferAutomaticRevalidation, onLeaseChange, revalidate, trimmedToken]);

  useEffect(() => {
    if (previousRevalidationKey.current === revalidationKey) return;
    previousRevalidationKey.current = revalidationKey;
    if (authenticated && !deferAutomaticRevalidation) void revalidate(trimmedToken);
  }, [authenticated, deferAutomaticRevalidation, revalidate, revalidationKey, trimmedToken]);

  const runtimeLease = runtime?.data === null
    ? null
    : runtime?.data.session?.state ?? "idle";
  useEffect(() => {
    const previous = previousRuntimeLease.current;
    previousRuntimeLease.current = runtimeLease;
    if (
      authenticated &&
      !deferAutomaticRevalidation &&
      previous === "running" &&
      (runtimeLease === "idle" || runtimeLease === "recovery_required")
    ) {
      void revalidate(trimmedToken);
    }
  }, [authenticated, deferAutomaticRevalidation, revalidate, runtimeLease, trimmedToken]);

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
          <span className="panel-index">実行ルート / .commandagent/runs</span>
          <h2>トライアル実行履歴</h2>
          <p className="source-note">設定された実行ルート内のトライアルセッションです。</p>
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
          トライアル履歴は認証待ちです。完全なトライアルアクセストークンを入力してください。
        </p>
      )}
      {authenticated && (
        <p className="session-index-freshness" data-testid="trial-session-freshness">
          {lastSuccessAt === null
            ? refreshing ? "実行ルートを確認中…" : "取得成功記録なし"
            : `最終取得: ${dateTimeLabel(lastSuccessAt, "取得成功記録なし")}${refreshing ? " · 再検証中" : ""}`}
        </p>
      )}
      {error !== null && (
        <div className="trial-error session-index-error" role="alert">
          <strong>トライアル履歴の更新エラー</strong>
          <p>{error}</p>
          {sessionIndex !== null && <small>最後に取得できた一覧を表示しています。</small>}
        </div>
      )}
      {authenticated && sessionIndex !== null && sessions.length === 0 && (
        <p className="session-index-empty">確認済みのトライアルセッションはありません。</p>
      )}
      {authenticated && sessions.length > 0 && (
        <ol className="session-list">
          {sessions.map((session) => (
            <li
              aria-current={highlight === session.id ? "true" : undefined}
              className={highlight === session.id ? "highlight" : undefined}
              data-session-id={session.id}
              id={sessionAnchor(session.id)}
              key={session.id}
            >
              <div>
                <code>{session.id}</code>
                <time>開始: {dateTimeLabel(session.started_epoch_seconds, "反映待ち")}</time>
                <time>最終更新: {dateTimeLabel(session.modified_epoch_seconds, "反映待ち")}</time>
              </div>
              <span className={`session-status ${session.status}`}>
                {trialGateLabel(session.gate)} / {trialStatusLabel(session.status)}
              </span>
              <span className="session-pack" data-testid="session-pack">
                {session.pack === null
                  ? "パック: 選択なし"
                  : `パック: ${session.pack.id}@${session.pack.version} · ${session.pack.source_label}`}
              </span>
              <a data-testid="session-reconnect-link" href={sessionLink(session.id)}>
                再接続
              </a>
              {session.status === "failed" && (
                <TrialFailureDiagnostics
                  diagnostics={session.failure_diagnostics}
                  testId="session-failure-diagnostics"
                />
              )}
            </li>
          ))}
        </ol>
      )}
    </section>
  );
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
      pack: null,
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
