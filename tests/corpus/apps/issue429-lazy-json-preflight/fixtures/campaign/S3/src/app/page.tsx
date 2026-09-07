"use client";

import { useEffect, useMemo, useState } from "react";

type Hazard = { id: number; x: number; y: number; alive: boolean };
type Pulse = { x: number; y: number };

const columns = 9;
const rows = 4;

function initialHazards(): Hazard[] {
  return Array.from({ length: columns * rows }, (_, id) => ({
    id,
    x: 8 + (id % columns) * 10,
    y: 12 + Math.floor(id / columns) * 8,
    alive: true,
  }));
}

export default function Page() {
  const [player, setPlayer] = useState(50);
  const [pulses, setPulses] = useState<Pulse[]>([]);
  const [hazards, setHazards] = useState<Hazard[]>(() => initialHazards());
  const [tick, setTick] = useState(0);
  const [running, setRunning] = useState(true);
  const [lives, setLives] = useState(3);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") setPlayer((value) => Math.max(5, value - 4));
      if (event.key === "ArrowRight") setPlayer((value) => Math.min(95, value + 4));
      if (event.key === " ") setPulses((value) => [...value, { x: player, y: 86 }].slice(-6));
      if (event.key.toLowerCase() === "r") {
        setHazards(initialHazards());
        setPulses([]);
        setRunning(true);
        setLives(3);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [player]);

  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => {
      setTick((value) => value + 1);
      setPulses((value) => value.map((pulse) => ({ ...pulse, y: pulse.y - 5 })).filter((pulse) => pulse.y > 4));
      setHazards((value) =>
        value.map((hazard) => ({
          ...hazard,
          x: hazard.x + Math.sin((tick + hazard.id) / 8) * 0.45,
          y: hazard.y + 0.035,
        })),
      );
    }, 70);
    return () => window.clearInterval(timer);
  }, [running, tick]);

  useEffect(() => {
    setHazards((current) =>
      current.map((hazard) => {
        if (!hazard.alive) return hazard;
        const hit = pulses.some((pulse) => Math.abs(pulse.x - hazard.x) < 3.2 && Math.abs(pulse.y - hazard.y) < 3.8);
        return hit ? { ...hazard, alive: false } : hazard;
      }),
    );
  }, [pulses]);

  const alive = hazards.filter((hazard) => hazard.alive).length;
  const score = useMemo(() => (columns * rows - alive) * 100, [alive]);

  useEffect(() => {
    const breach = hazards.some((hazard) => hazard.alive && hazard.y > 78);
    if (breach) setLives((value) => Math.max(0, value - 1));
    if (alive === 0 || breach || lives === 0) {
      setRunning(false);
    }
  }, [alive, hazards, lives]);

  return (
    <main className="screen">
      <section className="hud">
        <strong>INTERACTIVE CHALLENGE</strong>
        <span>SCORE {score}</span>
        <span>LIVES {lives}</span>
        <span>{running ? "LIVE" : alive === 0 ? "CLEAR" : "RESET READY"}</span>
      </section>
      <section className="arena" aria-label="Interactive challenge play field">
        <div className="stars" />
        {hazards.map((hazard) =>
          hazard.alive ? (
            <div
              className="hazard"
              key={hazard.id}
              style={{ left: `${hazard.x}%`, top: `${hazard.y}%` }}
            />
          ) : null,
        )}
        {pulses.map((pulse, index) => (
          <div className="pulse" key={`${pulse.x}-${pulse.y}-${index}`} style={{ left: `${pulse.x}%`, top: `${pulse.y}%` }} />
        ))}
        <div className="player" style={{ left: `${player}%` }} />
      </section>
      <nav className="controls">
        <button onClick={() => setPlayer((value) => Math.max(5, value - 5))}>Left</button>
        <button onClick={() => setPulses((value) => [...value, { x: player, y: 86 }].slice(-6))}>Action</button>
        <button onClick={() => setPlayer((value) => Math.min(95, value + 5))}>Right</button>
        <button
          onClick={() => {
            setHazards(initialHazards());
            setPulses([]);
            setRunning(true);
            setLives(3);
          }}
        >
          Reset
        </button>
      </nav>
      <style jsx>{`
        .screen {
          min-height: 100vh;
          padding: 24px;
          display: grid;
          grid-template-rows: auto 1fr auto;
          gap: 16px;
          background: #05070d;
          color: #edfaff;
          font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        }
        .hud, .controls {
          display: flex;
          justify-content: center;
          gap: 12px;
          flex-wrap: wrap;
        }
        .hud span, .hud strong, .controls button {
          border: 1px solid rgba(129, 245, 255, 0.45);
          background: rgba(5, 12, 24, 0.72);
          color: #effcff;
          padding: 10px 14px;
          border-radius: 6px;
          box-shadow: 0 0 18px rgba(0, 229, 255, 0.16);
        }
        .controls button { cursor: pointer; min-width: 84px; }
        .arena {
          position: relative;
          overflow: hidden;
          min-height: 560px;
          border: 1px solid rgba(129, 245, 255, 0.36);
          background: rgba(2, 4, 12, 0.84);
          box-shadow: inset 0 0 70px rgba(0, 229, 255, 0.12);
        }
        .stars {
          position: absolute;
          inset: 0;
          background-image: radial-gradient(#fff 1px, transparent 1px);
          background-size: 31px 29px;
          opacity: 0.2;
        }
        .hazard, .pulse, .player { position: absolute; transform: translate(-50%, -50%); }
        .hazard {
          width: 24px;
          height: 18px;
          background: #7dffbf;
          clip-path: polygon(12% 0, 88% 0, 100% 35%, 70% 35%, 70% 70%, 88% 70%, 88% 100%, 60% 78%, 40% 78%, 12% 100%, 12% 70%, 30% 70%, 30% 35%, 0 35%);
          filter: drop-shadow(0 0 12px #7dffbf);
        }
        .pulse {
          width: 4px;
          height: 20px;
          border-radius: 999px;
          background: #ffec7d;
          box-shadow: 0 0 14px #ffec7d;
        }
        .player {
          bottom: 28px;
          width: 46px;
          height: 30px;
          background: #7dc7ff;
          clip-path: polygon(50% 0, 100% 100%, 66% 82%, 34% 82%, 0 100%);
          filter: drop-shadow(0 0 16px #7dc7ff);
        }
      `}</style>
    </main>
  );
}
