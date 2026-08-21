"use client";

import { useEffect, useState } from "react";

import { DocumentViewer } from "../../components/document-viewer";
import { Shell } from "../../components/shell";
import { EmptyState, ErrorState, LoadingState } from "../../components/states";
import { apiPath } from "../../lib/base-path";
import { describeError, responseError } from "../../lib/errors";
import type { DocumentRecord, DocumentSummary } from "../../lib/types";
import { useResource } from "../../lib/use-resource";

export default function MeasurementsPage() {
  const reports = useResource<DocumentSummary[]>("reports");
  const scoreTimeMapPath = apiPath("maps/score-time.svg");
  const [selected, setSelected] = useState<DocumentRecord | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const selectedPath = selected?.path ?? null;

  async function readReport(path: string, signal?: AbortSignal) {
    setLoading(true);
    setError(null);
    try {
      const query = new URLSearchParams({ path });
      const response = await fetch(apiPath("reports/view", query), { signal });
      if (!response.ok) throw await responseError(response);
      setSelected((await response.json()) as DocumentRecord);
    } catch (reason) {
      if (reason instanceof DOMException && reason.name === "AbortError") return;
      setError(describeError(reason));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (reports.data?.some((report) => report.path === selectedPath)) return;
    const first = reports.data?.at(0);
    if (first === undefined) return;
    const controller = new AbortController();
    void readReport(first.path, controller.signal);
    return () => controller.abort();
  }, [reports.data, selectedPath]);

  return (
    <Shell
      active="measurements"
      title="計測"
      description="スコアと所要時間の分布、根拠となる既存レポートを確認します。"
    >
      <section className="measure-map panel">
        <div>
          <span className="panel-index">参照マップ</span>
          <h2>到達度 × 構成時間</h2>
          <p>
            各点はリポジトリの計測行に対応します。画面が狭い場合は横にスクロールするか、原寸 SVG
            を開いて拡大し、出典を確認できます。
          </p>
          <a className="map-source-link" href={scoreTimeMapPath} rel="noreferrer" target="_blank">
            原寸 SVG を開く ↗
          </a>
        </div>
        <div
          aria-label="横スクロールできるスコアと時間の計測マップ"
          className="map-frame compact"
          data-testid="measurement-map-frame"
          role="region"
          tabIndex={0}
        >
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            data-testid="measurement-score-time-map"
            src={scoreTimeMapPath}
            alt="スコアと時間の計測マップ"
          />
        </div>
      </section>

      <section className="measurement-workbench">
        <aside className="report-index panel">
          <header>
            <span>レポート一覧</span>
            <strong>{reports.data?.length ?? "—"}</strong>
          </header>
          {reports.loading && <LoadingState label="計測レポートを索引化しています" />}
          {reports.error !== null && <ErrorState message={reports.error} />}
          {reports.data?.length === 0 && <EmptyState message="レポートが見つかりません。" />}
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
          {loading && <LoadingState label="計測レポートを読み込んでいます" />}
          {error !== null && <ErrorState message={error} />}
          {!loading && error === null && (
            <DocumentViewer
              document={selected}
              empty="一覧からレポートを選択してください。"
              sourceHref={
                selected === null
                  ? null
                  : apiPath("reports/view", new URLSearchParams({ path: selected.path }))
              }
            />
          )}
        </div>
      </section>
    </Shell>
  );
}
