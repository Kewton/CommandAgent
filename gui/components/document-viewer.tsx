"use client";

import { useId, useState } from "react";

import type { DocumentRecord } from "../lib/types";

type DocumentViewerProps = {
  document: DocumentRecord | null;
  empty: string;
  sourceHref?: string | null;
};

export function DocumentViewer({ document, empty, sourceHref = null }: DocumentViewerProps) {
  const [wrapLines, setWrapLines] = useState(true);
  const contentId = useId();

  if (document === null) {
    return <div className="document-empty" data-testid="document-empty">{empty}</div>;
  }
  return (
    <article className="document-viewer">
      <header>
        <div>
          <span>読み取り専用文書</span>
          <h2>{document.id}</h2>
        </div>
        <div className="document-viewer-actions">
          <code>{document.path}</code>
          {sourceHref !== null && (
            <a
              className="document-source-link"
              data-testid="document-source-link"
              href={sourceHref}
              rel="noreferrer"
              target="_blank"
            >
              元の GET を開く ↗
            </a>
          )}
          <button
            aria-controls={contentId}
            aria-pressed={wrapLines}
            className="document-wrap-toggle"
            data-testid="document-wrap-toggle"
            onClick={() => setWrapLines((current) => !current)}
            type="button"
          >
            折り返し: {wrapLines ? "有効" : "無効"}
          </button>
        </div>
      </header>
      <pre
        className={wrapLines ? "document-content--wrapped" : "document-content--unwrapped"}
        data-testid="document-content"
        id={contentId}
      >
        {document.content}
      </pre>
    </article>
  );
}
