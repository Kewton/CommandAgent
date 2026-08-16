"use client";

import { useEffect, useState } from "react";

import { apiPath } from "../lib/base-path";
import type {
  TrialSessionIndex,
  TrialWorkspaceLease,
} from "../lib/types";

type TrialSessionIndexProps = {
  lease: TrialWorkspaceLease | null;
  onLeaseChange: (lease: TrialWorkspaceLease | null) => void;
  token: string;
};

export function TrialSessionIndexPanel({
  lease,
  onLeaseChange,
  token,
}: TrialSessionIndexProps) {
  const [sessionIndex, setSessionIndex] = useState<TrialSessionIndex | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const trimmed = token.trim();
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
          if (!cancelled) setError(message(reason));
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [onLeaseChange, token]);

  async function refresh() {
    if (token.trim() === "") {
      setError("Enter the runtime Trial access token before refreshing sessions.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const value = await fetchSessionIndex(token);
      setSessionIndex(value);
      onLeaseChange(value.lease);
    } catch (reason) {
      setError(message(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel session-index" data-testid="trial-session-index">
      <header className="panel-heading">
        <div>
          <span className="panel-index">EXECUTION ROOT / SESSIONS</span>
          <h2>Trial history and workspace lease</h2>
        </div>
        <button
          className="secondary-action"
          data-testid="refresh-trial-sessions"
          disabled={busy || token.trim() === ""}
          onClick={() => void refresh()}
          type="button"
        >
          Refresh sessions
        </button>
      </header>
      <div className="lease-summary" data-testid="workspace-lease-status">
        <span>Workspace lease</span>
        <strong>{leaseLabel(lease)}</strong>
        <small>Read-only snapshot; the server-side lease remains authoritative.</small>
      </div>
      {error !== null && <p className="trial-error" role="alert">{error}</p>}
      {sessionIndex === null && error === null && (
        <p className="session-index-empty">Enter the runtime Trial token to load this execution root.</p>
      )}
      {sessionIndex !== null && sessionIndex.sessions.length === 0 && (
        <p className="session-index-empty">No confirmed Trial sessions were found.</p>
      )}
      {sessionIndex !== null && sessionIndex.sessions.length > 0 && (
        <ol className="session-list">
          {sessionIndex.sessions.map((session) => (
            <li key={session.id}>
              <div>
                <code>{session.id}</code>
                <time>{sessionTime(session.modified_epoch_seconds)}</time>
              </div>
              <span className={`session-status ${session.status}`}>{session.status}</span>
              <a data-testid="session-reconnect-link" href={sessionLink(session.id)}>
                Reconnect
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
  if (!response.ok) throw new Error(await apiError(response));
  return (await response.json()) as TrialSessionIndex;
}

function leaseLabel(lease: TrialWorkspaceLease | null): string {
  if (lease === null) return "not loaded";
  if (lease.status === "idle") return "idle";
  return `${lease.status}(${lease.session_id})`;
}

function sessionLink(id: string): string {
  return `?session=${encodeURIComponent(id)}`;
}

function sessionTime(epochSeconds: number): string {
  if (epochSeconds <= 0) return "time not recorded";
  return new Date(epochSeconds * 1_000).toISOString();
}

async function apiError(response: Response): Promise<string> {
  const text = await response.text();
  try {
    const parsed = JSON.parse(text) as { error?: string };
    return `${response.status}: ${parsed.error ?? text}`;
  } catch {
    return `${response.status}: ${text}`;
  }
}

function message(reason: unknown): string {
  return reason instanceof Error ? reason.message : "The session index request failed.";
}
