"use client";

import { useEffect, useState } from "react";

type Note = { id: string; title: string };

export default function Home() {
  const [notes, setNotes] = useState<Note[]>([]);

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
      <button data-anvil-action="primary" onClick={create}>
        New note
      </button>
      <ul>
        {notes.map((note) => (
          <li key={note.id}>{note.title}</li>
        ))}
      </ul>
    </main>
  );
}
