"use client";

import { useEffect, useState } from "react";

import { apiPath } from "../lib/base-path";
import { describeError, responseError } from "../lib/errors";
import type {
  TrialSessionIndex,
  TrialWorkspaceLease,
} from "../lib/types";

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
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const trimmed = accessToken.trim();
    if (trimmed.length < 32) {
      setSessionIndex(null);
      setError(null);
      onLeaseChange(null);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void fetchSessionIndex(trimmed)
        .then((value) => {
          if (cancelled) return;
          setSessionIndex(value);
          onLeaseChange(value.lease);
          setError(null);
        })
        .catch((reason: unknown) => {
          if (!cancelled) {
            onAccessTokenRejected(reason, trimmed);
            setError(describeError(reason));
          }
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [accessToken, onAccessTokenRejected, onLeaseChange]);

  async function refresh() {
    if (accessToken.trim() === "") {
      setError("セッションを更新する前に Trial アクセストークンを入力してください。");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const value = await fetchSessionIndex(accessToken);
      setSessionIndex(value);
      onLeaseChange(value.lease);
    } catch (reason) {
      onAccessTokenRejected(reason, accessToken);
      setError(describeError(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel session-index" data-testid="trial-session-index">
      <header className="panel-heading">
        <div>
          <span className="panel-index">EXECUTION ROOT / セッション</span>
          <h2>Trial セッション</h2>
        </div>
        <button
          className="secondary-action"
          data-testid="refresh-trial-sessions"
          disabled={busy || accessToken.trim() === ""}
          onClick={() => void refresh()}
          type="button"
        >
          セッションを更新
        </button>
      </header>
      {error !== null && <p className="trial-error" role="alert">{error}</p>}
      {sessionIndex === null && error === null && (
        <p className="session-index-empty">Trial トークンを入力すると実行ルートを読み込みます。</p>
      )}
      {sessionIndex !== null && sessionIndex.sessions.length === 0 && (
        <p className="session-index-empty">確認済み Trial セッションはありません。</p>
      )}
      {sessionIndex !== null && sessionIndex.sessions.length > 0 && (
        <ol className="session-list">
          {sessionIndex.sessions.map((session) => (
            <li key={session.id}>
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
    headers: {
      "x-commandagent-trial-authorization": `Bearer ${token.trim()}`,
    },
  });
  if (!response.ok) throw await responseError(response);
  return (await response.json()) as TrialSessionIndex;
}

function sessionLink(id: string): string {
  return `?session=${encodeURIComponent(id)}`;
}

function sessionTime(epochSeconds: number): string {
  if (epochSeconds <= 0) return "未記録";
  return new Date(epochSeconds * 1_000).toISOString();
}
