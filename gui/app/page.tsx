"use client";

import { Shell } from "../components/shell";
import { EmptyState, ErrorState, LoadingState } from "../components/states";
import { apiPath, routePath, withBasePath } from "../lib/base-path";
import type { DocumentRecord, RunSummary } from "../lib/types";
import { useResource } from "../lib/use-resource";

function statusTone(status: string): string {
  const normalized = status.toLowerCase();
  if (normalized.includes("pass") || normalized.includes("full") || normalized.includes("green")) {
    return "positive";
  }
  if (normalized.includes("fail") || normalized.includes("block")) return "negative";
  return "neutral";
}

function bandFact(document: DocumentRecord): string {
  const denominator = document.content.match(/Included denominator[^:]*:\s*`?([^`\n]+)`?/i)?.[1];
  const window = document.content.match(/Window start:\s*`?([^`\n]+)`?/i)?.[1];
  return denominator === undefined
    ? window ?? "正式な計測記録"
    : `対象記録 ${denominator.trim()} 件`;
}

function shortBandName(document: DocumentRecord): string {
  return document.content.match(/^#\s+(.+)$/m)?.[1] ?? document.id;
}

function dateLabel(epochSeconds: number): string {
  if (epochSeconds === 0) return "時刻未記録";
  return new Intl.DateTimeFormat("ja-JP", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(epochSeconds * 1000));
}

export default function DashboardPage() {
  const runs = useResource<RunSummary[]>("runs");
  const bands = useResource<DocumentRecord[]>("bands");
  const recentRuns = runs.data?.slice(0, 8) ?? [];
  const completed = recentRuns.filter((run) => statusTone(run.status) === "positive").length;

  return (
    <Shell
      active="dashboard"
      title="概要"
      description="リポジトリに記録された実行・計測・運用アセットをまとめて確認します。"
    >
      <section className="metric-strip" aria-label="リポジトリ概要">
        <div>
          <span>表示できる実行</span>
          <strong>{runs.data?.length ?? "—"}</strong>
        </div>
        <div>
          <span>直近の成功</span>
          <strong>{runs.data === null ? "—" : `${completed}/${recentRuns.length}`}</strong>
        </div>
        <div>
          <span>正式なバンド</span>
          <strong>{bands.data?.length ?? "—"}</strong>
        </div>
        <div>
          <span>実行経路</span>
          <strong className="accent-text">CLI のみ</strong>
        </div>
      </section>

      <section className="dashboard-grid">
        <article className="panel map-panel">
          <header className="panel-heading">
            <div>
              <span className="panel-index">A / スコア × 時間</span>
              <h2>能力マップ</h2>
            </div>
            <a href={withBasePath(routePath("measurements"))}>計測を確認 ↗</a>
          </header>
          <div className="map-frame">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img
              data-testid="score-time-map"
              src={apiPath("maps/score-time.svg")}
              alt="CommandAgent のスコアと実行時間の能力マップ"
            />
          </div>
        </article>

        <article className="panel bands-panel">
          <header className="panel-heading">
            <div>
              <span className="panel-index">B / 正式なバンド</span>
              <h2>計測済みの境界</h2>
            </div>
          </header>
          {bands.loading && <LoadingState label="バンド概要を読み込んでいます" />}
          {bands.error !== null && <ErrorState message={bands.error} />}
          {bands.data?.slice(0, 6).map((band, index) => (
            <div className="band-row" key={band.path}>
              <span>{String(index + 1).padStart(2, "0")}</span>
              <div>
                <strong>{shortBandName(band)}</strong>
                <small>{bandFact(band)}</small>
              </div>
            </div>
          ))}
        </article>
      </section>

      <section className="panel asset-entry">
        <div>
          <span className="panel-index">運用アセット</span>
          <h2>パック・契約・計測スイート</h2>
          <p>日常ナビゲーションから外した固定アセットを、必要なときだけ参照できます。</p>
        </div>
        <a data-testid="assets-link" href={withBasePath(routePath("assets"))}>アセットを開く ↗</a>
      </section>

      <section className="panel runs-panel">
        <header className="panel-heading">
          <div>
            <span className="panel-index">C / 実行台帳</span>
            <h2>最近更新された記録</h2>
          </div>
          <span className="live-label"><i /> ファイル投影</span>
        </header>
        {runs.loading && <LoadingState label="実行記録を索引化しています" />}
        {runs.error !== null && <ErrorState message={runs.error} />}
        {runs.data?.length === 0 && <EmptyState message="実行ディレクトリが見つかりません。" />}
        {recentRuns.length > 0 && (
          <div className="run-table">
            <div className="run-table-head" aria-hidden="true">
              <span>実行ID</span>
              <span>観測状態</span>
              <span>更新日時</span>
              <span aria-hidden="true" />
            </div>
            {recentRuns.map((run) => (
              <a
                className="run-row"
                href={withBasePath(routePath("run", run.id))}
                key={run.id}
              >
                <strong>{run.id}</strong>
                <span className={`status-badge ${statusTone(run.status)}`}>{run.status}</span>
                <time>{dateLabel(run.modified_epoch_seconds)}</time>
                <span aria-hidden="true" className="row-arrow">↗</span>
              </a>
            ))}
          </div>
        )}
      </section>
    </Shell>
  );
}
