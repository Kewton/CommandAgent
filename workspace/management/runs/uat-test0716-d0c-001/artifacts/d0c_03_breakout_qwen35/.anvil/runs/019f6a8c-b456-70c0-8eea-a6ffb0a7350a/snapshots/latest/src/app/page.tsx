"use client";
import { useEffect, useRef, useState, useCallback } from "react";

// ─── Types ───────────────────────────────────────────────────────────────────
interface Brick {
  x: number;
  y: number;
  w: number;
  h: number;
  color: string;
  alive: boolean;
  hits: number;
  maxHits: number;
}

interface Ball {
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
  speed: number;
}

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  maxLife: number;
  color: string;
  size: number;
}

type GameState = "idle" | "playing" | "gameover" | "victory";

// ─── Constants ───────────────────────────────────────────────────────────────
const CANVAS_W = 800;
const CANVAS_H = 600;
const BRICK_ROWS = 6;
const BRICK_COLS = 10;
const BRICK_H = 24;
const BRICK_PAD = 4;
const PADDLE_W = 100;
const PADDLE_H = 14;
const BALL_R = 8;
const MAX_LIVES = 3;
const NEON_COLORS = [
  "#ff006e", "#ff5c00", "#ffbe0b", "#00f5d4", "#00bbf9",
  "#9b5de5", "#f15bb5", "#fee440", "#8338ec", "#3a86ff",
];

// ─── Helpers ─────────────────────────────────────────────────────────────────
function createBricks(): Brick[] {
  const bricks: Brick[] = [];
  const totalW = BRICK_COLS * (BRICK_W() + BRICK_PAD) - BRICK_PAD;
  const startX = (CANVAS_W - totalW) / 2;
  for (let r = 0; r < BRICK_ROWS; r++) {
    for (let c = 0; c < BRICK_COLS; c++) {
      const x = startX + c * (BRICK_W() + BRICK_PAD);
      const y = 60 + r * (BRICK_H + BRICK_PAD);
      bricks.push({
        x, y,
        w: BRICK_W(),
        h: BRICK_H,
        color: NEON_COLORS[(r + c) % NEON_COLORS.length],
        alive: true,
        hits: 0,
        maxHits: r < 2 ? 2 : 1,
      });
    }
  }
  return bricks;
}

function BRICK_W(): number {
  return (CANVAS_W - BRICK_PAD * (BRICK_COLS + 1)) / BRICK_COLS;
}

function spawnParticles(
  cx: number,
  cy: number,
  color: string,
  count: number
): Particle[] {
  const particles: Particle[] = [];
  for (let i = 0; i < count; i++) {
    const angle = Math.random() * Math.PI * 2;
    const speed = 1 + Math.random() * 4;
    particles.push({
      x: cx,
      y: cy,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed,
      life: 1,
      maxLife: 0.4 + Math.random() * 0.6,
      color,
      size: 2 + Math.random() * 4,
    });
  }
  return particles;
}

// ─── Component ───────────────────────────────────────────────────────────────
export default function BlockBreaker() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animRef = useRef<number>(0);
  const keysRef = useRef<Record<string, boolean>>({});
  const mouseXRef = useRef<number>(CANVAS_W / 2);

  // Game state exposed to data-anvil-state
  const [gameState, setGameState] = useState<GameState>("idle");
  const [score, setScore] = useState(0);
  const [lives, setLives] = useState(MAX_LIVES);
  const [level, setLevel] = useState(1);
  const [paddleX, setPaddleX] = useState(CANVAS_W / 2 - PADDLE_W / 2);

  // Mutable game refs (not re-rendered)
  const bricksRef = useRef<Brick[]>(createBricks());
  const ballRef = useRef<Ball>({
    x: CANVAS_W / 2,
    y: CANVAS_H - 80,
    vx: 4,
    vy: -4,
    r: BALL_R,
    speed: 5,
  });
  const particlesRef = useRef<Particle[]>([]);
  const scoreRef = useRef(0);
  const livesRef = useRef(MAX_LIVES);
  const paddleXRef = useRef(CANVAS_W / 2 - PADDLE_W / 2);
  const comboRef = useRef(0);

  const resetBall = useCallback(() => {
    const b = ballRef.current;
    b.x = CANVAS_W / 2;
    b.y = CANVAS_H - 80;
    b.vx = (Math.random() > 0.5 ? 1 : -1) * 4;
    b.vy = -4;
    b.speed = 5 + level * 0.5;
  }, [level]);

  const resetGame = useCallback(() => {
    bricksRef.current = createBricks();
    particlesRef.current = [];
    scoreRef.current = 0;
    livesRef.current = MAX_LIVES;
    comboRef.current = 0;
    setScore(0);
    setLives(MAX_LIVES);
    setGameState("idle");
    resetBall();
  }, [resetBall]);

  const restartGame = useCallback(() => {
    resetGame();
  }, [resetGame]);

  // ─── Keyboard ──────────────────────────────────────────────────────────
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      keysRef.current[e.key] = true;
      if (e.key === " " || e.key === "Enter") {
        e.preventDefault();
        if (gameState === "idle") {
          setGameState("playing");
        } else if (gameState === "gameover" || gameState === "victory") {
          restartGame();
        }
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      keysRef.current[e.key] = false;
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [gameState, restartGame]);

  // ─── Mouse / Touch ─────────────────────────────────────────────────────
  const handlePointerMove = useCallback((clientX: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const scaleX = CANVAS_W / rect.width;
    mouseXRef.current = (clientX - rect.left) * scaleX;
  }, []);

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => handlePointerMove(e.clientX),
    [handlePointerMove]
  );
  const handleTouchMove = useCallback(
    (e: React.TouchEvent) => {
      if (e.touches.length > 0) {
        handlePointerMove(e.touches[0].clientX);
      }
    },
    [handlePointerMove]
  );

  // ─── Game Loop ─────────────────────────────────────────────────────────
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;

    let lastTime = performance.now();

    const update = (dt: number) => {
      if (gameState !== "playing") return;

      const b = ballRef.current;
      const bx = paddleXRef.current;
      const bricks = bricksRef.current;
      const particles = particlesRef.current;

      // Paddle movement
      if (keysRef.current["ArrowLeft"] || keysRef.current["a"]) {
        paddleXRef.current -= 8;
      }
      if (keysRef.current["ArrowRight"] || keysRef.current["d"]) {
        paddleXRef.current += 8;
      }
      // Mouse override
      paddleXRef.current = Math.max(
        0,
        Math.min(CANVAS_W - PADDLE_W, mouseXRef.current - PADDLE_W / 2)
      );
      setPaddleX(paddleXRef.current);

      // Ball movement
      b.x += b.vx;
      b.y += b.vy;

      // Wall collisions
      if (b.x - b.r <= 0) { b.x = b.r; b.vx = Math.abs(b.vx); }
      if (b.x + b.r >= CANVAS_W) { b.x = CANVAS_W - b.r; b.vx = -Math.abs(b.vx); }
      if (b.y - b.r <= 0) { b.y = b.r; b.vy = Math.abs(b.vy); }

      // Bottom – lose life
      if (b.y + b.r >= CANVAS_H) {
        livesRef.current -= 1;
        comboRef.current = 0;
        if (livesRef.current <= 0) {
          setGameState("gameover");
          setLives(0);
          return;
        }
        setLives(livesRef.current);
        resetBall();
        return;
      }

      // Paddle collision
      const py = CANVAS_H - 40;
      if (
        b.y + b.r >= py &&
        b.y + b.r <= py + PADDLE_H + 8 &&
        b.x >= bx &&
        b.x <= bx + PADDLE_W
      ) {
        const hitPos = (b.x - bx) / PADDLE_W; // 0..1
        const angle = (hitPos - 0.5) * Math.PI * 0.7;
        b.vx = Math.sin(angle) * b.speed;
        b.vy = -Math.cos(angle) * b.speed;
        b.y = py - b.r;
        particles.push(
          ...spawnParticles(b.x, py, "#00f5d4", 8)
        );
      }

      // Brick collisions
      for (const brick of bricks) {
        if (!brick.alive) continue;
        const closestX = Math.max(brick.x, Math.min(b.x, brick.x + brick.w));
        const closestY = Math.max(brick.y, Math.min(b.y, brick.y + brick.h));
        const dx = b.x - closestX;
        const dy = b.y - closestY;
        if (dx * dx + dy * dy <= b.r * b.r) {
          brick.hits += 1;
          if (brick.hits >= brick.maxHits) {
            brick.alive = false;
            comboRef.current += 1;
            scoreRef.current += 10 * comboRef.current;
            setScore(scoreRef.current);
            particles.push(
              ...spawnParticles(
                brick.x + brick.w / 2,
                brick.y + brick.h / 2,
                brick.color,
                15
              )
            );
          } else {
            particles.push(
              ...spawnParticles(closestX, closestY, brick.color, 6)
            );
          }
          // Reflect
          const overlapLeft = b.x + b.r - brick.x;
          const overlapRight = brick.x + brick.w - (b.x - b.r);
          const overlapTop = b.y + b.r - brick.y;
          const overlapBottom = brick.y + brick.h - (b.y - b.r);
          const minOverlap = Math.min(
            overlapLeft, overlapRight, overlapTop, overlapBottom
          );
          if (minOverlap === overlapLeft || minOverlap === overlapRight) {
            b.vx = -b.vx;
          } else {
            b.vy = -b.vy;
          }
          break; // one brick per frame
        }
      }

      // Check victory
      if (bricks.every((br) => !br.alive)) {
        setGameState("victory");
        return;
      }

      // Particles
      for (let i = particles.length - 1; i >= 0; i--) {
        const p = particles[i];
        p.x += p.vx;
        p.y += p.vy;
        p.vy += 0.15;
        p.life -= dt / p.maxLife;
        if (p.life <= 0) particles.splice(i, 1);
      }
    };

    const draw = () => {
      ctx.clearRect(0, 0, CANVAS_W, CANVAS_H);

      // Background gradient
      const grad = ctx.createLinearGradient(0, 0, 0, CANVAS_H);
      grad.addColorStop(0, "#0a0a1a");
      grad.addColorStop(1, "#1a0a2e");
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);

      // Grid lines (subtle)
      ctx.strokeStyle = "rgba(100, 100, 255, 0.05)";
      ctx.lineWidth = 1;
      for (let x = 0; x < CANVAS_W; x += 40) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, CANVAS_H);
        ctx.stroke();
      }
      for (let y = 0; y < CANVAS_H; y += 40) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(CANVAS_W, y);
        ctx.stroke();
      }

      // Bricks
      for (const brick of bricksRef.current) {
        if (!brick.alive) continue;
        const alpha = brick.hits > 0 ? 0.6 : 1;
        ctx.save();
        ctx.globalAlpha = alpha;
        ctx.shadowColor = brick.color;
        ctx.shadowBlur = 12;
        ctx.fillStyle = brick.color;
        ctx.beginPath();
        ctx.roundRect(brick.x, brick.y, brick.w, brick.h, 4);
        ctx.fill();
        // Inner highlight
        ctx.shadowBlur = 0;
        ctx.fillStyle = "rgba(255,255,255,0.2)";
        ctx.fillRect(brick.x + 2, brick.y + 2, brick.w - 4, brick.h / 2 - 2);
        ctx.restore();
      }

      // Paddle
      ctx.save();
      ctx.shadowColor = "#00f5d4";
      ctx.shadowBlur = 20;
      const paddleGrad = ctx.createLinearGradient(
        paddleXRef.current, 0,
        paddleXRef.current + PADDLE_W, 0
      );
      paddleGrad.addColorStop(0, "#00f5d4");
      paddleGrad.addColorStop(1, "#00bbf9");
      ctx.fillStyle = paddleGrad;
      ctx.beginPath();
      ctx.roundRect(
        paddleXRef.current,
        CANVAS_H - 40,
        PADDLE_W,
        PADDLE_H,
        7
      );
      ctx.fill();
      ctx.restore();

      // Ball
      const ball = ballRef.current;
      ctx.save();
      ctx.shadowColor = "#fff";
      ctx.shadowBlur = 15;
      ctx.fillStyle = "#fff";
      ctx.beginPath();
      ctx.arc(ball.x, ball.y, ball.r, 0, Math.PI * 2);
      ctx.fill();
      // Ball trail
      ctx.shadowBlur = 0;
      ctx.fillStyle = "rgba(255,255,255,0.15)";
      ctx.beginPath();
      ctx.arc(ball.x - ball.vx, ball.y - ball.vy, ball.r * 0.7, 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();

      // Particles
      for (const p of particlesRef.current) {
        ctx.save();
        ctx.globalAlpha = p.life;
        ctx.shadowColor = p.color;
        ctx.shadowBlur = 6;
        ctx.fillStyle = p.color;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size * p.life, 0, Math.PI * 2);
        ctx.fill();
        ctx.restore();
      }
    };

    const loop = (now: number) => {
      const dt = Math.min((now - lastTime) / 1000, 0.05);
      lastTime = now;
      update(dt);
      draw();
      animRef.current = requestAnimationFrame(loop);
    };

    animRef.current = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(animRef.current);
  }, [gameState, resetBall]);

  // ─── State snapshot for data-anvil ─────────────────────────────────────
  const stateJson = JSON.stringify({
    paddleX: Math.round(paddleX),
    score,
    lives,
    gameState,
  });

  // ─── Render ────────────────────────────────────────────────────────────
  return (
    <div
      className="flex flex-col items-center justify-center min-h-screen bg-black"
      data-anvil-state={stateJson}
    >
      {/* Header */}
      <div className="mb-4 flex items-center gap-8">
        <h1
          className="text-3xl font-bold tracking-widest"
          style={{
            color: "#ff006e",
            textShadow: "0 0 10px #ff006e, 0 0 20px #ff006e",
          }}
        >
          NEON BREAKER
        </h1>
        <div className="flex gap-6 text-sm font-mono">
          <span style={{ color: "#00f5d4" }}>SCORE: {score}</span>
          <span style={{ color: "#ffbe0b" }}>LIVES: {lives}</span>
          <span style={{ color: "#9b5de5" }}>LEVEL: {level}</span>
        </div>
      </div>

      {/* Canvas Container */}
      <div className="relative">
        <canvas
          ref={canvasRef}
          width={CANVAS_W}
          height={CANVAS_H}
          className="rounded-xl border border-purple-900/50 cursor-none"
          style={{
            boxShadow: "0 0 30px rgba(155,93,229,0.3), 0 0 60px rgba(0,245,212,0.1)",
          }}
          onMouseMove={handleMouseMove}
          onTouchMove={handleTouchMove}
        />

        {/* Overlay: Idle */}
        {gameState === "idle" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/60 rounded-xl">
            <p
              className="text-5xl font-bold mb-6"
              style={{
                color: "#00f5d4",
                textShadow: "0 0 20px #00f5d4, 0 0 40px #00f5d4",
              }}
            >
              NEON BREAKER
            </p>
            <p className="text-gray-400 mb-8 text-sm">
              Use mouse or arrow keys to move the paddle
            </p>
            <button
              data-anvil-action="primary"
              onClick={() => setGameState("playing")}
              className="px-10 py-3 rounded-lg text-lg font-bold tracking-wider transition-all duration-200 hover:scale-105 active:scale-95"
              style={{
                background: "linear-gradient(135deg, #ff006e, #9b5de5)",
                color: "#fff",
                boxShadow: "0 0 20px rgba(255,0,110,0.5)",
              }}
            >
              START GAME
            </button>
            <p className="mt-4 text-gray-500 text-xs">or press Space / Enter</p>
          </div>
        )}

        {/* Overlay: Game Over */}
        {gameState === "gameover" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/70 rounded-xl">
            <p
              className="text-5xl font-bold mb-4"
              style={{
                color: "#ff006e",
                textShadow: "0 0 30px #ff006e, 0 0 60px #ff006e",
              }}
            >
              GAME OVER
            </p>
            <p className="text-xl mb-2" style={{ color: "#ffbe0b" }}>
              Final Score: {score}
            </p>
            <p className="text-gray-400 mb-8">The bricks have won… this time.</p>
            <button
              data-anvil-action="restart"
              onClick={restartGame}
              className="px-10 py-3 rounded-lg text-lg font-bold tracking-wider transition-all duration-200 hover:scale-105 active:scale-95"
              style={{
                background: "linear-gradient(135deg, #ff006e, #9b5de5)",
                color: "#fff",
                boxShadow: "0 0 20px rgba(255,0,110,0.5)",
              }}
            >
              TRY AGAIN
            </button>
            <p className="mt-4 text-gray-500 text-xs">or press Space / Enter</p>
          </div>
        )}

        {/* Overlay: Victory */}
        {gameState === "victory" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/70 rounded-xl">
            <p
              className="text-5xl font-bold mb-4"
              style={{
                color: "#00f5d4",
                textShadow: "0 0 30px #00f5d4, 0 0 60px #00f5d4",
              }}
            >
              VICTORY!
            </p>
            <p className="text-xl mb-2" style={{ color: "#ffbe0b" }}>
              Final Score: {score}
            </p>
            <p className="text-gray-400 mb-8">All bricks destroyed! You are unstoppable.</p>
            <button
              data-anvil-action="restart"
              onClick={restartGame}
              className="px-10 py-3 rounded-lg text-lg font-bold tracking-wider transition-all duration-200 hover:scale-105 active:scale-95"
              style={{
                background: "linear-gradient(135deg, #00f5d4, #00bbf9)",
                color: "#fff",
                boxShadow: "0 0 20px rgba(0,245,212,0.5)",
              }}
            >
              PLAY AGAIN
            </button>
            <p className="mt-4 text-gray-500 text-xs">or press Space / Enter</p>
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="mt-6 text-gray-600 text-xs text-center">
        <p>Mouse / Arrow Keys / Touch to move paddle</p>
        <p className="mt-1">Bricks with multiple hits require extra strikes</p>
      </div>
    </div>
  );
}
