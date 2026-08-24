"use client";

import React, { useEffect, useRef, useState } from "react";

type GameState = "START" | "PLAYING" | "GAME_OVER" | "VICTORY";

interface Alien {
  x: number;
  y: number;
  alive: boolean;
}

interface Projectile {
  x: number;
  y: number;
  dy: number;
}

export default function SpaceInvaders() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  
  // Game state mirrored to React state for data-anvil observability.
  // This ensures that the DOM attribute data-anvil-state is updated on every render.
  const [gameSnapshot, setGameSnapshot] = useState({
    gameState: "START" as GameState,
    score: 0,
    lives: 3,
    level: 1,
    playerX: 375,
    alienPositions: [] as { x: number; y: number }[],
  });

  // Core game state using refs for high-performance loop to avoid React render bottlenecks.
  const stateRef = useRef({
    gameState: "START" as GameState,
    score: 0,
    lives: 3,
    level: 1,
    playerX: 375,
    playerY: 540,
    projectiles: [] as Projectile[],
    alienProjectiles: [] as Projectile[],
    aliens: [] as Alien[],
    alienDirection: 1,
    alienMoveTimer: 0,
    lastShotTime: 0,
  });

  const keysPressed = useRef<{ [key: string]: boolean }>({});

  // Constants
  const CANVAS_WIDTH = 750;
  const CANVAS_HEIGHT = 600;
  const PLAYER_WIDTH = 50;
  const PLAYER_HEIGHT = 30;
  const ALIEN_ROWS = 5;
  const ALIEN_COLS = 11;
  const ALIEN_WIDTH = 40;
  const ALIEN_HEIGHT = 30;
  const ALIEN_SPACING = 15;

  const initAliens = () => {
    const aliens: Alien[] = [];
    for (let row = 0; row < ALIEN_ROWS; row++) {
      for (let col = 0; col < ALIEN_COLS; col++) {
        aliens.push({
          x: col * (ALIEN_WIDTH + ALIEN_SPACING) + 50,
          y: row * (ALIEN_HEIGHT + ALIEN_SPACING) + 50,
          alive: true,
        });
      }
    }
    return aliens;
  };

  const updateSnapshot = () => {
    const s = stateRef.current;
    setGameSnapshot({
      gameState: s.gameState,
      score: s.score,
      lives: s.lives,
      level: s.level,
      playerX: s.playerX,
      alienPositions: s.aliens
        .filter((a) => a.alive)
        .map((a) => ({ x: a.x, y: a.y })),
    });
  };

  const startGame = () => {
    stateRef.current = {
      gameState: "PLAYING",
      score: 0,
      lives: 3,
      level: 1,
      playerX: CANVAS_WIDTH / 2 - PLAYER_WIDTH / 2,
      playerY: CANVAS_HEIGHT - 60,
      projectiles: [],
      alienProjectiles: [],
      aliens: initAliens(),
      alienDirection: 1,
      alienMoveTimer: 0,
      lastShotTime: 0,
    };
    updateSnapshot();
  };

  const restartGame = () => {
    startGame();
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      keysPressed.current[e.code] = true;
      if (e.code === "KeyR") {
        restartGame();
      }
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      keysPressed.current[e.code] = false;
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let animationFrameId: number;

    const gameLoop = () => {
      const s = stateRef.current;

      if (s.gameState === "PLAYING") {
        // 1. Player Movement
        if (keysPressed.current["ArrowLeft"] && s.playerX > 0) {
          s.playerX -= 5;
        }
        if (keysPressed.current["ArrowRight"] && s.playerX < CANVAS_WIDTH - PLAYER_WIDTH) {
          s.playerX += 5;
        }

        // 2. Player Shooting
        if (keysPressed.current["Space"]) {
          const now = Date.now();
          if (now - s.lastShotTime > 400) {
            s.projectiles.push({ x: s.playerX + PLAYER_WIDTH / 2, y: s.playerY, dy: -7 });
            s.lastShotTime = now;
          }
        }

        // 3. Alien Movement Logic
        let moveDown = false;
        const aliveAliens = s.aliens.filter((a) => a.alive);
        if (aliveAliens.length === 0) {
          s.gameState = "VICTORY";
        } else {
          for (const a of aliveAliens) {
            if ((s.alienDirection === 1 && a.x + ALIEN_WIDTH > CANVAS_WIDTH - 20) ||
                (s.alienDirection === -1 && a.x < 20)) {
              moveDown = true;
              break;
            }
          }

          if (moveDown) {
            s.alienDirection *= -1;
            s.aliens.forEach((a) => { if(a.alive) a.y += 20 });
          } else {
            s.aliens.forEach((a) => { if(a.alive) a.x += s.alienDirection * (1 + s.level * 0.5); });
          }

          if (Math.random() < 0.02 + s.level * 0.005) {
            const shooters = aliveAliens;
            const shooter = shooters[Math.floor(Math.random() * shooters.length)];
            s.alienProjectiles.push({ x: shooter.x + ALIEN_WIDTH / 2, y: shooter.y + ALIEN_HEIGHT, dy: 4 });
          }
        }

        // 4. Projectile Updates & Collision Detection
        s.projectiles = s.projectiles.filter((p) => {
          p.y += p.dy;
          let hit = false;
          for (const a of s.aliens) {
            if (a.alive && p.x > a.x && p.x < a.x + ALIEN_WIDTH && p.y > a.y && p.y < a.y + ALIEN_HEIGHT) {
              a.alive = false;
              s.score += 10 * s.level;
              hit = true;
              break;
            }
          }
          return !hit && p.y > 0;
        });

        s.alienProjectiles = s.alienProjectiles.filter((p) => {
          p.y += p.dy;
          if (p.x > s.playerX && p.x < s.playerX + PLAYER_WIDTH && p.y > s.playerY && p.y < s.playerY + PLAYER_HEIGHT) {
            s.lives--;
            if (s.lives <= 0) s.gameState = "GAME_OVER";
            return false;
          }
          return p.y < CANVAS_HEIGHT;
        });

        for (const a of aliveAliens) {
          if (a.y + ALIEN_HEIGHT >= s.playerY) {
            s.gameState = "GAME_OVER";
          }
        }

        updateSnapshot();
      }

      // Rendering logic
      ctx.fillStyle = "#000";
      ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

      if (s.gameState === "PLAYING") {
        ctx.fillStyle = "#0f0";
        ctx.fillRect(s.playerX, s.playerY, PLAYER_WIDTH, PLAYER_HEIGHT);
        ctx.fillRect(s.playerX + PLAYER_WIDTH / 2 - 5, s.playerY - 10, 10, 10);

        ctx.fillStyle = "#f0f";
        for (const a of s.aliens) {
          if (a.alive) {
            ctx.fillRect(a.x, a.y, ALIEN_WIDTH, ALIEN_HEIGHT);
            ctx.fillStyle = "#fff";
            ctx.fillRect(a.x + 5, a.y + 5, 5, 5);
            ctx.fillRect(a.x + ALIEN_WIDTH - 10, a.y + 5, 5, 5);
            ctx.fillStyle = "#f0f";
          }
        }

        ctx.fillStyle = "#fff";
        s.projectiles.forEach((p) => ctx.fillRect(p.x - 2, p.y, 4, 10));
        ctx.fillStyle = "#f00";
        s.alienProjectiles.forEach((p) => ctx.fillRect(p.x - 2, p.y, 4, 10));

        ctx.fillStyle = "#fff";
        ctx.font = "20px monospace";
        ctx.fillText(`Score: ${s.score}`, 20, 30);
        ctx.fillText(`Lives: ${s.lives}`, CANVAS_WIDTH - 120, 30);
        ctx.fillText(`Level: ${s.level}`, CANVAS_WIDTH / 2 - 40, 30);
      }

      animationFrameId = requestAnimationFrame(gameLoop);
    };

    animationFrameId = requestAnimationFrame(gameLoop);
    return () => cancelAnimationFrame(animationFrameId);
  }, []);

  return (
    <div className="min-h-screen bg-slate-950 flex items-center justify-center p-4 font-mono text-white">
      <div 
        className="relative" 
        data-anvil-state={JSON.stringify(gameSnapshot)}
      >
        <canvas 
          ref={canvasRef} 
          width={CANVAS_WIDTH} 
          height={CANVAS_HEIGHT} 
          className="border-4 border-slate-700 rounded-lg shadow-2xl bg-black"
        />

        {gameSnapshot.gameState === "START" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/80 text-center p-6">
            <h1 className="text-5xl font-bold mb-4 text-green-400 italic uppercase tracking-widest">Space Invaders</h1>
            <p className="mb-8 text-slate-300">Use ARROWS to move, SPACE to shoot</p>
            <button 
              onClick={startGame}
              data-anvil-action="primary"
              className="px-8 py-4 bg-green-600 hover:bg-green-500 text-white rounded-full font-bold text-xl transition-all transform hover:scale-110 active:scale-95 shadow-[0_0_20px_rgba(34,197,94,0.5)]"
            >
              START GAME
            </button>
          </div>
        )}

        {gameSnapshot.gameState === "GAME_OVER" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-red-900/80 text-center p-6">
            <h2 className="text-6xl font-bold mb-4 text-white uppercase tracking-tighter">GAME OVER</h2>
            <p className="text-2xl mb-8">Final Score: {gameSnapshot.score}</p>
            <button 
              onClick={restartGame}
              data-anvil-action="restart"
              className="px-8 py-4 bg-white text-red-900 rounded-full font-bold text-xl transition-all transform hover:scale-110 active:scale-95"
            >
              TRY AGAIN
            </button>
          </div>
        )}

        {gameSnapshot.gameState === "VICTORY" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-green-900/80 text-center p-6">
            <h2 className="text-6xl font-bold mb-4 text-white uppercase tracking-tighter">VICTORY!</h2>
            <p className="text-2xl mb-8">You saved the galaxy!</p>
            <button 
              onClick={restartGame}
              data-anvil-action="restart"
              className="px-8 py-4 bg-white text-green-900 rounded-full font-bold text-xl transition-all transform hover:scale-110 active:scale-95"
            >
              PLAY AGAIN
            </button>
          </div>
        )}

        {gameSnapshot.gameState === "PLAYING" && (
          <div className="absolute bottom-4 right-4 opacity-60 text-xs flex items-center gap-2">
            <span>Press [R] to Restart</span> 
            <button 
              onClick={restartGame}
              data-anvil-action="restart"
              className="bg-slate-800 px-2 py-1 rounded hover:bg-slate-700 transition-colors border border-slate-600"
            >
              Reset
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
