"use client";

import { useState } from "react";

// Synthetic fixture: a contract-mode app whose primary press and the four
// ranked fallback buttons leave the exported state untouched. Only a small,
// lower-ranked button changes it.
export default function Page() {
  const [status, setStatus] = useState("menu");
  const [pos, setPos] = useState(0);

  return (
    <main>
      <div
        data-anvil-state={JSON.stringify({ status, pos })}
        onKeyDown={(event) => {
          if (event.key === "ArrowLeft" && status === "playing") setPos(pos - 1);
        }}
      >
        <button data-anvil-action="primary" onClick={() => {}}>
          Start
        </button>
        <button onClick={() => {}}>Tap</button>
        <button onClick={() => {}}>Tap</button>
        <button onClick={() => {}}>Tap</button>
        <button onClick={() => {}}>Tap</button>
        <button style={{ fontSize: 8 }} onClick={() => setStatus("playing")}>
          Go
        </button>
      </div>
    </main>
  );
}
