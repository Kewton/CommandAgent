"use client";

import { useShellRuntimeStatus } from "./shell";
import { trialRoutePath, withBasePath } from "../lib/base-path";
import type { RuntimePrerequisite } from "../lib/types";

const prerequisiteLabels = {
  execution_root: "トライアルの作業場所",
  extension_root: "非公開の拡張ルート",
  commandagent_binary: "CommandAgent CLI",
  trial_authentication: "トライアルアクセス",
} as const;

const firstUseRoutes = [
  ["01", "実行指示", "目標を入力し、実行前の Gate 1 を確認します。", "compose"],
  ["02", "実行状況", "進行中のセッションへ読み取り専用で再接続します。", "status"],
  ["03", "実行履歴", "開始時刻、状態、profile、目的、pack の要約を探します。", "history"],
  ["04", "結果詳細", "最終判定、失敗診断、受入シート、イベント、成果物を読みます。", "detail"],
] as const;

export function GettingStarted() {
  const runtime = useShellRuntimeStatus();
  const runtimeData = runtime?.data ?? null;

  return (
    <section
      aria-labelledby="getting-started-heading"
      className="getting-started panel"
      data-testid="getting-started"
    >
      <header>
        <div>
          <span className="panel-index">FIRST USE / はじめに</span>
          <h2 id="getting-started-heading">最初のトライアルから結果確認まで</h2>
          <p>前提を確認し、サンプル目標から実行前確認、進行状況、履歴、結果へ順に進みます。</p>
        </div>
      </header>

      <div className="getting-started-body">
        <div className="prerequisite-list" data-testid="getting-started-prerequisites">
          <h3>前提チェック</h3>
          {runtime?.failed ? (
            <p className="source-note" role="status">
              runtime-status を取得できません。以前の値を準備済みとして扱いません。
            </p>
          ) : runtimeData === null ? (
            <p className="source-note">
              実行環境を確認中です…
            </p>
          ) : (
            Object.entries(prerequisiteLabels).map(([id, label]) => (
              <PrerequisiteRow
                key={id}
                label={label}
                prerequisite={runtimeData.prerequisites[id as keyof typeof prerequisiteLabels]}
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
            href={withBasePath(`${trialRoutePath("compose")}?sample=python-cli`)}
          >
            サンプル目標をトライアルに入力
          </a>
        </div>
      </div>

      <nav aria-label="最初のトライアルの確認先" className="getting-started-route-grid">
        {firstUseRoutes.map(([index, title, detail, route]) => (
          <a href={withBasePath(trialRoutePath(route))} key={route}>
            <span>{index}</span>
            <strong>{title}</strong>
            <small>{detail}</small>
          </a>
        ))}
      </nav>

      <details className="term-help" data-testid="getting-started-terms">
        <summary>用語ヘルプ</summary>
        <dl>
          <div><dt>Gate 1</dt><dd>CLI を動かす前に、目標・変更範囲・検証条件を確認する段階です。</dd></div>
          <div><dt>実行ルート</dt><dd>トライアルがファイルを変更できる、専用の作業ディレクトリです。</dd></div>
          <div><dt>profile</dt><dd>タスク向けの進め方と、最低限必要な検証をまとめたものです。</dd></div>
          <div><dt>pack</dt><dd>目標に追加する検証知識。選択した版とハッシュが確認内容に固定されます。</dd></div>
          <div><dt>assurance</dt><dd>実際に通った検証と証拠から得る保証水準です。</dd></div>
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
