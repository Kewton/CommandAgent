"use client";

import { useEffect, useState } from "react";

type Note = { id: string; title: string; body: string };

export default function Home() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    const saved = window.localStorage.getItem("notes");
    if (saved) {
      setNotes(JSON.parse(saved));
    }
  }, []);

  function save(next: Note[]) {
    setNotes(next);
    window.localStorage.setItem("notes", JSON.stringify(next));
  }

  return (
    <main>
      <textarea
        data-anvil-action="input"
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
      />
      <pre>{draft}</pre>
      <button
        data-anvil-action="primary"
        onClick={() => {
          save([...notes, { id: String(Date.now()), title: draft, body: draft }]);
          setDraft("");
        }}
      >
        Save
      </button>
      <ul>
        {notes.map((note) => (
          <li key={note.id}>
            {note.title}
            <button
              data-anvil-action="delete"
              onClick={() => save(notes.filter((other) => other.id !== note.id))}
            >
              Delete
            </button>
          </li>
        ))}
      </ul>
    </main>
  );
}
