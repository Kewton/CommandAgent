"use client";

import { useEffect, useState } from "react";

type Note = { id: string; title: string };

// Markdown links become external anchors; `$2` is a replacement reference.
function renderLinks(text: string) {
  return text.replace(
    /\[([^\]]+)\]\((https?:\/\/[^)]+)\)/g,
    '<a href="$2" target="_blank" rel="noreferrer">$1</a>',
  );
}

export default function Home() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [search, setSearch] = useState("");

  useEffect(() => {
    fetch("/api/notes")
      .then((response) => response.json())
      .then(setNotes);
  }, []);

  async function create() {
    const response = await fetch("/api/notes", { method: "POST" });
    setNotes([...notes, await response.json()]);
  }

  return (
    <main>
      <input
        data-anvil-action="input"
        value={search}
        onChange={(event) => setSearch(event.target.value)}
      />
      <button data-anvil-action="primary" onClick={create}>
        New note
      </button>
      <ul>
        {notes
          .filter((note) => note.title.includes(search))
          .map((note) => (
            <li key={note.id}>
              <span dangerouslySetInnerHTML={{ __html: renderLinks(note.title) }} />
            </li>
          ))}
      </ul>
    </main>
  );
}
