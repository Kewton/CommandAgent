"use client";

import { useState } from "react";

type Bookmark = { id: string; title: string };

export default function BookmarksPage() {
  const [items, setItems] = useState<Bookmark[]>([]);
  const [notice, setNotice] = useState("");

  async function save(title: string, isNew: boolean) {
    const body = JSON.stringify({ title });
    /* CASE */
  }

  return (
    <main>
      <button onClick={() => void save("docs", items.length === 0)}>保存</button>
      <p>{notice}</p>
      <ul>
        {items.map((item) => (
          <li key={item.id}>{item.title}</li>
        ))}
      </ul>
    </main>
  );
}
