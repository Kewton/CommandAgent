"use client";

import { useEffect, useMemo, useRef, useState } from "react";

import { DocumentViewer } from "../../components/document-viewer";
import { Shell } from "../../components/shell";
import { EmptyState, ErrorState, LoadingState } from "../../components/states";
import { apiPath, routePath, withBasePath } from "../../lib/base-path";
import { describeError, responseError } from "../../lib/errors";
import { byteLabel, dateTimeLabel } from "../../lib/format";
import type { DocumentRecord, RunDetail, RunIndex } from "../../lib/types";
import { useResource } from "../../lib/use-resource";

type RunOwned<T> = {
  runId: string;
  value: T;
};

export default function RunDetailPage() {
  const runs = useResource<RunIndex>("runs");
  const [runId, setRunId] = useState("");
  const [loadedDetail, setLoadedDetail] = useState<RunOwned<RunDetail> | null>(null);
  const [selectedEvidence, setSelectedEvidence] = useState<RunOwned<DocumentRecord> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState("");
  const requestVersion = useRef(0);
  const evidenceController = useRef<AbortController | null>(null);

  const detail = loadedDetail?.runId === runId ? loadedDetail.value : null;
  const selected = selectedEvidence?.runId === runId ? selectedEvidence.value : null;

  useEffect(() => {
    const requested = new URLSearchParams(window.location.search).get("id");
    if (requested !== null) setRunId(requested);
  }, []);

  useEffect(() => {
    const version = ++requestVersion.current;
    evidenceController.current?.abort();
    evidenceController.current = null;
    setLoadedDetail(null);
    setSelectedEvidence(null);
    setError(null);
    if (runId === "") {
      setLoading(false);
      return;
    }
    const controller = new AbortController();
    setLoading(true);
    fetch(apiPath(`runs/${encodeURIComponent(runId)}`), { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) throw await responseError(response);
        return response.json() as Promise<RunDetail>;
      })
      .then((value) => {
        if (requestVersion.current === version) setLoadedDetail({ runId, value });
      })
      .catch((reason: unknown) => {
        if (requestVersion.current !== version) return;
        if (reason instanceof DOMException && reason.name === "AbortError") return;
        setError(describeError(reason));
      })
      .finally(() => {
        if (requestVersion.current === version) setLoading(false);
      });
    return () => {
      controller.abort();
      if (requestVersion.current === version) requestVersion.current += 1;
    };
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
      [run.id, dateTimeLabel(run.modified_epoch_seconds, "時刻不明"), run.status_text, run.state]
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
    requestVersion.current += 1;
    evidenceController.current?.abort();
    evidenceController.current = null;
    setLoadedDetail(null);
    setSelectedEvidence(null);
    setError(null);
    setLoading(id !== "");
    setRunId(id);
    window.history.replaceState(null, "", withBasePath(routePath("run", id)));
  }

  function showAcceptance() {
    requestVersion.current += 1;
    evidenceController.current?.abort();
    evidenceController.current = null;
    setSelectedEvidence(null);
    setError(null);
    setLoading(false);
  }

  async function readEvidence(path: string) {
    if (runId === "") return;
    const requestedRunId = runId;
    const version = ++requestVersion.current;
    evidenceController.current?.abort();
    const controller = new AbortController();
    evidenceController.current = controller;
    setLoading(true);
    setError(null);
    try {
      const query = new URLSearchParams({ path });
      const response = await fetch(apiPath(`runs/${encodeURIComponent(requestedRunId)}/evidence`, query), {
        signal: controller.signal,
      });
      if (!response.ok) throw await responseError(response);
      const value = (await response.json()) as DocumentRecord;
      if (requestVersion.current === version) {
        setSelectedEvidence({ runId: requestedRunId, value });
      }
    } catch (reason: unknown) {
      if (requestVersion.current !== version) return;
      if (reason instanceof DOMException && reason.name === "AbortError") return;
      setError(describeError(reason));
    } finally {
      if (requestVersion.current === version) setLoading(false);
      if (evidenceController.current === controller) evidenceController.current = null;
    }
  }

  return (
    <Shell
      active="run"
      title="リポジトリ実行記録"
      description="repository に保存された実行の受入シートと証跡を確認します。"
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
                {dateTimeLabel(run.modified_epoch_seconds, "時刻不明")} — {run.status_text} — {run.id}
              </option>
            ))}
          </select>
          {runs.loading && <LoadingState label="実行一覧を読み込んでいます" />}
          {runs.error !== null && <ErrorState message={runs.error} />}
          {runs.data?.runs.length === 0 && (
            <EmptyState
              label="リポジトリ実行記録なし"
              message="workspace/management/runs にリポジトリ実行記録がありません。"
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
                  onClick={showAcceptance}
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
