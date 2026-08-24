"use client";

import { useEffect, useRef, useState, useCallback } from "react";

// ─── Types ───────────────────────────────────────────────────────────────────
interface Ball {
  x: number;
  y: number;
  dx: number;
  dy: number;
  radius: number;
  speed: number;
}

interface Paddle {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface Brick {
  x: number;
  y: number;
  width: number;
  height: number;
  color: string;
  alive: boolean;
}

interface Particle {
  x: number;
  y: number;
  dx: number;
  dy: number;
  life: number;
  maxLife: number;
  color: string;
  size: number;
}

type GameState = "start" | "playing" | "paused" | "game-over" | "victory";

// ─── Constants ───────────────────────────────────────────────────────────────
const CANVAS_W = 800;
const CANVAS_H = 600;
const PADDLE_W = 120;
const PADDLE_H = 16;
const BALL_R = 8;
const BALL_SPEED = 5;
const BRICK_ROWS = 6;
const BRICK_COLS = 10;
const BRICK_H = 24;
const BRICK_PAD = 4;
const BRICK_TOP = 60;
const LIVES_INIT = 3;
const MAX_PARTICLES = 200;

const NEON_COLORS = [
  "#ff00ff", "#00ffff", "#ff0080", "#80ff00",
  "#ff8000", "#0080ff", "#ffff00", "#00ff80",
];

// ─── Helpers ─────────────────────────────────────────────────────────────────
function createBricks(): Brick[] {
  const bricks: Brick[] = [];
  const totalW = BRICK_COLS * (BRICK_W() + BRICK_PAD) - BRICK_PAD;
  const offsetX = (CANVAS_W - totalW) / 2;
  for (let r = 0; r < BRICK_ROWS; r++) {
    for (let c = 0; c < BRICK_COLS; c++) {
      bricks.push({
        x: offsetX + c * (BRICK_W() + BRICK_PAD),
        y: BRICK_TOP + r * (BRICK_H + BRICK_PAD),
        width: BRICK_W(),
        height: BRICK_H,
        color: NEON_COLORS[r % NEON_COLORS.length],
        alive: true,
      });
    }
  }
  return bricks;
}

function BRICK_W(): number {
  return (CANVAS_W - BRICK_PAD * (BRICK_COLS + 1)) / BRICK_COLS;
}

function createBall(): Ball {
  return {
    x: CANVAS_W / 2,
    y: CANVAS_H - 60,
    dx: BALL_SPEED * (Math.random() > 0.5 ? 1 : -1),
    dy: -BALL_SPEED,
    radius: BALL_R,
    speed: BALL_SPEED,
  };
}

function createPaddle(): Paddle {
  return {
    x: (CANVAS_W - PADDLE_W) / 2,
    y: CANVAS_H - 40,
    width: PADDLE_W,
    height: PADDLE_H,
  };
}

function spawnParticles(
  px: number,
  py: number,
  color: string,
  particles: Particle[]
) {
  for (let i = 0; i < 12 && particles.length < MAX_PARTICLES; i++) {
    const angle = Math.random() * Math.PI * 2;
    const speed = 1 + Math.random() * 3;
    particles.push({
      x: px,
      y: py,
      dx: Math.cos(angle) * speed,
      dy: Math.sin(angle) * speed,
      life: 1,
      maxLife: 0.4 + Math.random() * 0.6,
      color,
      size: 2 + Math.random() * 4,
    });
  }
}

// ─── Component ───────────────────────────────────────────────────────────────
export default function BreakoutGame() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animRef = useRef<number>(0);
  const ballRef = useRef<Ball>(createBall());
  const paddleRef = useRef<Paddle>(createPaddle());
  const bricksRef = useRef<Brick[]>(createBricks());
  const particlesRef = useRef<Particle[]>([]);
  const keysRef = useRef<Record<string, boolean>>({});
  const mouseRef = useRef<number>(CANVAS_W / 2);
  const launchedRef = useRef(false);
  const [gameState, setGameState] = useState<GameState>("start");
  const [score, setScore] = useState(0);
  const [lives, setLives] = useState(LIVES_INIT);
  const [level, setLevel] = useState(1);
  const [highScore, setHighScore] = useState(0);

  // ─── Reset game ──────────────────────────────────────────────────────────
  const resetGame = useCallback(() => {
    ballRef.current = createBall();
    paddleRef.current = createPaddle();
    bricksRef.current = createBricks();
    particlesRef.current = [];
    launchedRef.current = false;
    setScore(0);
    setLives(LIVES_INIT);
    setLevel(1);
    setGameState("start");
  }, []);

  const resetLevel = useCallback(() => {
    ballRef.current = createBall();
    bricksRef.current = createBricks();
    particlesRef.current = [];
    launchedRef.current = false;
    setScore(0);
    setLives(LIVES_INIT);
    setGameState("start");
  }, []);

  // ─── Start / Restart handlers ────────────────────────────────────────────
  const handleStart = useCallback(() => {
    setGameState("playing");
    launchedRef.current = false;
  }, []);

  const handleRestart = useCallback(() => {
    resetGame();
  }, [resetGame]);

  // ─── Keyboard input ──────────────────────────────────────────────────────
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      keysRef.current[e.key] = true;
      if (e.key === " " || e.key === "Space") {
        e.preventDefault();
        if (gameState === "playing" && !launchedRef.current) {
          launchedRef.current = true;
        }
      }
      if (e.key === "Escape" && gameState === "playing") {
        setGameState("paused");
      } else if (e.key === "Escape" && gameState === "paused") {
        setGameState("playing");
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
  }, [gameState]);

  // ─── Mouse input ─────────────────────────────────────────────────────────
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const onMouseMove = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      const scaleX = CANVAS_W / rect.width;
      mouseRef.current = (e.clientX - rect.left) * scaleX;
    };
    const onClick = () => {
      if (gameState === "playing" && !launchedRef.current) {
        launchedRef.current = true;
      }
    };
    canvas.addEventListener("mousemove", onMouseMove);
    canvas.addEventListener("click", onClick);
    return () => {
      canvas.removeEventListener("mousemove", onMouseMove);
      canvas.removeEventListener("click", onClick);
    };
  }, [gameState]);

  // ─── Game loop ───────────────────────────────────────────────────────────
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let lastTime = 0;

    const gameLoop = (timestamp: number) => {
      const dt = Math.min((timestamp - lastTime) / 1000, 0.05);
      lastTime = timestamp;

      const ball = ballRef.current;
      const paddle = paddleRef.current;
      const bricks = bricksRef.current;
      const particles = particlesRef.current;

      // ── Paddle movement ────────────────────────────────────────────────
      const paddleSpeed = 400;
      if (keysRef.current["ArrowLeft"] || keysRef.current["a"]) {
        paddle.x = Math.max(0, paddle.x - paddleSpeed * dt);
      }
      if (keysRef.current["ArrowRight"] || keysRef.current["d"]) {
        paddle.x = Math.min(CANVAS_W - paddle.width, paddle.x + paddleSpeed * dt);
      }
      // Mouse override
      if (mouseRef.current !== undefined) {
        paddle.x = Math.max(0, Math.min(CANVAS_W - paddle.width, mouseRef.current - paddle.width / 2));
      }

      // ── Ball physics (only when playing) ───────────────────────────────
      if (gameState === "playing") {
        if (!launchedRef.current) {
          ball.x = paddle.x + paddle.width / 2;
          ball.y = paddle.y - ball.radius - 2;
        } else {
          ball.x += ball.dx;
          ball.y += ball.dy;

          // Wall collisions
          if (ball.x - ball.radius <= 0) {
            ball.x = ball.radius;
            ball.dx = Math.abs(ball.dx);
          }
          if (ball.x + ball.radius >= CANVAS_W) {
            ball.x = CANVAS_W - ball.radius;
            ball.dx = -Math.abs(ball.dx);
          }
          if (ball.y - ball.radius <= 0) {
            ball.y = ball.radius;
            ball.dy = Math.abs(ball.dy);
          }

          // Paddle collision
          if (
            ball.dy > 0 &&
            ball.y + ball.radius >= paddle.y &&
            ball.y + ball.radius <= paddle.y + paddle.height + 8 &&
            ball.x >= paddle.x &&
            ball.x <= paddle.x + paddle.width
          ) {
            const hitPos = (ball.x - paddle.x) / paddle.width;
            const angle = -Math.PI / 2 + (hitPos - 0.5) * Math.PI * 0.7;
            const speed = Math.sqrt(ball.dx * ball.dx + ball.dy * ball.dy);
            ball.dx = Math.cos(angle) * speed;
            ball.dy = Math.sin(angle) * speed;
            ball.y = paddle.y - ball.radius - 1;
            spawnParticles(ball.x, ball.y, "#00ffff", particles);
          }

          // Brick collisions
          let allDead = true;
          for (const brick of bricks) {
            if (!brick.alive) continue;
            allDead = false;
            if (
              ball.x + ball.radius > brick.x &&
              ball.x - ball.radius < brick.x + brick.width &&
              ball.y + ball.radius > brick.y &&
              ball.y - ball.radius < brick.y + brick.height
            ) {
              brick.alive = false;
              spawnParticles(
                brick.x + brick.width / 2,
                brick.y + brick.height / 2,
                brick.color,
                particles
              );

              // Determine bounce direction
              const overlapLeft = ball.x + ball.radius - brick.x;
              const overlapRight = brick.x + brick.width - (ball.x - ball.radius);
              const overlapTop = ball.y + ball.radius - brick.y;
              const overlapBottom = brick.y + brick.height - (ball.y - ball.radius);
              const minOverlap = Math.min(
                overlapLeft,
                overlapRight,
                overlapTop,
                overlapBottom
              );
              if (minOverlap === overlapTop || minOverlap === overlapBottom) {
                ball.dy = -ball.dy;
              } else {
                ball.dx = -ball.dx;
              }

              setScore((prev) => {
                const newScore = prev + 10;
                if (newScore > highScore) setHighScore(newScore);
                return newScore;
              });
              break;
            }
          }

          // Check if all bricks destroyed
          if (allDead && bricks.some((b) => b.alive) === false) {
            // Victory!
            setGameState("victory");
          }

          // Ball lost
          if (ball.y - ball.radius > CANVAS_H) {
            const newLives = lives - 1;
            setLives(newLives);
            if (newLives <= 0) {
              setGameState("game-over");
            } else {
              ballRef.current = createBall();
              launchedRef.current = false;
            }
          }
        }
      }

      // ── Particle update ────────────────────────────────────────────────
      for (let i = particles.length - 1; i >= 0; i--) {
        const p = particles[i];
        p.x += p.dx;
        p.y += p.dy;
        p.dy += 3 * dt; // gravity
        p.life -= dt / p.maxLife;
        if (p.life <= 0) {
          particles.splice(i, 1);
        }
      }

      // ── Render ─────────────────────────────────────────────────────────
      // Background with gradient
      const bgGrad = ctx.createLinearGradient(0, 0, 0, CANVAS_H);
      bgGrad.addColorStop(0, "#0a0a1a");
      bgGrad.addColorStop(1, "#1a0a2a");
      ctx.fillStyle = bgGrad;
      ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);

      // Grid lines (subtle neon grid)
      ctx.strokeStyle = "rgba(100, 0, 255, 0.08)";
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

      // Bricks with neon glow
      for (const brick of bricks) {
        if (!brick.alive) continue;
        ctx.shadowColor = brick.color;
        ctx.shadowBlur = 15;
        ctx.fillStyle = brick.color;
        ctx.fillRect(brick.x, brick.y, brick.width, brick.height);
        ctx.shadowBlur = 0;

        // Brick highlight
        ctx.fillStyle = "rgba(255,255,255,0.2)";
        ctx.fillRect(brick.x, brick.y, brick.width, brick.height / 2);
      }

      // Paddle with glow
      ctx.shadowColor = "#00ffff";
      ctx.shadowBlur = 20;
      const paddleGrad = ctx.createLinearGradient(paddle.x, 0, paddle.x + paddle.width, 0);
      paddleGrad.addColorStop(0, "#0080ff");
      paddleGrad.addColorStop(0.5, "#00ffff");
      paddleGrad.addColorStop(1, "#0080ff");
      ctx.fillStyle = paddleGrad;
      ctx.beginPath();
      ctx.roundRect(paddle.x, paddle.y, paddle.width, paddle.height, 8);
      ctx.fill();
      ctx.shadowBlur = 0;

      // Paddle edge glow
      ctx.strokeStyle = "#ffffff";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.roundRect(paddle.x, paddle.y, paddle.width, paddle.height, 8);
      ctx.stroke();

      // Ball
      if (gameState === "playing" || gameState === "start") {
        ctx.shadowColor = "#ffffff";
        ctx.shadowBlur = 25;
        ctx.fillStyle = "#ffffff";
        ctx.beginPath();
        ctx.arc(ball.x, ball.y, ball.radius, 0, Math.PI * 2);
        ctx.fill();
        ctx.shadowBlur = 0;

        // Ball trail
        ctx.fillStyle = "rgba(255, 255, 255, 0.15)";
        ctx.beginPath();
        ctx.arc(ball.x - ball.dx * 2, ball.y - ball.dy * 2, ball.radius * 0.8, 0, Math.PI * 2);
        ctx.fill();
      }

      // Particles
      for (const p of particles) {
        ctx.globalAlpha = p.life;
        ctx.shadowColor = p.color;
        ctx.shadowBlur = 10;
        ctx.fillStyle = p.color;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size * p.life, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.globalAlpha = 1;
      ctx.shadowBlur = 0;

      // HUD
      ctx.font = "bold 16px monospace";
      ctx.fillStyle = "#00ffff";
      ctx.textAlign = "left";
      ctx.fillText(`SCORE: ${score}`, 16, 28);
      ctx.textAlign = "right";
      ctx.fillText(`LIVES: ${lives}`, CANVAS_W - 16, 28);

      // "Press Space" hint
      if (gameState === "playing" && !launchedRef.current) {
        ctx.font = "14px monospace";
        ctx.fillStyle = "rgba(255,255,255,0.5)";
        ctx.textAlign = "center";
        ctx.fillText("Press SPACE or click to launch", CANVAS_W / 2, CANVAS_H - 70);
      }

      animRef.current = requestAnimationFrame(gameLoop);
    };

    animRef.current = requestAnimationFrame(gameLoop);
    return () => cancelAnimationFrame(animRef.current);
  }, [gameState, score, lives]);

  // ─── data-anvil-state snapshot ────────────────────────────────────────────
  const anvilState = JSON.stringify({
    paddleX: Math.round(paddleRef.current?.x ?? 0),
    score,
    gameState,
    lives,
    level,
  });

  return (
    <div
      className="min-h-screen bg-gray-950 flex flex-col items-center justify-center gap-6 p-4"
      data-anvil-state={anvilState}
    >
      {/* Title */}
      <h1 className="text-4xl font-bold text-transparent bg-clip-text bg-gradient-to-r from-purple-400 via-pink-500 to-cyan-400 tracking-wider">
        NEON BREAKOUT
      </h1>

      {/* Canvas */}
      <div className="relative">
        <canvas
          ref={canvasRef}
          width={CANVAS_W}
          height={CANVAS_H}
          className="rounded-xl border-2 border-purple-500/50 shadow-lg shadow-purple-500/20 cursor-none"
          style={{ maxWidth: "100%", maxHeight: "70vh" }}
        />

        {/* Start Overlay */}
        {gameState === "start" && (
          <div className="absolute inset-0 bg-black/70 flex flex-col items-center justify-center rounded-xl">
            <p className="text-cyan-400 text-xl mb-6 font-mono">
              Break the bricks. Don&apos;t lose the ball.
            </p>
            <button
              onClick={handleStart}
              data-anvil-action="primary"
              className="px-8 py-3 bg-gradient-to-r from-purple-600 to-cyan-500 text-white font-bold text-lg rounded-lg hover:scale-105 transition-transform shadow-lg shadow-purple-500/40"
            >
              ▶ START GAME
            </button>
            <p className="text-gray-500 text-sm mt-4 font-mono">
              Mouse or Arrow Keys to move · Space to launch
            </p>
          </div>
        )}

        {/* Paused Overlay */}
        {gameState === "paused" && (
          <div className="absolute inset-0 bg-black/70 flex flex-col items-center justify-center rounded-xl">
            <p className="text-yellow-400 text-3xl font-bold mb-4 font-mono">
              PAUSED
            </p>
            <button
              onClick={() => setGameState("playing")}
              data-anvil-action="primary"
              className="px-6 py-2 bg-yellow-600 text-white font-bold rounded-lg hover:scale-105 transition-transform"
            >
              ▶ RESUME
            </button>
          </div>
        )}

        {/* Game Over Overlay */}
        {gameState === "game-over" && (
          <div className="absolute inset-0 bg-black/80 flex flex-col items-center justify-center rounded-xl">
            <p className="text-red-500 text-4xl font-bold mb-2 font-mono">
              GAME OVER
            </p>
            <p className="text-gray-400 text-lg mb-6 font-mono">
              Final Score: {score}
            </p>
            <button
              onClick={handleRestart}
              data-anvil-action="restart"
              className="px-8 py-3 bg-gradient-to-r from-red-600 to-orange-500 text-white font-bold text-lg rounded-lg hover:scale-105 transition-transform shadow-lg shadow-red-500/40"
            >
              ↻ RESTART
            </button>
          </div>
        )}

        {/* Victory Overlay */}
        {gameState === "victory" && (
          <div className="absolute inset-0 bg-black/80 flex flex-col items-center justify-center rounded-xl">
            <p className="text-yellow-400 text-4xl font-bold mb-2 font-mono animate-pulse">
              ★ VICTORY ★
            </p>
            <p className="text-gray-400 text-lg mb-6 font-mono">
              Score: {score} · Lives: {lives}
            </p>
            <button
              onClick={handleRestart}
              data-anvil-action="restart"
              className="px-8 py-3 bg-gradient-to-r from-yellow-500 to-cyan-500 text-white font-bold text-lg rounded-lg hover:scale-105 transition-transform shadow-lg shadow-yellow-500/40"
            >
              ↻ PLAY AGAIN
            </button>
          </div>
        )}
      </div>

      {/* Restart button for in-play recovery (visible at bottom) */}
      {gameState === "playing" && (
        <button
          onClick={handleRestart}
          data-anvil-action="restart"
          className="px-4 py-2 text-sm text-gray-500 border border-gray-700 rounded hover:text-white hover:border-gray-500 transition-colors font-mono"
        >
          ↻ Restart
        </button>
      )}

      {/* Controls info */}
      <div className="text-gray-600 text-sm font-mono text-center">
        <span className="text-gray-500">← →</span> or{" "}
        <span className="text-gray-500">A D</span> to move ·{" "}
        <span className="text-gray-500">SPACE</span> to launch ·{" "}
        <span className="text-gray-500">ESC</span> pause
      </div>
    </div>
  );
}
