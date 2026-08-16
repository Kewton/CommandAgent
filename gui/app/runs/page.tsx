"use client";

import { useEffect, useMemo, useState } from "react";

import { DocumentViewer } from "../../components/document-viewer";
import { Shell } from "../../components/shell";
import { EmptyState, ErrorState, LoadingState } from "../../components/states";
import { apiPath, routePath, withBasePath } from "../../lib/base-path";
import { describeError, responseError } from "../../lib/errors";
import type { DocumentRecord, RunDetail, RunSummary } from "../../lib/types";
import { useResource } from "../../lib/use-resource";

function byteLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KiB`;
}

export default function RunDetailPage() {
  const runs = useResource<RunSummary[]>("runs");
  const [runId, setRunId] = useState("");
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [selected, setSelected] = useState<DocumentRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

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
      eyebrow="02 / ACCEPTANCE LEDGER"
      title="One run. Every receipt."
      description="Select a run to inspect its acceptance sheet and bounded evidence inventory exactly as recorded."
    >
      <section className="run-workbench">
        <aside className="run-picker panel">
          <label htmlFor="run-select">RUN IDENTITY</label>
          <select id="run-select" value={runId} onChange={(event) => chooseRun(event.target.value)}>
            <option value="">Choose a run…</option>
            {runs.data?.map((run) => (
              <option key={run.id} value={run.id}>
                {run.id}
              </option>
            ))}
          </select>
          {runs.loading && <LoadingState label="Reading run index" />}
          {runs.error !== null && <ErrorState message={runs.error} />}
          {detail !== null && (
            <>
              <div className="picker-heading">
                <span>EVIDENCE FILES</span>
                <strong>{detail.evidence.length}</strong>
              </div>
              <div className="evidence-list">
                <button
                  className={selected === null ? "active" : ""}
                  onClick={() => setSelected(null)}
                  type="button"
                >
                  <span>Acceptance sheet</span>
                  <small>{detail.acceptance_path ?? "projection"}</small>
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
          {loading && <LoadingState label="Reading immutable evidence" />}
          {error !== null && <ErrorState message={error} />}
          {!loading && error === null && runId === "" && (
            <EmptyState message="Choose a run from the ledger to begin inspection." />
          )}
          {!loading && error === null && runId !== "" && (
            <DocumentViewer document={selected ?? acceptance} empty="No acceptance record was found." />
          )}
        </div>
      </section>
    </Shell>
  );
}
