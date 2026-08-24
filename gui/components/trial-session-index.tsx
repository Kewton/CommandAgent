"use client";

import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";

import { describeError } from "../lib/errors";
import { dateTimeLabel, trialGateLabel, trialStatusLabel } from "../lib/format";
import { fetchSessionIndex } from "../lib/trial-api";
import { trialRoutePath } from "../lib/base-path";
import type {
  TrialSessionIndex,
  TrialSessionSummary,
  TrialWorkspaceLease,
} from "../lib/types";
import { useShellRuntimeStatus } from "./shell";

const COMPLETE_TOKEN_LENGTH = 32;

type TrialSessionIndexProps = {
  accessToken: string;
  onAccessTokenRejected: (reason: unknown, rejectedValue: string) => void;
  onLeaseChange: (lease: TrialWorkspaceLease | null) => void;
};

export function TrialSessionIndexPanel({
  accessToken,
  onAccessTokenRejected,
  onLeaseChange,
}: TrialSessionIndexProps) {
  const [sessionIndex, setSessionIndex] = useState<TrialSessionIndex | null>(null);
  const [busy, setBusy] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastSuccessAt, setLastSuccessAt] = useState<string | null>(null);
  const requestSequence = useRef(0);
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

  const sessions = sessionIndex?.sessions ?? [];

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
              data-terminal={isTerminalSession(session)}
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
              <span className="session-profile" data-testid="session-profile">
                プロファイル: {session.profile ?? "記録なし"}
              </span>
              <span className="session-intent" data-testid="session-intent">
                目的: {intentLabel(session.intent)}
              </span>
              <span className="session-pack" data-testid="session-pack">
                {session.pack === null
                  ? "パック: 選択なし"
                  : `パック: ${session.pack.id}@${session.pack.version} · ${session.pack.source_label}`}
              </span>
              <Link data-testid="session-route-link" href={sessionLink(session)}>
                {isTerminalSession(session) ? "結果詳細" : "進行状況"}
              </Link>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function sessionAnchor(id: string): string {
  return `trial-session-${id}`;
}

function sessionLink(session: TrialSessionSummary): string {
  return trialRoutePath(isTerminalSession(session) ? "detail" : "status", session.id);
}

function isTerminalSession(session: TrialSessionSummary): boolean {
  return session.gate === "gate_3" || session.gate === "gate_4" ||
    ["completed", "failed", "interrupted", "aborted", "incomplete", "unreadable"]
      .includes(session.status);
}

function intentLabel(intent: TrialSessionSummary["intent"]): string {
  if (intent === "create") return "作成";
  if (intent === "fix") return "修正";
  if (intent === "investigate") return "調査";
  return intent ?? "記録なし";
}
