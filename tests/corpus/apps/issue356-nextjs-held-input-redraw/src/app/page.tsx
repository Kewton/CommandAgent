"use client";

import { useEffect, useRef, useState } from "react";

export default function Page() {
  const keysRef = useRef(new Set<string>());
  const [snapshot, setSnapshot] = useState({ phase: "start", player_x: 300 });

  useEffect(() => {
    const timer = setInterval(() => {
      if (keysRef.current.has("ArrowLeft")) {
        setSnapshot((value) => ({ ...value, player_x: value.player_x - 6 }));
      }
    }, 16);
    return () => clearInterval(timer);
  }, []);

  const anvilState = JSON.stringify({
    phase: snapshot.phase,
    player_x: snapshot.player_x,
  });

  return <main data-anvil-state={anvilState}><canvas /></main>;
}
