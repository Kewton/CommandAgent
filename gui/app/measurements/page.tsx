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
      setError(reason instanceof Error ? reason.message : "レポートを読み込めませんでした");
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
      title="計測"
      description="スコアと所要時間の分布、根拠となる既存レポートを確認します。"
    >
      <section className="measure-map panel">
        <div>
          <span className="panel-index">参照マップ</span>
          <h2>到達度 × 構成時間</h2>
          <p>各点はリポジトリの計測行に対応します。SVG の点にカーソルを合わせると出典を確認できます。</p>
        </div>
        <div className="map-frame compact">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src={apiPath("maps/score-time.svg")} alt="スコアと時間の計測マップ" />
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
            <DocumentViewer document={selected} empty="一覧からレポートを選択してください。" />
          )}
        </div>
      </section>
    </Shell>
  );
}
