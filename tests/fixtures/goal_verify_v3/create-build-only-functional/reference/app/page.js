"use client";

import { useState } from "react";

export default function Page() {
  const [count, setCount] = useState(0);
  return (
    <main>
      <button id="increment" type="button" onClick={() => setCount((value) => value + 1)}>
        Increment
      </button>
      <span id="count">{count}</span>
    </main>
  );
}
