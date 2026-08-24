"use client";
import { useEffect, useRef, useState, useCallback } from "react";

// ─── Types ───────────────────────────────────────────────────────────────────
interface Invader {
  id: number;
  x: number;
  y: number;
  alive: boolean;
  type: number; // 0-2 for different invader types
}

interface Bullet {
  id: number;
  x: number;
  y: number;
  type: "player" | "enemy";
}

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  color: string;
}

type GameState = "idle" | "playing" | "gameOver" | "victory";

// ─── Constants ───────────────────────────────────────────────────────────────
const GAME_WIDTH = 600;
const GAME_HEIGHT = 500;
const PLAYER_WIDTH = 40;
const PLAYER_HEIGHT = 20;
const INVADER_SIZE = 24;
const BULLET_WIDTH = 4;
const BULLET_HEIGHT = 12;
const PLAYER_SPEED = 5;
const BULLET_SPEED = 7;
const INVADER_SPEED_BASE = 1;
const INVADER_DROP = 20;
const MAX_PLAYER_BULLETS = 3;

const INVADER_COLORS = ["#ff6b6b", "#ffd93d", "#6bcb77"];
const INVADER_EYES = ["#000", "#000", "#000"];

// ─── Helpers ─────────────────────────────────────────────────────────────────
function createInvaders(): Invader[] {
  const invaders: Invader[] = [];
  const rows = 5;
  const cols = 8;
  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col++) {
      invaders.push({
        id: invaders.length,
        x: 60 + col * 55,
        y: 40 + row * 40,
        alive: true,
        type: row % 3,
      });
    }
  }
  return invaders;
}

function createExplosion(x: number, y: number, color: string): Particle[] {
  const particles: Particle[] = [];
  for (let i = 0; i < 12; i++) {
    const angle = (Math.PI * 2 * i) / 12;
    const speed = 1 + Math.random() * 3;
    particles.push({
      x,
      y,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed,
      life: 30 + Math.random() * 20,
      color,
    });
  }
  return particles;
}

// ─── Main Component ──────────────────────────────────────────────────────────
export default function SpaceInvaders() {
  const [gameState, setGameState] = useState<GameState>("idle");
  const [score, setScore] = useState(0);
  const [lives, setLives] = useState(3);
  const [playerX, setPlayerX] = useState(GAME_WIDTH / 2 - PLAYER_WIDTH / 2);
  const [invaders, setInvaders] = useState<Invader[]>(createInvaders);
  const [bullets, setBullets] = useState<Bullet[]>([]);
  const [enemyBullets, setEnemyBullets] = useState<Bullet[]>([]);
  const [particles, setParticles] = useState<Particle[]>([]);
  const [highScore, setHighScore] = useState(0);
  const [level, setLevel] = useState(1);

  const keysRef = useRef<Set<string>>(new Set());
  const gameLoopRef = useRef<number | null>(null);
  const lastShotRef = useRef(0);
  const invaderDirRef = useRef(1);
  const invaderMoveTimerRef = useRef(0);
  const enemyFireTimerRef = useRef(0);
  const canvasRef = useRef<HTMLDivElement>(null);

  // ─── Game Loop ───────────────────────────────────────────────────────────
  const gameLoop = useCallback(() => {
    if (gameState !== "playing") return;

    setPlayerX((prev) => {
      let newX = prev;
      if (keysRef.current.has("ArrowLeft") || keysRef.current.has("a")) {
        newX = Math.max(0, newX - PLAYER_SPEED);
      }
      if (keysRef.current.has("ArrowRight") || keysRef.current.has("d")) {
        newX = Math.min(GAME_WIDTH - PLAYER_WIDTH, newX + PLAYER_SPEED);
      }
      return newX;
    });

    // Player shooting
    if (keysRef.current.has(" ") && Date.now() - lastShotRef.current > 300) {
      lastShotRef.current = Date.now();
      setBullets((prev) => {
        if (prev.length >= MAX_PLAYER_BULLETS) return prev;
        return [
          ...prev,
          {
            id: Date.now(),
            x: playerX + PLAYER_WIDTH / 2 - BULLET_WIDTH / 2,
            y: GAME_HEIGHT - 40,
            type: "player",
          },
        ];
      });
    }

    // Move player bullets
    setBullets((prev) =>
      prev
        .map((b) => ({ ...b, y: b.y - BULLET_SPEED }))
        .filter((b) => b.y > 0)
    );

    // Move enemy bullets
    setEnemyBullets((prev) =>
      prev
        .map((b) => ({ ...b, y: b.y + BULLET_SPEED * 0.6 }))
        .filter((b) => b.y < GAME_HEIGHT)
    );

    // Move invaders
    invaderMoveTimerRef.current++;
    const speed = Math.max(5, 30 - invaders.filter((i) => i.alive).length);
    if (invaderMoveTimerRef.current >= speed) {
      invaderMoveTimerRef.current = 0;
      setInvaders((prev) => {
        const aliveInvaders = prev.filter((i) => i.alive);
        if (aliveInvaders.length === 0) return prev;

        let shouldDrop = false;
        const dir = invaderDirRef.current;
        aliveInvaders.forEach((inv) => {
          if ((dir > 0 && inv.x + INVADER_SIZE + 10 > GAME_WIDTH) ||
              (dir < 0 && inv.x - 10 < 0)) {
            shouldDrop = true;
          }
        });

        if (shouldDrop) {
          invaderDirRef.current *= -1;
          return prev.map((inv) =>
            inv.alive ? { ...inv, y: inv.y + INVADER_DROP } : inv
          );
        }

        return prev.map((inv) =>
          inv.alive ? { ...inv, x: inv.x + 10 * dir } : inv
        );
      });

      // Enemy shooting
      enemyFireTimerRef.current++;
      if (enemyFireTimerRef.current >= Math.max(20, 60 - level * 5)) {
        enemyFireTimerRef.current = 0;
        setInvaders((prev) => {
          const aliveInvaders = prev.filter((i) => i.alive);
          if (aliveInvaders.length === 0) return prev;
          // Pick a random bottom invader from each column
          const columns = new Map<number, Invader>();
          aliveInvaders.forEach((inv) => {
            const col = Math.round(inv.x / 55);
            if (!columns.has(col) || (columns.get(col)?.y ?? 0) < inv.y) {
              columns.set(col, inv);
            }
          });
          const bottomInvaders = Array.from(columns.values());
          if (bottomInvaders.length > 0) {
            const shooter =
              bottomInvaders[Math.floor(Math.random() * bottomInvaders.length)];
            setEnemyBullets((prevBullets) => [
              ...prevBullets,
              {
                id: Date.now() + Math.random(),
                x: shooter.x + INVADER_SIZE / 2,
                y: shooter.y + INVADER_SIZE,
                type: "enemy",
              },
            ]);
          }
          return prev;
        });
      }
    }

    // Collision detection: player bullets vs invaders
    setBullets((prevBullets) => {
      let newScore = 0;
      const remainingBullets: Bullet[] = [];
      const newParticles: Particle[] = [];

      prevBullets.forEach((bullet) => {
        if (bullet.type !== "player") return;
        let hit = false;
        setInvaders((prevInvaders) => {
          const updated = prevInvaders.map((inv) => {
            if (!inv.alive) return inv;
            if (
              bullet.x < inv.x + INVADER_SIZE &&
              bullet.x + BULLET_WIDTH > inv.x &&
              bullet.y < inv.y + INVADER_SIZE &&
              bullet.y + BULLET_HEIGHT > inv.y
            ) {
              hit = true;
              newScore += (3 - inv.type) * 10 + 10;
              newParticles.push(
                ...createExplosion(
                  inv.x + INVADER_SIZE / 2,
                  inv.y + INVADER_SIZE / 2,
                  INVADER_COLORS[inv.type]
                )
              );
              return { ...inv, alive: false };
            }
            return inv;
          });
          return updated;
        });
        if (!hit) remainingBullets.push(bullet);
      });

      if (newScore > 0) {
        setScore((prev) => prev + newScore);
        setParticles((prev) => [...prev, ...newParticles]);
      }

      return remainingBullets;
    });

    // Collision detection: enemy bullets vs player
    setEnemyBullets((prevBullets) => {
      const remaining: Bullet[] = [];
      prevBullets.forEach((bullet) => {
        if (
          bullet.x < playerX + PLAYER_WIDTH &&
          bullet.x + BULLET_WIDTH > playerX &&
          bullet.y < GAME_HEIGHT - 20 &&
          bullet.y + BULLET_HEIGHT > GAME_HEIGHT - 40
        ) {
          setLives((prev) => {
            const newLives = prev - 1;
            if (newLives <= 0) {
              setGameState("gameOver");
            }
            return newLives;
          });
        } else {
          remaining.push(bullet);
        }
      });
      return remaining;
    });

    // Update particles
    setParticles((prev) =>
      prev
        .map((p) => ({
          ...p,
          x: p.x + p.vx,
          y: p.y + p.vy,
          life: p.life - 1,
        }))
        .filter((p) => p.life > 0)
    );

    // Check victory
    setInvaders((prev) => {
      if (prev.filter((i) => i.alive).length === 0) {
        setLevel((prev) => prev + 1);
        setScore((prev) => prev + 1000);
        return createInvaders();
      }
      // Check if invaders reached player
      if (prev.some((i) => i.alive && i.y > GAME_HEIGHT - 80)) {
        setGameState("gameOver");
      }
      return prev;
    });

    gameLoopRef.current = requestAnimationFrame(gameLoop);
  }, [gameState, playerX, level]);

  // ─── Game Loop Management ────────────────────────────────────────────────
  useEffect(() => {
    if (gameState === "playing") {
      gameLoopRef.current = requestAnimationFrame(gameLoop);
    }
    return () => {
      if (gameLoopRef.current) {
        cancelAnimationFrame(gameLoopRef.current);
      }
    };
  }, [gameState, gameLoop]);

  // ─── Keyboard Handlers ──────────────────────────────────────────────────
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      keysRef.current.add(e.key);
      if (e.key === " " || e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
      }
      // Restart with R key during playing
      if (e.key === "r" || e.key === "R") {
        if (gameState === "playing") {
          resetGame();
        }
      }
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      keysRef.current.delete(e.key);
    };
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [gameState]);

  // ─── Game Actions ───────────────────────────────────────────────────────
  const startGame = () => {
    setGameState("playing");
    setScore(0);
    setLives(3);
    setInvaders(createInvaders());
    setBullets([]);
    setEnemyBullets([]);
    setParticles([]);
    invaderDirRef.current = 1;
  };

  const resetGame = () => {
    setGameState("idle");
    setScore(0);
    setLives(3);
    setInvaders(createInvaders());
    setBullets([]);
    setEnemyBullets([]);
    setParticles([]);
    invaderDirRef.current = 1;
  };

  const restartAfterGameOver = () => {
    setHighScore((prev) => Math.max(prev, score));
    resetGame();
  };

  // ─── Render Helpers ─────────────────────────────────────────────────────
  const aliveInvaders = invaders.filter((i) => i.alive);
  const dataAnvilState = JSON.stringify({
    playerX,
    score,
    lives,
    gameState,
    level,
    highScore,
    invadersAlive: aliveInvaders.length,
  });

  return (
    <div
      data-anvil-state={dataAnvilState}
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        minHeight: "100vh",
        background: "linear-gradient(135deg, #0c0c1d 0%, #1a1a2e 50%, #16213e 100%)",
        fontFamily: "'Courier New', monospace",
        color: "#fff",
        padding: "20px",
      }}
    >
      {/* Title */}
      <h1
        style={{
          fontSize: "48px",
          fontWeight: "bold",
          marginBottom: "10px",
          background: "linear-gradient(90deg, #ff6b6b, #ffd93d, #6bcb77, #4d96ff)",
          WebkitBackgroundClip: "text",
          WebkitTextFillColor: "transparent",
          textShadow: "0 0 30px rgba(77, 150, 255, 0.5)",
          letterSpacing: "4px",
        }}
      >
        SPACE INVADERS
      </h1>

      {/* Score Display */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          width: GAME_WIDTH,
          marginBottom: "10px",
          fontSize: "18px",
        }}
      >
        <span>SCORE: {score.toString().padStart(6, "0")}</span>
        <span>LIVES: {"❤️".repeat(Math.max(0, lives))}</span>
        <span>LEVEL: {level}</span>
        <span>HIGH: {highScore.toString().padStart(6, "0")}</span>
      </div>

      {/* Game Canvas */}
      <div
        ref={canvasRef}
        style={{
          position: "relative",
          width: GAME_WIDTH,
          height: GAME_HEIGHT,
          background: "#000",
          border: "2px solid #4d96ff",
          borderRadius: "8px",
          overflow: "hidden",
          boxShadow: "0 0 30px rgba(77, 150, 255, 0.3)",
        }}
      >
        {/* Stars background */}
        {Array.from({ length: 50 }).map((_, i) => (
          <div
            key={`star-${i}`}
            style={{
              position: "absolute",
              left: `${Math.random() * 100}%`,
              top: `${Math.random() * 100}%`,
              width: "2px",
              height: "2px",
              background: "#fff",
              borderRadius: "50%",
              opacity: 0.3 + Math.random() * 0.7,
            }}
          />
        ))}

        {/* Invaders */}
        {invaders.map(
          (inv) =>
            inv.alive && (
              <div
                key={inv.id}
                style={{
                  position: "absolute",
                  left: inv.x,
                  top: inv.y,
                  width: INVADER_SIZE,
                  height: INVADER_SIZE,
                  background: INVADER_COLORS[inv.type],
                  borderRadius: "4px",
                  boxShadow: `0 0 10px ${INVADER_COLORS[inv.type]}`,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  fontSize: "12px",
                  fontWeight: "bold",
                  color: "#000",
                }}
              >
                <span>
                  {inv.type === 0 ? "👾" : inv.type === 1 ? "👽" : "🛸"}
                </span>
              </div>
            )
        )}

        {/* Player Bullets */}
        {bullets.map(
          (bullet) =>
            bullet.type === "player" && (
              <div
                key={bullet.id}
                style={{
                  position: "absolute",
                  left: bullet.x,
                  top: bullet.y,
                  width: BULLET_WIDTH,
                  height: BULLET_HEIGHT,
                  background: "#ffd93d",
                  borderRadius: "2px",
                  boxShadow: "0 0 8px #ffd93d",
                }}
              />
            )
        )}

        {/* Enemy Bullets */}
        {enemyBullets.map(
          (bullet) => (
            <div
              key={bullet.id}
              style={{
                position: "absolute",
                left: bullet.x - BULLET_WIDTH / 2,
                top: bullet.y,
                width: BULLET_WIDTH,
                height: BULLET_HEIGHT,
                background: "#ff6b6b",
                borderRadius: "2px",
                boxShadow: "0 0 8px #ff6b6b",
              }}
            />
          )
        )}

        {/* Player */}
        {gameState !== "gameOver" && (
          <div
            style={{
              position: "absolute",
              left: playerX,
              top: GAME_HEIGHT - 40,
              width: PLAYER_WIDTH,
              height: PLAYER_HEIGHT,
              background: "linear-gradient(180deg, #4d96ff, #2d5aa0)",
              borderRadius: "4px 4px 0 0",
              boxShadow: "0 0 15px #4d96ff",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontSize: "16px",
            }}
          >
            🚀
          </div>
        )}

        {/* Particles */}
        {particles.map((p, i) => (
          <div
            key={`particle-${i}`}
            style={{
              position: "absolute",
              left: p.x,
              top: p.y,
              width: "4px",
              height: "4px",
              background: p.color,
              borderRadius: "50%",
              opacity: p.life / 50,
            }}
          />
        ))}

        {/* Game Over Overlay */}
        {gameState === "gameOver" && (
          <div
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              background: "rgba(0, 0, 0, 0.85)",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              zIndex: 10,
            }}
          >
            <h2
              style={{
                fontSize: "48px",
                color: "#ff6b6b",
                marginBottom: "20px",
                textShadow: "0 0 20px #ff6b6b",
              }}
            >
              GAME OVER
            </h2>
            <p style={{ fontSize: "24px", marginBottom: "10px" }}>
              Final Score: {score}
            </p>
            <p style={{ fontSize: "18px", marginBottom: "30px", color: "#aaa" }}>
              Level Reached: {level}
            </p>
            <button
              data-anvil-action="restart"
              onClick={restartAfterGameOver}
              style={{
                padding: "12px 32px",
                fontSize: "20px",
                background: "linear-gradient(90deg, #ff6b6b, #ffd93d)",
                border: "none",
                borderRadius: "8px",
                cursor: "pointer",
                fontWeight: "bold",
                color: "#000",
                boxShadow: "0 0 20px rgba(255, 107, 107, 0.5)",
                transition: "transform 0.2s",
              }}
              onMouseEnter={(e) =>
                (e.currentTarget.style.transform = "scale(1.05)")
              }
              onMouseLeave={(e) =>
                (e.currentTarget.style.transform = "scale(1)")
              }
            >
              PLAY AGAIN
            </button>
            <p style={{ marginTop: "20px", fontSize: "14px", color: "#888" }}>
              or press R to restart
            </p>
          </div>
        )}

        {/* Idle Overlay */}
        {gameState === "idle" && (
          <div
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              background: "rgba(0, 0, 0, 0.85)",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              zIndex: 10,
            }}
          >
            <h2
              style={{
                fontSize: "36px",
                color: "#4d96ff",
                marginBottom: "30px",
                textShadow: "0 0 20px #4d96ff",
              }}
            >
              READY?
            </h2>
            <button
              data-anvil-action="primary"
              onClick={startGame}
              style={{
                padding: "16px 48px",
                fontSize: "24px",
                background: "linear-gradient(90deg, #4d96ff, #6bcb77)",
                border: "none",
                borderRadius: "12px",
                cursor: "pointer",
                fontWeight: "bold",
                color: "#fff",
                boxShadow: "0 0 30px rgba(77, 150, 255, 0.6)",
                transition: "transform 0.2s",
              }}
              onMouseEnter={(e) =>
                (e.currentTarget.style.transform = "scale(1.05)")
              }
              onMouseLeave={(e) =>
                (e.currentTarget.style.transform = "scale(1)")
              }
            >
              START GAME
            </button>
            <div
              style={{
                marginTop: "30px",
                textAlign: "center",
                fontSize: "14px",
                color: "#aaa",
                lineHeight: "1.8",
              }}
            >
              <p>
                <span style={{ color: "#ffd93d" }}>← →</span> or{" "}
                <span style={{ color: "#ffd93d" }}>A D</span> to move
              </p>
              <p>
                <span style={{ color: "#ffd93d" }}>SPACE</span> to shoot
              </p>
              <p>
                <span style={{ color: "#ffd93d" }}>R</span> to restart
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Controls Info */}
      <div
        style={{
          marginTop: "20px",
          fontSize: "14px",
          color: "#888",
          textAlign: "center",
        }}
      >
        <p>
          Use <strong>Arrow Keys</strong> or <strong>A/D</strong> to move,{" "}
          <strong>Space</strong> to shoot,{" "}
          <strong>R</strong> to restart
        </p>
        <p style={{ marginTop: "8px", fontSize: "12px" }}>
          Destroy all invaders to advance to the next level!
        </p>
      </div>
    </div>
  );
}
