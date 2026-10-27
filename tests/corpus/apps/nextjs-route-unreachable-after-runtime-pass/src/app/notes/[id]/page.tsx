"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useEffect, useState } from "react";

export default function NoteDetail() {
  const { id } = useParams<{ id: string }>();
  const [body, setBody] = useState("");

  useEffect(() => {
    const saved = JSON.parse(window.localStorage.getItem("notes") ?? "[]");
    const note = saved.find((candidate: { id: string }) => candidate.id === id);
    setBody(note?.body ?? "");
  }, [id]);

  return (
    <main>
      <textarea
        data-anvil-action="input"
        value={body}
        onChange={(event) => setBody(event.target.value)}
      />
      <Link data-anvil-action="primary" href="/">
        Back
      </Link>
    </main>
  );
}
