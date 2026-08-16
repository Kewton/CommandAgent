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
    ? window ?? "Formal measurement record"
    : `${denominator.trim()} admitted records`;
}

function shortBandName(document: DocumentRecord): string {
  return document.content.match(/^#\s+(.+)$/m)?.[1] ?? document.id;
}

function dateLabel(epochSeconds: number): string {
  if (epochSeconds === 0) return "time unavailable";
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
      eyebrow="01 / SYSTEM OVERVIEW"
      title="Evidence, at a glance."
      description="Repository truth projected into one quiet surface. Trial runs cross the existing confirmed CLI boundary; observation remains file-backed."
    >
      <section className="metric-strip" aria-label="Repository summary">
        <div>
          <span>Visible runs</span>
          <strong>{runs.data?.length ?? "—"}</strong>
        </div>
        <div>
          <span>Recent positive</span>
          <strong>{runs.data === null ? "—" : `${completed}/${recentRuns.length}`}</strong>
        </div>
        <div>
          <span>Formal bands</span>
          <strong>{bands.data?.length ?? "—"}</strong>
        </div>
        <div>
          <span>Execution surface</span>
          <strong className="accent-text">CLI ONLY</strong>
        </div>
      </section>

      <section className="dashboard-grid">
        <article className="panel map-panel">
          <header className="panel-heading">
            <div>
              <span className="panel-index">A / SCORE × TIME</span>
              <h2>Capability map</h2>
            </div>
            <a href={withBasePath(routePath("measurements"))}>Inspect measures ↗</a>
          </header>
          <div className="map-frame">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img
              data-testid="score-time-map"
              src={apiPath("maps/score-time.svg")}
              alt="CommandAgent score and execution-time capability map"
            />
          </div>
        </article>

        <article className="panel bands-panel">
          <header className="panel-heading">
            <div>
              <span className="panel-index">B / FORMAL BANDS</span>
              <h2>Measured boundaries</h2>
            </div>
          </header>
          {bands.loading && <LoadingState label="Reading band summaries" />}
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

      <section className="panel runs-panel">
        <header className="panel-heading">
          <div>
            <span className="panel-index">C / RUN LEDGER</span>
            <h2>Most recently touched records</h2>
          </div>
          <span className="live-label"><i /> filesystem projection</span>
        </header>
        {runs.loading && <LoadingState label="Indexing run records" />}
        {runs.error !== null && <ErrorState message={runs.error} />}
        {runs.data?.length === 0 && <EmptyState message="No run directories were found." />}
        {recentRuns.length > 0 && (
          <div className="run-table">
            <div className="run-table-head" aria-hidden="true">
              <span>Run identity</span>
              <span>Observed state</span>
              <span>Modified</span>
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
