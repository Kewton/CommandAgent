"use client";

import { useId, useState } from "react";

import type { DocumentRecord } from "../lib/types";

export function DocumentViewer({ document, empty }: { document: DocumentRecord | null; empty: string }) {
  const [wrapLines, setWrapLines] = useState(true);
  const contentId = useId();

  if (document === null) {
    return <div className="document-empty" data-testid="document-empty">{empty}</div>;
  }
  return (
    <article className="document-viewer">
      <header>
        <div>
          <span>READ-ONLY DOCUMENT</span>
          <h2>{document.id}</h2>
        </div>
        <div className="document-viewer-actions">
          <code>{document.path}</code>
          <button
            aria-controls={contentId}
            aria-pressed={wrapLines}
            className="document-wrap-toggle"
            data-testid="document-wrap-toggle"
            onClick={() => setWrapLines((current) => !current)}
            type="button"
          >
            Wrap lines: {wrapLines ? "on" : "off"}
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
