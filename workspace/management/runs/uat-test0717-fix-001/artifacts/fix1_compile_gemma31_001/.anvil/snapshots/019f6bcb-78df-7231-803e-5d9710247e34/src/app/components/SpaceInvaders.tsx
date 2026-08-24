"use client";

import { useEffect, useRef, useState, useCallback } from "react";

// ─── Types ───────────────────────────────────────────────────────────────────
type GameState = "idle" | "playing" | "gameover" | "victory";

interface Bullet {
  x: number;
  y: number;
  dy: number;
}

interface InvaderBullet {
  x: number;
  y: number;
  dy: number;
}

interface Invader {
  x: number;
  y: number;
  alive: boolean;
  type: number; // 0, 1, 2 for different rows
}

// ─── Constants ───────────────────────────────────────────────────────────────
const CANVAS_W = 640;
const CANVAS_H = 520;
const PLAYER_W = 40;
const PLAYER_H = 20;
const PLAYER_SPEED = 5;
const BULLET_SPEED = 7;
const INV_BULLET_SPEED = 3.5;
const INVADER_COLS = 10;
const INVADER_ROWS = 5;
const INVADER_W = 32;
const INVADER_H = 24;
const INVADER_GAP_X = 12;
const INVADER_GAP_Y = 10;
const INVADER_START_X = 40;
const INVADER_START_Y = 60;
const PLAYER_Y = CANVAS_H - 50;
const MAX_LIVES = 3;
const WIN_SCORE = 1000;

// ─── Helpers ─────────────────────────────────────────────────────────────────
function createInvaders(): Invader[] {
  const inv: Invader[] = [];
  for (let r = 0; r < INVADER_ROWS; r++) {
    for (let c = 0; c < INVADER_COLS; c++) {
      inv.push({
        x: INVADER_START_X + c * (INVADER_W + INVADER_GAP_X),
        y: INVADER_START_Y + r * (INVADER_H + INVADER_GAP_Y),
        alive: true,
        type: r,
      });
    }
  }
  return inv;
}

function rectsOverlap(
  ax: number, ay: number, aw: number, ah: number,
  bx: number, by: number, bw: number, bh: number
): boolean {
  return ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by;
}

// ─── Component ───────────────────────────────────────────────────────────────
export default function SpaceInvaders() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animRef = useRef<number>(0);
  const keysRef = useRef<Set<string>>(new Set());
  const lastShotRef = useRef<number>(0);
  const lastInvShotRef = useRef<number>(0);

  // Game state stored in refs for the game loop
  const stateRef = useRef({
    playerX: CANVAS_W / 2 - PLAYER_W / 2,
    bullets: [] as Bullet[],
    invBullets: [] as InvaderBullet[],
    invaders: [] as Invader[],
    invDir: 1,
    invMoveTimer: 0,
    invMoveInterval: 40,
    score: 0,
    lives: MAX_LIVES,
    gameState: "idle" as GameState,
    playerHitFlash: 0,
  });

  // React state for data-anvil-state snapshot
  const [snapshot, setSnapshot] = useState({
    playerX: CANVAS_W / 2 - PLAYER_W / 2,
    score: 0,
    lives: MAX_LIVES,
    gameState: "idle" as GameState,
    invaderCount: INVADER_COLS * INVADER_ROWS,
  });

  // ─── Start / Restart ─────────────────────────────────────────────────────
  const startGame = useCallback(() => {
    const s = stateRef.current;
    s.playerX = CANVAS_W / 2 - PLAYER_W / 2;
    s.bullets = [];
    s.invBullets = [];
    s.invaders = createInvaders();
    s.invDir = 1;
    s.invMoveTimer = 0;
    s.invMoveInterval = 40;
    s.score = 0;
    s.lives = MAX_LIVES;
    s.gameState = "playing";
    s.playerHitFlash = 0;
    setSnapshot({
      playerX: s.playerX,
      score: 0,
      lives: MAX_LIVES,
      gameState: "playing",
      invaderCount: INVADER_COLS * INVADER_ROWS,
    });
  }, []);

  const restartGame = useCallback(() => {
    const s = stateRef.current;
    s.playerX = CANVAS_W / 2 - PLAYER_W / 2;
    s.bullets = [];
    s.invBullets = [];
    s.invaders = createInvaders();
    s.invDir = 1;
    s.invMoveTimer = 0;
    s.invMoveInterval = 40;
    s.score = 0;
    s.lives = MAX_LIVES;
    s.gameState = "playing";
    s.playerHitFlash = 0;
    setSnapshot({
      playerX: s.playerX,
      score: 0,
      lives: MAX_LIVES,
      gameState: "playing",
      invaderCount: INVADER_COLS * INVADER_ROWS,
    });
  }, []);

  // ─── Drawing helpers ─────────────────────────────────────────────────────
  const drawPlayer = useCallback((ctx: CanvasRenderingContext2D, x: number) => {
    ctx.save();
    // Ship body
    ctx.fillStyle = "#00ffcc";
    ctx.shadowColor = "#00ffcc";
    ctx.shadowBlur = 12;
    ctx.beginPath();
    ctx.moveTo(x + PLAYER_W / 2, PLAYER_Y - PLAYER_H);
    ctx.lineTo(x + PLAYER_W, PLAYER_Y);
    ctx.lineTo(x, PLAYER_Y);
    ctx.closePath();
    ctx.fill();
    // Cockpit
    ctx.fillStyle = "#ccffee";
    ctx.shadowBlur = 6;
    ctx.beginPath();
    ctx.arc(x + PLAYER_W / 2, PLAYER_Y - PLAYER_H / 2, 4, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();
  }, []);

  const drawInvader = useCallback((ctx: CanvasRenderingContext2D, inv: Invader) => {
    const colors = ["#ff3366", "#ff6633", "#ffcc00", "#33ff99", "#6699ff"];
    ctx.save();
    ctx.shadowColor = colors[inv.type] || "#ff3366";
    ctx.shadowBlur = 8;
    ctx.fillStyle = colors[inv.type] || "#ff3366";
    // Simple pixel-art style invader
    const cx = inv.x + INVADER_W / 2;
    const cy = inv.y + INVADER_H / 2;
    // Body
    ctx.fillRect(inv.x + 4, inv.y + 4, INVADER_W - 8, INVADER_H - 8);
    // Eyes
    ctx.fillStyle = "#000";
    ctx.fillRect(inv.x + 8, inv.y + 8, 4, 4);
    ctx.fillRect(inv.x + INVADER_W - 12, inv.y + 8, 4, 4);
    // Antennae
    ctx.fillStyle = colors[inv.type] || "#ff3366";
    ctx.fillRect(inv.x + 6, inv.y, 2, 4);
    ctx.fillRect(inv.x + INVADER_W - 8, inv.y, 2, 4);
    ctx.restore();
  }, []);

  const drawBullet = useCallback((ctx: CanvasRenderingContext2D, b: Bullet) => {
    ctx.save();
    ctx.fillStyle = "#ffff00";
    ctx.shadowColor = "#ffff00";
    ctx.shadowBlur = 6;
    ctx.fillRect(b.x - 2, b.y, 4, 10);
    ctx.restore();
  }, []);

  const drawInvBullet = useCallback((ctx: CanvasRenderingContext2D, b: InvaderBullet) => {
    ctx.save();
    ctx.fillStyle = "#ff0066";
    ctx.shadowColor = "#ff0066";
    ctx.shadowBlur = 6;
    ctx.fillRect(b.x - 2, b.y, 4, 10);
    ctx.restore();
  }, []);

  const drawStars = useCallback((ctx: CanvasRenderingContext2D, time: number) => {
    ctx.save();
    ctx.fillStyle = "#ffffff";
    for (let i = 0; i < 80; i++) {
      const sx = ((i * 73 + time * 0.02) % CANVAS_W);
      const sy = ((i * 137) % CANVAS_H);
      const alpha = 0.3 + 0.7 * Math.abs(Math.sin(time * 0.001 + i));
      ctx.globalAlpha = alpha;
      ctx.fillRect(sx, sy, 1.5, 1.5);
    }
    ctx.restore();
  }, []);

  // ─── Game loop ───────────────────────────────────────────────────────────
  const gameLoop = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const s = stateRef.current;
    const keys = keysRef.current;
    const now = performance.now();

    // ── Draw background ──
    ctx.fillStyle = "#0a0a1a";
    ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);
    drawStars(ctx, now);

    // ── Game state: idle ──
    if (s.gameState === "idle") {
      // Draw title
      ctx.save();
      ctx.textAlign = "center";
      ctx.font = "bold 40px monospace";
      ctx.fillStyle = "#00ffcc";
      ctx.shadowColor = "#00ffcc";
      ctx.shadowBlur = 20;
      ctx.fillText("SPACE INVADERS", CANVAS_W / 2, CANVAS_H / 2 - 40);
      ctx.font = "18px monospace";
      ctx.fillStyle = "#aabbcc";
      ctx.shadowBlur = 0;
      ctx.fillText("Arrow Keys / A-D to move", CANVAS_W / 2, CANVAS_H / 2 + 10);
      ctx.fillText("Spacebar to shoot", CANVAS_W / 2, CANVAS_H / 2 + 35);
      ctx.restore();
      animRef.current = requestAnimationFrame(gameLoop);
      return;
    }

    // ── Game state: gameover / victory ──
    if (s.gameState === "gameover" || s.gameState === "victory") {
      // Draw game elements behind overlay
      s.invaders.forEach((inv) => {
        if (inv.alive) drawInvader(ctx, inv);
      });
      drawPlayer(ctx, s.playerX);

      ctx.save();
      ctx.textAlign = "center";
      if (s.gameState === "gameover") {
        ctx.font = "bold 48px monospace";
        ctx.fillStyle = "#ff3366";
        ctx.shadowColor = "#ff3366";
        ctx.shadowBlur = 20;
        ctx.fillText("GAME OVER", CANVAS_W / 2, CANVAS_H / 2 - 20);
      } else {
        ctx.font = "bold 48px monospace";
        ctx.fillStyle = "#00ffcc";
        ctx.shadowColor = "#00ffcc";
        ctx.shadowBlur = 20;
        ctx.fillText("VICTORY!", CANVAS_W / 2, CANVAS_H / 2 - 20);
      }
      ctx.font = "20px monospace";
      ctx.fillStyle = "#ffffff";
      ctx.shadowBlur = 0;
      ctx.fillText(`Score: ${s.score}`, CANVAS_W / 2, CANVAS_H / 2 + 20);
      ctx.font = "16px monospace";
      ctx.fillStyle = "#aabbcc";
      ctx.fillText("Press R or click button to restart", CANVAS_W / 2, CANVAS_H / 2 + 50);
      ctx.restore();
      animRef.current = requestAnimationFrame(gameLoop);
      return;
    }

    // ── Game state: playing ──
    // ── Player movement ──
    if (keys.has("ArrowLeft") || keys.has("a") || keys.has("A")) {
      s.playerX = Math.max(0, s.playerX - PLAYER_SPEED);
    }
    if (keys.has("ArrowRight") || keys.has("d") || keys.has("D")) {
      s.playerX = Math.min(CANVAS_W - PLAYER_W, s.playerX + PLAYER_SPEED);
    }

    // ── Shooting cooldown ──
    if (keys.has(" ") && now - lastShotRef.current > 250) {
      lastShotRef.current = now;
      s.bullets.push({ x: s.playerX + PLAYER_W / 2 - 2, y: PLAYER_Y - PLAYER_H } as any);
    }

    // ── Update player bullets ──
    s.bullets = s.bullets.filter((b) => {
      b.y += BULLET_SPEED;
      return b.y > 0;
    });

    // ── Update invader bullets ──
    s.invBullets = s.invBullets.filter((b) => {
      b.y += INV_BULLET_SPEED;
      return b.y < CANVAS_H;
    });

    // ── Invader movement ──
    s.invMoveTimer++;
    const aliveInvaders = s.invaders.filter((inv) => inv.alive);
    const moveInterval = Math.max(8, s.invMoveInterval - (INVADER_COLS * INVADER_ROWS - aliveInvaders.length));

    if (s.invMoveTimer >= moveInterval) {
      s.invMoveTimer = 0;
      let edgeHit = false;
      aliveInvaders.forEach((inv) => {
        inv.x += s.invDir * (INVADER_W + INVADER_GAP_X);
        if (inv.x <= 4 || inv.x + INVADER_W >= CANVAS_W - 4) edgeHit = true;
      });
      if (edgeHit) {
        s.invDir *= -1;
        aliveInvaders.forEach((inv) => {
          inv.y += INVADER_H + INVADER_GAP_Y;
        });
      }
    }

    // ── Invader shooting ──
    if (now - lastInvShotRef.current > 800 && aliveInvaders.length > 0) {
      lastInvShotRef.current = now;
      // Pick random alive invader from bottom row
      const bottomInvaders: Invader[] = [];
      for (let c = 0; c < INVADER_COLS; c++) {
        for (let r = INVADER_ROWS - 1; r >= 0; r--) {
          const idx = r * INVADER_COLS + c;
          if (s.invaders[idx].alive) {
            bottomInvaders.push(s.invaders[idx]);
            break;
          }
        }
      }
      if (bottomInvaders.length > 0) {
        const shooter = bottomInvaders[Math.floor(Math.random() * bottomInvaders.length)];
         s.invBullets.push({ x: shooter.x + INVADER_W / 2 - 2, y: shooter.y + INVADER_H } as any);
      }
    }

    // ── Collision: player bullets vs invaders ──
    for (let i = s.bullets.length - 1; i >= 0; i--) {
      const b = s.bullets[i];
      let hit = false;
      for (let j = 0; j < s.invaders.length; j++) {
        const inv = s.invaders[j];
        if (inv.alive && rectsOverlap(b.x, b.y, 4, 10, inv.x, inv.y, INVADER_W, INVADER_H)) {
          inv.alive = false;
          hit = true;
          s.score += (3 - inv.type + 1) * 10;
          break;
        }
      }
      if (hit) {
        s.bullets.splice(i, 1);
      }
    }

    // ── Collision: invader bullets vs player ──
    for (let i = s.invBullets.length - 1; i >= 0; i--) {
      const b = s.invBullets[i];
      if (rectsOverlap(b.x, b.y, 4, 10, s.playerX, PLAYER_Y - PLAYER_H, PLAYER_W, PLAYER_H)) {
        s.invBullets.splice(i, 1);
        s.lives--;
        s.playerHitFlash = 30;
        if (s.lives <= 0) {
          s.gameState = "gameover";
        }
      }
    }

    // ── Collision: invaders reaching player ──
    for (const inv of aliveInvaders) {
      if (inv.y + INVADER_H >= PLAYER_Y - PLAYER_H) {
        s.gameState = "gameover";
        break;
      }
    }

    // ── Check victory ──
    if (aliveInvaders.length === 0) {
      s.gameState = "victory";
    }

    // ── Draw everything ──
    // Invaders
    s.invaders.forEach((inv) => {
      if (inv.alive) drawInvader(ctx, inv);
    });

    // Player bullets
    s.bullets.forEach((b) => drawBullet(ctx, b));

    // Invader bullets
    s.invBullets.forEach((b) => drawInvBullet(ctx, b));

    // Player
    if (s.playerHitFlash > 0) {
      s.playerHitFlash--;
      if (s.playerHitFlash % 4 < 2) {
        drawPlayer(ctx, s.playerX);
      }
    } else {
      drawPlayer(ctx, s.playerX);
    }

    // ── HUD ──
    ctx.save();
    ctx.font = "bold 16px monospace";
    ctx.fillStyle = "#00ffcc";
    ctx.textAlign = "left";
    ctx.shadowBlur = 0;
    ctx.fillText(`SCORE: ${s.score}`, 10, 20);
    ctx.textAlign = "right";
    ctx.fillText(`LIVES: ${s.lives}`, CANVAS_W - 10, 20);
    ctx.restore();

    // ── Update snapshot ──
    setSnapshot({
      playerX: s.playerX,
      score: s.score,
      lives: s.lives,
      gameState: s.gameState,
      invaderCount: aliveInvaders.length,
    });

    animRef.current = requestAnimationFrame(gameLoop);
  }, [drawPlayer, drawInvader, drawBullet, drawInvBullet, drawStars]);

  // ─── Keyboard events ─────────────────────────────────────────────────────
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      keysRef.current.add(e.key);
      if (e.key === "r" || e.key === "R") {
        const s = stateRef.current;
        if (s.gameState === "gameover" || s.gameState === "victory") {
          restartGame();
        }
      }
      // Prevent scrolling
      if ([" ", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
        e.preventDefault();
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      keysRef.current.delete(e.key);
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [restartGame]);

  // ─── Game loop lifecycle ─────────────────────────────────────────────────
  useEffect(() => {
    animRef.current = requestAnimationFrame(gameLoop);
    return () => cancelAnimationFrame(animRef.current);
  }, [gameLoop]);

  return (
    <div style={{
      display: "flex",
      flexDirection: "column",
      alignItems: "center",
      justifyContent: "center",
      minHeight: "100vh",
      backgroundColor: "#0a0a1a",
      fontFamily: "monospace",
      color: "#ffffff",
    }}>
      {/* HUD overlay */}
      <div style={{
        display: "flex",
        justifyContent: "space-between",
        width: CANVAS_W,
        marginBottom: 8,
        fontSize: 14,
        color: "#00ffcc",
      }}>
        <span>SCORE: {snapshot.score}</span>
        <span>LIVES: {"♥".repeat(snapshot.lives)}</span>
      </div>

      {/* Canvas */}
      <div style={{ position: "relative" }}>
        <canvas
          ref={canvasRef}
          width={CANVAS_W}
          height={CANVAS_H}
          style={{
            border: "2px solid #00ffcc",
            borderRadius: 4,
            boxShadow: "0 0 30px rgba(0,255,204,0.3)",
            display: "block",
          }}
          data-anvil-state={JSON.stringify({
            playerX: snapshot.playerX,
            score: snapshot.score,
            lives: snapshot.lives,
            gameState: snapshot.gameState,
            invaderCount: snapshot.invaderCount,
          })}
        />

        {/* Game Over / Victory overlay buttons */}
        {(snapshot.gameState === "gameover" || snapshot.gameState === "victory") && (
          <div style={{
            position: "absolute",
            top: 0,
            left: 0,
            width: CANVAS_W,
            height: CANVAS_H,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            backgroundColor: "rgba(10,10,26,0.7)",
            borderRadius: 4,
          }}>
            <button
              onClick={restartGame}
              data-anvil-action="restart"
              style={{
                padding: "12px 32px",
                fontSize: 18,
                fontFamily: "monospace",
                backgroundColor: "#00ffcc",
                color: "#0a0a1a",
                border: "none",
                borderRadius: 4,
                cursor: "pointer",
                fontWeight: "bold",
                boxShadow: "0 0 20px rgba(0,255,204,0.5)",
              }}
            >
              {snapshot.gameState === "gameover" ? "TRY AGAIN" : "PLAY AGAIN"}
            </button>
          </div>
        )}
      </div>

      {/* Start button for idle state */}
      {snapshot.gameState === "idle" && (
        <button
          onClick={startGame}
          data-anvil-action="primary"
          style={{
            marginTop: 16,
            padding: "14px 40px",
            fontSize: 20,
            fontFamily: "monospace",
            backgroundColor: "#00ffcc",
            color: "#0a0a1a",
            border: "none",
            borderRadius: 4,
            cursor: "pointer",
            fontWeight: "bold",
            boxShadow: "0 0 20px rgba(0,255,204,0.5)",
          }}
        >
          START GAME
        </button>
      )}

      {/* In-play restart button */}
      {snapshot.gameState === "playing" && snapshot.lives <= 0 && (
        <button
          onClick={restartGame}
          data-anvil-action="restart"
          style={{
            marginTop: 12,
            padding: "10px 28px",
            fontSize: 14,
            fontFamily: "monospace",
            backgroundColor: "#ff3366",
            color: "#ffffff",
            border: "none",
            borderRadius: 4,
            cursor: "pointer",
          }}
        >
          RESTART
        </button>
      )}

      {/* Controls hint */}
      <div style={{
        marginTop: 12,
        fontSize: 12,
        color: "#667788",
        textAlign: "center",
      }}>
        ← → / A D to move &nbsp;|&nbsp; SPACE to shoot &nbsp;|&nbsp; R to restart
      </div>
    </div>
  );
}
