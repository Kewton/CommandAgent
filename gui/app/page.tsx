"use client";

import { Shell } from "../components/shell";
import { GettingStarted } from "../components/getting-started";
import { EmptyState, ErrorState, LoadingState } from "../components/states";
import { apiPath, routePath, withBasePath } from "../lib/base-path";
import { dateLabel } from "../lib/format";
import type { DocumentRecord, RunIndex, RunState } from "../lib/types";
import { useResource } from "../lib/use-resource";

function statusTone(state: RunState): string {
  if (state === "pass") return "positive";
  if (state === "fail") return "negative";
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

export default function DashboardPage() {
  const runs = useResource<RunIndex>("runs");
  const bands = useResource<DocumentRecord[]>("bands");
  const recentRuns = runs.data?.runs.slice(0, 8) ?? [];

  return (
    <Shell
      active="dashboard"
      title="概要"
      description="repository に記録された検証・運用レポート、計測、固定アセットを確認します。"
    >
      <GettingStarted />

      <section className="metric-strip" aria-label="リポジトリ概要">
        <div>
          <span>表示件数 / 総数</span>
          <strong data-testid="run-count">
            {runs.data === null ? "—" : `${recentRuns.length} / ${runs.data.total}`}
          </strong>
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
          <span className="panel-index">拡張カタログ</span>
          <h2>パック・契約・計測スイート</h2>
          <p>pack の供給元、承認状態、exact-byte pin を読み取り専用で確認できます。</p>
        </div>
        <a data-testid="assets-link" href={withBasePath(routePath("assets"))}>拡張を開く ↗</a>
      </section>

      <section className="panel runs-panel">
        <header className="panel-heading">
          <div>
            <span className="panel-index">C / 検証・運用レポート</span>
            <h2>repository の最近の記録</h2>
            <p className="source-note" data-testid="repository-run-source">
              参照元: workspace/management/runs
            </p>
          </div>
          <span className="live-label"><i /> ファイル投影</span>
        </header>
        {runs.loading && <LoadingState label="実行記録を索引化しています" />}
        {runs.error !== null && <ErrorState message={runs.error} />}
        {runs.data?.runs.length === 0 && <EmptyState message="実行ディレクトリが見つかりません。" />}
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
                <span className={`status-badge ${statusTone(run.state)}`}>{run.status_text}</span>
                <time>{dateLabel(run.modified_epoch_seconds, "時刻未記録")}</time>
                <span aria-hidden="true" className="row-arrow">↗</span>
              </a>
            ))}
          </div>
        )}
      </section>
    </Shell>
  );
}
