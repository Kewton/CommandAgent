"use client";

import { useEffect, useState } from "react";

import { DocumentViewer } from "../../components/document-viewer";
import { Shell } from "../../components/shell";
import { EmptyState, ErrorState, LoadingState } from "../../components/states";
import { apiPath } from "../../lib/base-path";
import type { DocumentRecord, DocumentSummary } from "../../lib/types";
import { useResource } from "../../lib/use-resource";

export default function MeasurementsPage() {
  const reports = useResource<DocumentSummary[]>("reports");
  const scoreTimeMapPath = apiPath("maps/score-time.svg");
  const [selected, setSelected] = useState<DocumentRecord | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function readReport(path: string, signal?: AbortSignal) {
    setLoading(true);
    setError(null);
    try {
      const query = new URLSearchParams({ path });
      const response = await fetch(apiPath("reports/view", query), { signal });
      if (!response.ok) throw new Error(await response.text());
      setSelected((await response.json()) as DocumentRecord);
    } catch (reason) {
      if (reason instanceof DOMException && reason.name === "AbortError") return;
      setError(reason instanceof Error ? reason.message : "Unable to read report");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    const first = reports.data?.at(0);
    if (first === undefined) return;
    const controller = new AbortController();
    void readReport(first.path, controller.signal);
    return () => controller.abort();
  }, [reports.data]);

  return (
    <Shell
      active="measurements"
      eyebrow="04 / MEASUREMENT ARCHIVE"
      title="Claims need coordinates."
      description="Browse score/time geometry and the reports that explain it. Values are displayed from existing evidence only."
    >
      <section className="measure-map panel">
        <div>
          <span className="panel-index">REFERENCE MAP</span>
          <h2>Attainment × configuration time</h2>
          <p>
            Every mark is backed by a repository measurement row. On a narrow screen, scroll the map
            horizontally or open the full-size SVG to zoom and inspect source details.
          </p>
          <a className="map-source-link" href={scoreTimeMapPath} rel="noreferrer" target="_blank">
            Open full-size SVG ↗
          </a>
        </div>
        <div
          aria-label="Scrollable score and time map"
          className="map-frame compact"
          data-testid="measurement-map-frame"
          role="region"
          tabIndex={0}
        >
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            data-testid="measurement-score-time-map"
            src={scoreTimeMapPath}
            alt="Score and time measurement map"
          />
        </div>
      </section>

      <section className="measurement-workbench">
        <aside className="report-index panel">
          <header>
            <span>REPORT INDEX</span>
            <strong>{reports.data?.length ?? "—"}</strong>
          </header>
          {reports.loading && <LoadingState label="Indexing measurement reports" />}
          {reports.error !== null && <ErrorState message={reports.error} />}
          {reports.data?.length === 0 && <EmptyState message="No reports were found." />}
          <div className="report-list">
            {reports.data?.map((report) => (
              <button
                className={selected?.path === report.path ? "active" : ""}
                key={report.path}
                onClick={() => void readReport(report.path)}
                type="button"
              >
                <strong>{report.id}</strong>
                <small>{report.path}</small>
              </button>
            ))}
          </div>
        </aside>
        <div className="report-document">
          {loading && <LoadingState label="Reading measurement report" />}
          {error !== null && <ErrorState message={error} />}
          {!loading && error === null && (
            <DocumentViewer document={selected} empty="Select a report from the archive." />
          )}
        </div>
      </section>
    </Shell>
  );
}
