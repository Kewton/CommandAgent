"use client";

import { useEffect, useState } from "react";

import { describeError } from "../lib/errors";
import { fetchSessionPaths } from "../lib/trial-api";
import type { SessionPathProjection } from "../lib/types";

type TrialSessionPathsProps = {
  accessToken: string;
  authenticationEnabled: boolean;
  onAccessTokenRejected: (reason: unknown, rejectedValue: string) => void;
  sessionId: string | null;
};

export function TrialSessionPaths({
  accessToken,
  authenticationEnabled,
  onAccessTokenRejected,
  sessionId,
}: TrialSessionPathsProps) {
  const [paths, setPaths] = useState<SessionPathProjection | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copyAnnouncement, setCopyAnnouncement] = useState("");
  const token = accessToken.trim();

  useEffect(() => {
    setPaths(null);
    setError(null);
    setCopyAnnouncement("");
    if (!authenticationEnabled || sessionId === null || token === "") {
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    void fetchSessionPaths(token, sessionId)
      .then((value) => {
        if (!cancelled) setPaths(value);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        onAccessTokenRejected(reason, token);
        setError(describeError(reason));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, [authenticationEnabled, onAccessTokenRejected, sessionId, token]);

  async function copyWorkingDirectory() {
    const path = paths?.working_directory.path;
    if (path === undefined) return;
    try {
      if (navigator.clipboard?.writeText === undefined) {
        throw new Error("Clipboard API is unavailable");
      }
      await navigator.clipboard.writeText(path);
      setCopyAnnouncement("作業ディレクトリのパスをクリップボードにコピーしました。");
    } catch {
      setCopyAnnouncement("コピーできませんでした。パスを選択して手動でコピーしてください。");
    }
  }

  return (
    <section
      className="panel trial-session-paths"
      data-state={pathPanelState(authenticationEnabled, sessionId, token, loading, paths, error)}
      data-testid="trial-session-paths"
    >
      <header className="panel-heading">
        <div>
          <span className="panel-index">実行場所 / 読み取り専用</span>
          <h2>セッションの作業ディレクトリ</h2>
        </div>
      </header>

      {!authenticationEnabled && (
        <p className="source-note" data-testid="trial-session-paths-auth-required">
          絶対パスは Trial トークン認証を有効にした専用セッション API からだけ取得できます。
        </p>
      )}
      {authenticationEnabled && sessionId === null && (
        <p className="source-note">セッション ID を指定すると作業ディレクトリを確認できます。</p>
      )}
      {authenticationEnabled && sessionId !== null && token === "" && (
        <p className="source-note">絶対パスを取得するにはトライアルアクセストークンを入力してください。</p>
      )}
      {loading && <p className="source-note">作業ディレクトリを確認中…</p>}
      {error !== null && <p className="trial-error" role="alert">{error}</p>}

      {paths !== null && (
        <div className="trial-session-paths-grid">
          <div className="trial-working-directory" data-testid="trial-working-directory">
            <div>
              <span className="trial-path-label">CLI 作業ディレクトリ</span>
              <code data-testid="trial-working-directory-path">
                {paths.working_directory.path}
              </code>
            </div>
            <span
              className={`trial-path-state ${paths.working_directory.state}`}
              data-testid="trial-working-directory-state"
            >
              {paths.working_directory.state === "available" ? "利用可能" : "削除済み"}
            </span>
            <button
              aria-label="作業ディレクトリの絶対パスをコピー"
              className="secondary-action"
              data-testid="copy-working-directory"
              onClick={() => void copyWorkingDirectory()}
              type="button"
            >
              パスをコピー
            </button>
          </div>
          {paths.working_directory.state === "missing" && (
            <p className="trial-path-missing" data-testid="trial-working-directory-missing" role="status">
              この作業ディレクトリは削除済みです。生成コードや実行対象が残っている状態ではありません。
            </p>
          )}
          <div className="trial-run-record-paths" data-testid="trial-run-record-paths">
            <strong>実行記録の保存先（作業ディレクトリとは別）</strong>
            <dl>
              <div><dt>記録ディレクトリ</dt><dd><code>{paths.run_records.directory}</code></dd></div>
              <div><dt>イベント</dt><dd><code>{paths.run_records.events}</code></dd></div>
              <div><dt>サマリー</dt><dd><code>{paths.run_records.summary}</code></dd></div>
            </dl>
          </div>
        </div>
      )}
      <p
        aria-atomic="true"
        aria-live="polite"
        className="trial-copy-announcement"
        data-testid="trial-copy-announcement"
        role="status"
      >
        {copyAnnouncement}
      </p>
    </section>
  );
}

function pathPanelState(
  authenticationEnabled: boolean,
  sessionId: string | null,
  token: string,
  loading: boolean,
  paths: SessionPathProjection | null,
  error: string | null,
): string {
  if (!authenticationEnabled) return "authentication_required";
  if (sessionId === null) return "session_required";
  if (token === "") return "token_required";
  if (loading) return "loading";
  if (error !== null) return "error";
  return paths?.working_directory.state ?? "loading";
}
