"use client";

import { useEffect, useState } from "react";

import { useShellRuntimeStatus } from "./shell";
import { guiBasePath, routePath, withBasePath } from "../lib/base-path";
import type { RuntimePrerequisite } from "../lib/types";

const DISMISSED_KEY = `commandagent.gui.getting-started-dismissed:${guiBasePath() || "/"}`;

const prerequisiteLabels = {
  execution_root: "トライアルの作業場所",
  commandagent_binary: "CommandAgent CLI",
  trial_authentication: "トライアルアクセス",
} as const;

export function GettingStarted() {
  const runtime = useShellRuntimeStatus();
  const runtimeData = runtime?.data ?? null;
  const [ready, setReady] = useState(false);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    try {
      setDismissed(window.sessionStorage.getItem(DISMISSED_KEY) === "true");
    } catch {
      setDismissed(false);
    } finally {
      setReady(true);
    }
  }, []);

  if (!ready || dismissed) return null;

  const dismiss = () => {
    try {
      window.sessionStorage.setItem(DISMISSED_KEY, "true");
    } catch {
      // A blocked storage API must not make the guide impossible to close.
    }
    setDismissed(true);
  };

  return (
    <section className="getting-started panel" data-testid="getting-started">
      <header>
        <div>
          <span className="panel-index">初回案内 / はじめに</span>
          <h2>最初のトライアルを準備する</h2>
          <p>前提を確認し、サンプル目標から Gate 1 の実行前確認を試せます。</p>
        </div>
        <button
          aria-label="はじめにを閉じる"
          className="getting-started-close"
          data-testid="getting-started-close"
          onClick={dismiss}
          type="button"
        >
          閉じる
        </button>
      </header>

      <div className="getting-started-body">
        <div className="prerequisite-list" data-testid="getting-started-prerequisites">
          <h3>前提チェック</h3>
          {runtimeData === null ? (
            <p className="source-note">
              {runtime?.failed ? "runtime-status を取得できません。" : "実行環境を確認中です…"}
            </p>
          ) : (
            Object.entries(runtimeData.prerequisites).map(([id, prerequisite]) => (
              <PrerequisiteRow
                key={id}
                label={prerequisiteLabels[id as keyof typeof prerequisiteLabels]}
                prerequisite={prerequisite}
              />
            ))
          )}
        </div>

        <div className="getting-started-actions">
          <h3>サンプルから始める</h3>
          <p>「--pattern で行を絞り込む CLI コマンドを作ってください」を入力します。モデル ID は環境に合わせて指定してください。</p>
          <a
            className="primary-action"
            data-testid="getting-started-sample"
            href={withBasePath(`${routePath("try")}?sample=python-cli`)}
          >
            サンプル目標をトライアルに入力
          </a>
        </div>
      </div>

      <details className="term-help" data-testid="getting-started-terms">
        <summary>用語ヘルプ</summary>
        <dl>
          <div><dt>Gate 1</dt><dd>CLI を動かす前に、目標・変更範囲・検証条件を確認する段階です。</dd></div>
          <div><dt>実行ルート</dt><dd>トライアルがファイルを変更できる、専用の作業ディレクトリです。</dd></div>
          <div><dt>パック</dt><dd>目標に追加する検証知識。選択した版とハッシュが確認内容に固定されます。</dd></div>
        </dl>
      </details>
    </section>
  );
}

function PrerequisiteRow({
  label,
  prerequisite,
}: {
  label: string;
  prerequisite: RuntimePrerequisite;
}) {
  const statusLabel = prerequisite.status === "ready"
    ? "準備済み"
    : prerequisite.status === "unconfigured"
      ? "未設定"
      : "要対応";
  return (
    <div className="prerequisite-row" data-status={prerequisite.status}>
      <span aria-hidden="true" />
      <div><strong>{label}</strong><small>{prerequisite.detail}</small></div>
      <em>{statusLabel}</em>
    </div>
  );
}
