"use client";

import { useEffect, useMemo, useState } from "react";

import { DocumentViewer } from "../../components/document-viewer";
import { Shell } from "../../components/shell";
import { EmptyState, ErrorState, LoadingState } from "../../components/states";
import { apiPath, routePath, withBasePath } from "../../lib/base-path";
import { describeError, responseError } from "../../lib/errors";
import type { DocumentRecord, RunDetail, RunIndex } from "../../lib/types";
import { useResource } from "../../lib/use-resource";

function byteLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KiB`;
}

function dateLabel(epochSeconds: number): string {
  if (epochSeconds === 0) return "時刻不明";
  return new Intl.DateTimeFormat("ja-JP", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(epochSeconds * 1000));
}

export default function RunDetailPage() {
  const runs = useResource<RunIndex>("runs");
  const [runId, setRunId] = useState("");
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [selected, setSelected] = useState<DocumentRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState("");

  useEffect(() => {
    const requested = new URLSearchParams(window.location.search).get("id");
    if (requested !== null) setRunId(requested);
  }, []);

  useEffect(() => {
    if (runId === "") return;
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    setSelected(null);
    fetch(apiPath(`runs/${encodeURIComponent(runId)}`), { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) throw await responseError(response);
        return response.json() as Promise<RunDetail>;
      })
      .then((value) => setDetail(value))
      .catch((reason: unknown) => {
        if (reason instanceof DOMException && reason.name === "AbortError") return;
        setError(describeError(reason));
      })
      .finally(() => setLoading(false));
    return () => controller.abort();
  }, [runId]);

  const acceptance = useMemo<DocumentRecord | null>(() => {
    if (detail === null) return null;
    return {
      id: detail.acceptance_path?.split("/").at(-1) ?? "acceptance",
      path: detail.acceptance_path ?? detail.id,
      content: detail.acceptance,
    };
  }, [detail]);

  const filteredRuns = useMemo(() => {
    const available = runs.data?.runs ?? [];
    const query = filter.trim().toLocaleLowerCase("ja-JP");
    if (query === "") return available;
    return available.filter((run) =>
      [run.id, dateLabel(run.modified_epoch_seconds), run.status_text, run.state]
        .join(" ")
        .toLocaleLowerCase("ja-JP")
        .includes(query),
    );
  }, [filter, runs.data]);

  const documentSourceHref = useMemo(() => {
    if (runId === "" || acceptance === null) return null;
    if (selected === null) return apiPath(`runs/${encodeURIComponent(runId)}`);
    return apiPath(
      `runs/${encodeURIComponent(runId)}/evidence`,
      new URLSearchParams({ path: selected.path }),
    );
  }, [acceptance, runId, selected]);

  function chooseRun(id: string) {
    setRunId(id);
    window.history.replaceState(null, "", withBasePath(routePath("run", id)));
  }

  async function readEvidence(path: string) {
    if (runId === "") return;
    setLoading(true);
    setError(null);
    try {
      const query = new URLSearchParams({ path });
      const response = await fetch(apiPath(`runs/${encodeURIComponent(runId)}/evidence`, query));
      if (!response.ok) throw await responseError(response);
      setSelected((await response.json()) as DocumentRecord);
    } catch (reason) {
      setError(describeError(reason));
    } finally {
      setLoading(false);
    }
  }

  return (
    <Shell
      active="run"
      title="検証・運用レポート"
      description="repository に保存された検証・運用記録の受入シートと証跡を確認します。"
    >
      <section className="panel source-banner" data-testid="repository-run-source">
        <span className="panel-index">REPOSITORY / workspace/management/runs</span>
        <p>GUI Trial の execution root ではなく、repository 側の永続レポートを参照しています。</p>
      </section>
      <section className="run-workbench">
        <aside className="run-picker panel">
          <div className="run-filter">
            <label htmlFor="run-filter">実行を絞り込む</label>
            <input
              id="run-filter"
              onChange={(event) => setFilter(event.target.value)}
              placeholder="ID・日付・状態で検索"
              type="search"
              value={filter}
            />
          </div>
          <label htmlFor="run-select">実行ID・日付・状態</label>
          <select id="run-select" value={runId} onChange={(event) => chooseRun(event.target.value)}>
            <option value="">実行を選択…</option>
            {filteredRuns.map((run) => (
              <option key={run.id} value={run.id}>
                {dateLabel(run.modified_epoch_seconds)} — {run.status_text} — {run.id}
              </option>
            ))}
          </select>
          {runs.loading && <LoadingState label="実行一覧を読み込んでいます" />}
          {runs.error !== null && <ErrorState message={runs.error} />}
          {runs.data?.runs.length === 0 && (
            <EmptyState
              label="repository 記録なし"
              message="workspace/management/runs に検証・運用レポートがありません。"
            />
          )}
          {runs.data !== null && runs.data.runs.length > 0 && filteredRuns.length === 0 && (
            <EmptyState label="該当なし" message="条件に一致する実行がありません。" />
          )}
          {detail !== null && (
            <>
              <div className="picker-heading">
                <span>証跡ファイル</span>
                <strong>{detail.evidence.length}</strong>
              </div>
              <div className="evidence-list">
                <button
                  className={selected === null ? "active" : ""}
                  onClick={() => setSelected(null)}
                  type="button"
                >
                  <span>受入シート</span>
                  <small>{detail.acceptance_path ?? "投影"}</small>
                </button>
                {detail.evidence.map((item) => (
                  <button
                    className={selected?.path === item.path ? "active" : ""}
                    key={item.path}
                    onClick={() => void readEvidence(item.path)}
                    type="button"
                  >
                    <span>{item.id}</span>
                    <small>{byteLabel(item.size_bytes)}</small>
                  </button>
                ))}
              </div>
            </>
          )}
        </aside>
        <div className="run-document">
          {loading && <LoadingState label="変更不可の証跡を読み込んでいます" />}
          {error !== null && <ErrorState message={error} />}
          {!loading && error === null && runId === "" && (
            <EmptyState label="実行未選択" message="台帳から実行を選択してください。" />
          )}
          {!loading && error === null && runId !== "" && (
            <DocumentViewer
              document={selected ?? acceptance}
              empty="受入記録が見つかりません。"
              sourceHref={documentSourceHref}
            />
          )}
        </div>
      </section>
    </Shell>
  );
}
