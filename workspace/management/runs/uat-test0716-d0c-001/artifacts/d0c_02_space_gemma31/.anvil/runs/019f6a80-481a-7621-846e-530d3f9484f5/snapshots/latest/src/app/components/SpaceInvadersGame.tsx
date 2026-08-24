"use client";

import React, { useState, useEffect, useRef } from "react";

type GameStatus = "MENU" | "PLAYING" | "GAME_OVER" | "VICTORY";

interface Entity {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface Bullet extends Entity {
  velocity: number;
}

interface Enemy extends Entity {
  id: number;
  alive: boolean;
}

const GAME_WIDTH = 800;
const GAME_HEIGHT = 600;
const PLAYER_WIDTH = 40;
const PLAYER_HEIGHT = 20;
const ENEMY_WIDTH = 30;
const ENEMY_HEIGHT = 20;
const BULLET_WIDTH = 4;
const BULLET_HEIGHT = 10;
const ENEMY_ROWS = 5;
const ENEMY_COLS = 11;
const PLAYER_SPEED = 5;
const BULLET_SPEED = 7;
const ENEMY_BULLET_SPEED = 3;

export default function SpaceInvadersGame() {
  const [gameStatus, setGameStatus] = useState<GameStatus>("MENU");
  const [score, setScore] = useState(0);
  const [playerX, setPlayerX] = useState(GAME_WIDTH / 2 - PLAYER_WIDTH / 2);

  // Refs for game loop to avoid React render cycle latency
  const requestRef = useRef<number>();
  const gameStateRef = useRef({
    playerX: GAME_WIDTH / 2 - PLAYER_WIDTH / 2,
    bullets: [] as Bullet[],
    enemyBullets: [] as Bullet[],
    enemies: [] as Enemy[],
    enemyDirection: 1,
    enemyMoveTimer: 0,
    lastShotTime: 0,
    score: 0,
    status: "MENU" as GameStatus,
  });

  const keysPressed = useRef<Set<string>>(new Set());

  const initGame = () => {
    const enemies: Enemy[] = [];
    for (let row = 0; row < ENEMY_ROWS; row++) {
      for (let col = 0; col < ENEMY_COLS; col++) {
        enemies.push({
          id: row * ENEMY_COLS + col,
          x: col * (ENEMY_WIDTH + 15) + 50,
          y: row * (ENEMY_HEIGHT + 15) + 50,
          width: ENEMY_WIDTH,
          height: ENEMY_HEIGHT,
          alive: true,
        });
      }
    }

    gameStateRef.current = {
      playerX: GAME_WIDTH / 2 - PLAYER_WIDTH / 2,
      bullets: [],
      enemyBullets: [],
      enemies,
      enemyDirection: 1,
      enemyMoveTimer: 0,
      lastShotTime: 0,
      score: 0,
      status: "PLAYING",
    };

    setPlayerX(GAME_WIDTH / 2 - PLAYER_WIDTH / 2);
    setScore(0);
    setGameStatus("PLAYING");
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    keysPressed.current.add(e.code);
    if (e.code === "KeyR" && gameStateRef.current.status === "PLAYING") {
      initGame();
    }
  };

  const handleKeyUp = (e: KeyboardEvent) => {
    keysPressed.current.delete(e.code);
  };

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, []);

  const update = (time: number) => {
    if (gameStateRef.current.status !== "PLAYING") return;

    const state = gameStateRef.current;

    // Player Movement
    if (keysPressed.current.has("ArrowLeft") && state.playerX > 0) {
      state.playerX -= PLAYER_SPEED;
    }
    if (keysPressed.current.has("ArrowRight") && state.playerX < GAME_WIDTH - PLAYER_WIDTH) {
      state.playerX += PLAYER_SPEED;
    }

    // Shooting
    if (keysPressed.current.has("Space")) {
      const now = Date.now();
      if (now - state.lastShotTime > 400) {
        state.bullets.push({
          x: state.playerX + PLAYER_WIDTH / 2 - BULLET_WIDTH / 2,
          y: GAME_HEIGHT - PLAYER_HEIGHT - 20,
          width: BULLET_WIDTH,
          height: BULLET_HEIGHT,
          velocity: -BULLET_SPEED,
        });
        state.lastShotTime = now;
      }
    }

    // Update Player X in React state for data-anvil-state and rendering (simplified)
    setPlayerX(state.playerX);

    // Move Bullets
    state.bullets = state.bullets.filter((b) => b.y + b.height > 0);
    state.bullets.forEach((b) => (b.y += b.velocity));

    state.enemyBullets = state.enemyBullets.filter((b) => b.y < GAME_HEIGHT);
    state.enemyBullets.forEach((b) => (b.y += ENEMY_BULLET_SPEED));

    // Move Enemies
    let shiftDown = false;
    const aliveEnemies = state.enemies.filter((e) => e.alive);
    if (aliveEnemies.length === 0) {
      setGameStatus("VICTORY");
      state.status = "VICTORY";
      return;
    }

    const rightEdge = Math.max(...aliveEnemies.map((e) => e.x + e.width));
    const leftEdge = Math.min(...aliveEnemies.map((e) => e.x));

    if (rightEdge >= GAME_WIDTH - 10 || leftEdge <= 10) {
      state.enemyDirection *= -1;
      shiftDown = true;
    }

    aliveEnemies.forEach((e) => {
      e.x += state.enemyDirection * 2;
      if (shiftDown) e.y += 20;
      if (e.y + e.height >= GAME_HEIGHT - PLAYER_HEIGHT - 10) {
        setGameStatus("GAME_OVER");
        state.status = "GAME_OVER";
      }
    });

    // Enemy Shooting
    if (Math.random() < 0.02 && aliveEnemies.length > 0) {
      const shooter = aliveEnemies[Math.floor(Math.random() * aliveEnemies.length)];
      state.enemyBullets.push({
        x: shooter.x + ENEMY_WIDTH / 2 - BULLET_WIDTH / 2,
        y: shooter.y + ENEMY_HEIGHT,
        width: BULLET_WIDTH,
        height: BULLET_HEIGHT,
        velocity: ENEMY_BULLET_SPEED,
      });
    }

    // Collisions: Player Bullet vs Enemy
    state.bullets.forEach((b) => {
      aliveEnemies.forEach((e) => {
        if (
          b.x < e.x + e.width &&
          b.x + b.width > e.x &&
          b.y < e.y + e.height &&
          b.y + b.height > e.y
        ) {
          e.alive = false;
          b.y = -100; // mark for removal
          state.score += 10;
          setScore(state.score);
        }
      });
    });

    // Collisions: Enemy Bullet vs Player
    state.enemyBullets.forEach((eb) => {
      if (
        eb.x < state.playerX + PLAYER_WIDTH &&
        eb.x + eb.width > state.playerX &&
        eb.y < GAME_HEIGHT - PLAYER_HEIGHT - 10 &&
        eb.y + eb.height > GAME_HEIGHT - PLAYER_HEIGHT - 20 // approximate player pos
      ) {
        setGameStatus("GAME_OVER");
        state.status = "GAME_OVER";
      }
    });

    requestRef.current = requestAnimationFrame(update);
  };

  useEffect(() => {
    if (gameStatus === "PLAYING") {
      requestRef.current = requestAnimationFrame(update);
    }
    return () => {
      if (requestRef.current) cancelAnimationFrame(requestRef.current);
    };
  }, [gameStatus]);

  const enemyCount = gameStateRef.current.enemies.filter((e) => e.alive).length;

  return (
    <div 
      className="flex flex-col items-center justify-center min-h-screen bg-slate-900 text-white font-mono"
      data-anvil-state={JSON.stringify({ playerX, score, gameStatus, enemyCount })}
    >
      <h1 className="text-4xl mb-4 font-bold tracking-widest text-green-500">SPACE INVADERS</h1>
      <div className="mb-2 text-xl">Score: {score}</div>

      <div 
        className="relative overflow-hidden bg-black border-4 border-slate-700 shadow-2xl"
        style={{ width: GAME_WIDTH, height: GAME_HEIGHT }}
      >
        {gameStatus === "MENU" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/80 z-10">
            <p className="mb-6 text-lg">Use Left/Right Arrows to Move, Space to Shoot</p>
            <button 
              data-anvil-action="primary"
              onClick={initGame} 
              className="px-6 py-3 bg-green-600 hover:bg-green-500 text-white font-bold rounded-lg transition-colors uppercase tracking-wider"
            >
              Start Mission
            </button>
          </div>
        )}

        {gameStatus === "GAME_OVER" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-red-900/80 z-10">
            <h2 className="text-6xl font-bold mb-4 text-white">MISSION FAILED</h2>
            <p className="mb-6 text-xl">The Earth has fallen.</p>
            <button 
              data-anvil-action="restart"
              onClick={initGame} 
              className="px-6 py-3 bg-white text-red-900 font-bold rounded-lg hover:bg-slate-200 transition-colors uppercase tracking-wider"
            >
              Try Again
            </button>
          </div>
        )}

        {gameStatus === "VICTORY" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-green-900/80 z-10">
            <h2 className="text-6xl font-bold mb-4 text-white">VICTORY!</h2>
            <p className="mb-6 text-xl">You saved the galaxy!</p>
            <button 
              data-anvil-action="restart"
              onClick={initGame} 
              className="px-6 py-3 bg-white text-green-900 font-bold rounded-lg hover:bg-slate-200 transition-colors uppercase tracking-wider"
            >
              Play Again
            </button>
          </div>
        )}

        {/* Game Elements Rendering */}
        <div 
          className="absolute bottom-[20px] bg-green-500 rounded-t-lg"
          style={{ 
            left: playerX, 
            width: PLAYER_WIDTH, 
            height: PLAYER_HEIGHT,
            transition: 'left 0.05s linear'
          }}
        />

        {gameStateRef.current.bullets.map((b, i) => (
          <div 
            key={`bullet-${i}`}
            className="absolute bg-white"
            style={{ left: b.x, top: b.y, width: b.width, height: b.height }}
          />
        ))}

        {gameStateRef.current.enemyBullets.map((eb, i) => (
          <div 
            key={`enemy-bullet-${i}`}
            className="absolute bg-red-500"
            style={{ left: eb.x, top: eb.y, width: eb.width, height: eb.height }}
          />
        ))}

        {gameStateRef.current.enemies.filter(e => e.alive).map((e) => (
          <div 
            key={`enemy-${e.id}`}
            className="absolute bg-purple-500 rounded-sm border border-purple-300"
            style={{ left: e.x, top: e.y, width: e.width, height: e.height }}
          />
        ))}
      </div>
      <p className="mt-4 text-slate-400">Press 'R' to restart at any time</p>
    </div>
  );
}
