"use client";

import { useState } from "react";

import { Shell } from "../../components/shell";
import { EmptyState, ErrorState, LoadingState } from "../../components/states";
import type { DocumentRecord, PackSummary } from "../../lib/types";
import { useResource } from "../../lib/use-resource";

type AssetTab = "packs" | "contracts" | "suites";

function assetTitle(document: DocumentRecord): string {
  return document.content.match(/^#\s+(.+)$/m)?.[1] ?? document.id;
}

export default function AssetsPage() {
  const [tab, setTab] = useState<AssetTab>("packs");
  const packs = useResource<PackSummary[]>("packs");
  const contracts = useResource<DocumentRecord[]>("contracts");
  const suites = useResource<DocumentRecord[]>("suites");

  return (
    <Shell
      active="assets"
      eyebrow="03 / ADMITTED ASSETS"
      title="Pinned means visible."
      description="Packs, contracts, and suites are read from their admitted repository bytes. This surface has no editor."
    >
      <section className="asset-tabs" aria-label="Asset category">
        {(["packs", "contracts", "suites"] as const).map((item, index) => (
          <button
            className={tab === item ? "active" : ""}
            key={item}
            onClick={() => setTab(item)}
            type="button"
          >
            <span>0{index + 1}</span>
            {item}
          </button>
        ))}
      </section>

      {tab === "packs" && (
        <section className="asset-content">
          {packs.loading && <LoadingState label="Reading pack pins" />}
          {packs.error !== null && <ErrorState message={packs.error} />}
          {packs.data?.length === 0 && <EmptyState message="No pinned packs were found." />}
          <div className="pack-grid">
            {packs.data?.map((pack) => (
              <article className="pack-card" key={`${pack.id}-${pack.version}`}>
                <header>
                  <span>ADMITTED PACK</span>
                  <strong>{pack.version}</strong>
                </header>
                <h2>{pack.id}</h2>
                <p>{pack.path}</p>
                <div className="pin-block">
                  <span>EXACT-BYTE PIN</span>
                  <code>{pack.pin}</code>
                </div>
                <footer>
                  <span className={pack.has_assist ? "present" : "absent"}>assist.yaml</span>
                  <span className={pack.has_eval ? "present" : "absent"}>eval.yaml</span>
                </footer>
              </article>
            ))}
          </div>
        </section>
      )}

      {tab === "contracts" && (
        <DocumentCards
          documents={contracts.data}
          error={contracts.error}
          loading={contracts.loading}
          empty="No contract documents were found."
        />
      )}

      {tab === "suites" && (
        <DocumentCards
          documents={suites.data}
          error={suites.error}
          loading={suites.loading}
          empty="No measurement suites were found."
        />
      )}
    </Shell>
  );
}

function DocumentCards({
  documents,
  error,
  loading,
  empty,
}: {
  documents: DocumentRecord[] | null;
  error: string | null;
  loading: boolean;
  empty: string;
}) {
  const [openPath, setOpenPath] = useState<string | null>(null);
  return (
    <section className="asset-content document-grid">
      {loading && <LoadingState />}
      {error !== null && <ErrorState message={error} />}
      {documents?.length === 0 && <EmptyState message={empty} />}
      {documents?.map((document) => {
        const open = document.path === openPath;
        return (
          <article className={open ? "document-card open" : "document-card"} key={document.path}>
            <button onClick={() => setOpenPath(open ? null : document.path)} type="button">
              <span>
                <small>{document.path}</small>
                <strong>{assetTitle(document)}</strong>
              </span>
              <i>{open ? "−" : "+"}</i>
            </button>
            {open && <pre>{document.content}</pre>}
          </article>
        );
      })}
    </section>
  );
}
