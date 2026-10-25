"use client";

import { useEffect, useState } from "react";

type Note = { id: string; title: string };

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
            <li key={note.id}>{note.title}</li>
          ))}
      </ul>
    </main>
  );
}
