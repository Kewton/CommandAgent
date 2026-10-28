"use client";

import { useEffect, useRef, useState } from "react";

type HighScoreSubmitResponse = {
  ok: boolean;
  ranks: number[];
  entry: { name: string; score: number; wave: number };
};

type GameState = { phase: "playing" | "gameover" | "victory"; score: number; wave: number };

export default function Page() {
  const [state] = useState<GameState>({ phase: "gameover", score: 0, wave: 1 });
  const [rank, setRank] = useState<number[]>([]);
  const [apiHighScore, setApiHighScore] = useState<number | null>(null);
  const submittedRef = useRef(false);

  // Submit once per finished run so the overlay can show the all-time best.
  useEffect(() => {
    const terminal = state.phase === "gameover" || state.phase === "victory";
    if (!terminal) {
      if (state.phase === "playing") submittedRef.current = false;
      return;
    }
    if (submittedRef.current) return;
    submittedRef.current = true;
    fetch("/api/highscore", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name: "PILOT", score: state.score, wave: state.wave }),
    })
      .then((res) =>
        res.ok ? (res.json() as Promise<HighScoreSubmitResponse>) : null,
      )
      .then((data) => {
        if (!data || data.ok !== true) return;
        setRank(data.ranks);
        setApiHighScore((prev) => Math.max(prev ?? 0, data.entry.score));
      })
      .catch(() => undefined);
  }, [state.phase, state.score, state.wave]);

  return (
    <main>
      <p>{apiHighScore ?? 0}</p>
      <p>{rank.join(",")}</p>
    </main>
  );
}
