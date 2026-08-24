"use client";

import { useId, useState } from "react";

import type { DocumentRecord } from "../lib/types";

type DocumentViewerProps = {
  document: DocumentRecord | null;
  empty: string;
  headingLevel?: 2 | 3;
  sourceHref?: string | null;
};

export function DocumentViewer({
  document,
  empty,
  headingLevel = 2,
  sourceHref = null,
}: DocumentViewerProps) {
  const [wrapLines, setWrapLines] = useState(true);
  const contentId = useId();
  const Heading = headingLevel === 2 ? "h2" : "h3";

  if (document === null) {
    return <div className="document-empty" data-testid="document-empty">{empty}</div>;
  }
  return (
    <article className="document-viewer">
      <header>
        <div>
          <span>読み取り専用文書</span>
          <Heading style={{ margin: "0.3rem 0 0", fontSize: "1rem", fontWeight: 650 }}>
            {document.id}
          </Heading>
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
