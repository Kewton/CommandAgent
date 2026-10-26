"use client";

import { useEffect, useState } from "react";

// Synthetic fixture: a contract-mode search page whose only persisted value is
// the search term the user typed. The term survives a reload and is shown in the
// results heading, but nothing is committed, so it is a draft copy and not
// evidence that a change persisted.
export default function Page() {
  const [search, setSearch] = useState("");
  const items = ["alpha", "beta", "gamma", "delta", "epsilon"];
  const visible = items.filter((item) => item.includes(search));

  useEffect(() => {
    setSearch(window.localStorage.getItem("search") ?? "");
  }, []);

  return (
    <main>
      <div data-anvil-state={JSON.stringify({ search, visibleCount: visible.length })}>
        <input
          value={search}
          onChange={(event) => {
            setSearch(event.target.value);
            window.localStorage.setItem("search", event.target.value);
          }}
        />
        <button data-anvil-action="primary" onClick={() => {}}>
          Search
        </button>
        <h2>Results for {search}</h2>
        <ul>
          {visible.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </div>
    </main>
  );
}
