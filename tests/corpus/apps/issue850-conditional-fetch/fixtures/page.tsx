"use client";

import { useCallback, useState } from "react";

type Note = { id: string; title: string; content: string; updatedAt: string };

export default function NotePage({ params }: { params: { id: string } }) {
  const id = params.id;
  const isNew = id === "new";
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const [lastSavedAt, setLastSavedAt] = useState<string | null>(null);

  const handleSave = useCallback(async () => {
    if (title.trim().length === 0) {
      setError("タイトルは必須です");
      return;
    }
    setError(null);
    setStatus("保存中…");
    try {
      const res = isNew
        ? await fetch("/api/notes", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ title, content }),
          })
        : await fetch(`/api/notes/${id}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ title, content }),
          });
      // Check Response.ok before accepting the mutation result.
      const body = (await res.json().catch(() => null)) as
        | { note?: Note; message?: string; error?: string }
        | null;
      if (!res.ok || !body || typeof body.note?.updatedAt !== "string") {
        setError(body?.message ?? body?.error ?? `保存に失敗しました (${res.status})`);
        setStatus("保存失敗");
        return;
      }
      setLastSavedAt(body.note.updatedAt);
      setStatus(`保存完了 ${new Date(body.note.updatedAt).toLocaleTimeString("ja-JP")}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存に失敗しました");
      setStatus("保存失敗");
    }
  }, [content, id, isNew, title]);

  return (
    <main>
      <input value={title} onChange={(event) => setTitle(event.target.value)} />
      <textarea value={content} onChange={(event) => setContent(event.target.value)} />
      <button onClick={handleSave}>保存</button>
      {error ? <p role="alert">{error}</p> : null}
      <p>{status}</p>
      {lastSavedAt ? <small>{lastSavedAt}</small> : null}
    </main>
  );
}
