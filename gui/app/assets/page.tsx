"use client";

import { useState, type KeyboardEvent } from "react";

import { PackWizard } from "../../components/pack-wizard";
import { Shell } from "../../components/shell";
import { EmptyState, ErrorState, LoadingState } from "../../components/states";
import { routePath, withBasePath } from "../../lib/base-path";
import type { DocumentRecord, PackSummary } from "../../lib/types";
import { useResource } from "../../lib/use-resource";

type AssetTab = "packs" | "contracts" | "suites";
const assetTabs = ["packs", "contracts", "suites"] as const;

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
  const warningCount = packs.data?.filter((pack) => pack.warning !== null).length ?? 0;

  function handleTabKeyDown(event: KeyboardEvent<HTMLButtonElement>, current: AssetTab) {
    const currentIndex = assetTabs.indexOf(current);
    let nextIndex: number;
    switch (event.key) {
      case "ArrowRight":
        nextIndex = (currentIndex + 1) % assetTabs.length;
        break;
      case "ArrowLeft":
        nextIndex = (currentIndex - 1 + assetTabs.length) % assetTabs.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = assetTabs.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    const nextTab = assetTabs[nextIndex];
    setTab(nextTab);
    document.getElementById(`asset-tab-${nextTab}`)?.focus();
  }

  return (
    <Shell
      active="assets"
      title="拡張"
      description="pack の供給元、承認状態、exact-byte hash と pin を読み取り専用で確認します。"
    >
      <div className="asset-tabs" aria-label="アセット種別" role="tablist">
        {assetTabs.map((item, index) => (
          <button
            aria-controls={`asset-panel-${item}`}
            aria-selected={tab === item}
            className={tab === item ? "active" : ""}
            id={`asset-tab-${item}`}
            key={item}
            onClick={() => setTab(item)}
            onKeyDown={(event) => handleTabKeyDown(event, item)}
            role="tab"
            tabIndex={tab === item ? 0 : -1}
            type="button"
          >
            <span>0{index + 1}</span>
            {assetTabLabels[item]}
          </button>
        ))}
      </div>

      {tab === "packs" && (
        <section
          aria-labelledby="asset-tab-packs"
          className="asset-content"
          id="asset-panel-packs"
          role="tabpanel"
        >
          <PackWizard onCatalogChange={packs.refresh} />
          {packs.loading && <LoadingState label="パックの固定情報を読み込んでいます" />}
          {packs.error !== null && <ErrorState message={packs.error} />}
          {packs.data?.length === 0 && <EmptyState message="固定済みパックが見つかりません。" />}
          {warningCount > 0 && (
            <p className="pack-warning-status" data-testid="pack-warning-status" role="status">
              {warningCount} 件のパック警告があります。
            </p>
          )}
          <div className="pack-grid">
            {packs.data?.map((pack) => {
              const selector = `${pack.id}@${pack.version}`;
              return (
                <article
                  className={pack.warning === null ? "pack-card" : "pack-card warning"}
                  data-pack-source={pack.source}
                  data-testid="extension-pack-row"
                  key={selector}
                >
                  <header>
                    <span className={`pack-source source-${pack.source}`}>{pack.source_label}</span>
                    <strong>{selector}</strong>
                  </header>
                  <h2>{pack.id}</h2>
                  <p>{pack.path} · {pack.profile ?? "profile 不明"} × {pack.intent ?? "intent 不明"}</p>
                  <div className="pin-block">
                    <span>pin / 期待 hash</span>
                    <code>{pack.expected_hash ?? "未固定"}</code>
                    <span>観測 hash</span>
                    <code>{pack.observed_hash ?? "算出不可"}</code>
                  </div>
                  {pack.warning !== null && (
                    <p className="pack-warning" data-testid="pack-warning" role="note">
                      {pack.warning}
                    </p>
                  )}
                  <footer>
                    <PackMemberPresence name="assist.yaml" present={pack.has_assist} />
                    <PackMemberPresence name="eval.yaml" present={pack.has_eval} />
                    {pack.trial_eligible && pack.intent === "create" && (
                      <a
                        className="pack-trial-link"
                        data-testid="pack-trial-link"
                        href={withBasePath(`${routePath("try")}?pack=${encodeURIComponent(selector)}`)}
                      >
                        Trial で使う ↗
                      </a>
                    )}
                  </footer>
                </article>
              );
            })}
          </div>
        </section>
      )}

      {tab === "contracts" && (
        <DocumentCards
          documents={contracts.data}
          error={contracts.error}
          loading={contracts.loading}
          empty="契約文書が見つかりません。"
          tab="contracts"
        />
      )}

      {tab === "suites" && (
        <DocumentCards
          documents={suites.data}
          error={suites.error}
          loading={suites.loading}
          empty="計測スイートが見つかりません。"
          tab="suites"
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
  tab,
}: {
  documents: DocumentRecord[] | null;
  error: string | null;
  loading: boolean;
  empty: string;
  tab: Exclude<AssetTab, "packs">;
}) {
  const [openPath, setOpenPath] = useState<string | null>(null);
  return (
    <section
      aria-labelledby={`asset-tab-${tab}`}
      className="asset-content document-grid"
      id={`asset-panel-${tab}`}
      role="tabpanel"
    >
      {loading && <LoadingState />}
      {error !== null && <ErrorState message={error} />}
      {documents?.length === 0 && <EmptyState message={empty} />}
      {documents?.map((document, index) => {
        const open = document.path === openPath;
        const contentId = `document-content-${tab}-${index}`;
        return (
          <article className={open ? "document-card open" : "document-card"} key={document.path}>
            <button
              aria-controls={contentId}
              aria-expanded={open}
              onClick={() => setOpenPath(open ? null : document.path)}
              type="button"
            >
              <span>
                <small>{document.path}</small>
                <strong>{assetTitle(document)}</strong>
              </span>
              <i aria-hidden="true">{open ? "−" : "+"}</i>
            </button>
            {open && <pre id={contentId} tabIndex={0}>{document.content}</pre>}
          </article>
        );
      })}
    </section>
  );
}

function PackMemberPresence({ name, present }: { name: "assist.yaml" | "eval.yaml"; present: boolean }) {
  return (
    <span className={present ? "present" : "absent"}>
      <span aria-hidden="true" className="pack-member-icon">{present ? "✓" : "−"}</span>
      {" "}{name}: {present ? "あり" : "なし"}
    </span>
  );
}
