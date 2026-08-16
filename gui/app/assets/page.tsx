"use client";

import { useState } from "react";

import { Shell } from "../../components/shell";
import { EmptyState, ErrorState, LoadingState } from "../../components/states";
import type { DocumentRecord, PackSummary } from "../../lib/types";
import { useResource } from "../../lib/use-resource";

type AssetTab = "packs" | "contracts" | "suites";

const assetTabLabels: Record<AssetTab, string> = {
  packs: "パック",
  contracts: "契約",
  suites: "計測スイート",
};

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
      title="アセット"
      description="登録済みのパック・契約・計測スイートを読み取り専用で確認します。"
    >
      <section className="asset-tabs" aria-label="アセット種別">
        {(["packs", "contracts", "suites"] as const).map((item, index) => (
          <button
            className={tab === item ? "active" : ""}
            key={item}
            onClick={() => setTab(item)}
            type="button"
          >
            <span>0{index + 1}</span>
            {assetTabLabels[item]}
          </button>
        ))}
      </section>

      {tab === "packs" && (
        <section className="asset-content">
          {packs.loading && <LoadingState label="パックの固定情報を読み込んでいます" />}
          {packs.error !== null && <ErrorState message={packs.error} />}
          {packs.data?.length === 0 && <EmptyState message="固定済みパックが見つかりません。" />}
          <div className="pack-grid">
            {packs.data?.map((pack) => (
              <article className="pack-card" key={`${pack.id}-${pack.version}`}>
                <header>
                  <span>登録済みパック</span>
                  <strong>{pack.version}</strong>
                </header>
                <h2>{pack.id}</h2>
                <p>{pack.path}</p>
                <div className="pin-block">
                  <span>完全一致の固定値</span>
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
          empty="契約文書が見つかりません。"
        />
      )}

      {tab === "suites" && (
        <DocumentCards
          documents={suites.data}
          error={suites.error}
          loading={suites.loading}
          empty="計測スイートが見つかりません。"
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
