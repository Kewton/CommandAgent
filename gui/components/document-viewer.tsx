import type { DocumentRecord } from "../lib/types";

export function DocumentViewer({ document, empty }: { document: DocumentRecord | null; empty: string }) {
  if (document === null) {
    return <div className="document-empty">{empty}</div>;
  }
  return (
    <article className="document-viewer">
      <header>
        <div>
          <span>READ-ONLY DOCUMENT</span>
          <h2>{document.id}</h2>
        </div>
        <code>{document.path}</code>
      </header>
      <pre>{document.content}</pre>
    </article>
  );
}
