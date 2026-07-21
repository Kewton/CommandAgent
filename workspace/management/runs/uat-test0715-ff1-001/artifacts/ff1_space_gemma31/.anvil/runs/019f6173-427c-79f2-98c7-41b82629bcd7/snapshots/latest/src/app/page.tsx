"use client";

import React, { useEffect, useRef, useState } from 'react';

type GameState = 'START' | 'PLAYING' | 'GAME_OVER' | 'VICTORY';

interface Entity {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface Bullet extends Entity {
  velocity: number;
}

interface Alien extends Entity {
  alive: boolean;
  type: number;
}

const CANVAS_WIDTH = 800;
const CANVAS_HEIGHT = 600;
const PLAYER_WIDTH = 50;
const PLAYER_HEIGHT = 30;
const ALIEN_ROWS = 5;
const ALIEN_COLS = 11;
const ALIEN_WIDTH = 40;
const ALIEN_HEIGHT = 30;
const ALIEN_PADDING = 15;
const BULLET_SPEED = 7;
const ALIEN_BULLET_SPEED = 4;

export default function SpaceInvaders() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [gameState, setGameState] = useState<GameState>('START');
  const [score, setScore] = useState(0);
  const [lives, setLives] = useState(3);
  const [level, setLevel] = useState(1);

  // Game refs to avoid React render cycles for the loop
  const gameRef = useRef({
    playerX: CANVAS_WIDTH / 2 - PLAYER_WIDTH / 2,
    bullets: [] as Bullet[],
    alienBullets: [] as Bullet[],
    aliens: [] as Alien[],
    alienDirection: 1,
    alienMoveTimer: 0,
    lastShotTime: 0,
    keys: {} as Record<string, boolean>,
  });

  const initAliens = (lvl: number) => {
    const aliens: Alien[] = [];
    for (let row = 0; row < ALIEN_ROWS; row++) {
      for (let col = 0; col < ALIEN_COLS; col++) {
        aliens.push({
          x: col * (ALIEN_WIDTH + ALIEN_PADDING) + 50,
          y: row * (ALIEN_HEIGHT + ALIEN_PADDING) + 50,
          width: ALIEN_WIDTH,
          height: ALIEN_HEIGHT,
          alive: true,
          type: Math.floor(row / 2),
        });
      }
    }
    return aliens;
  };

  const resetGame = (lvl = 1) => {
    gameRef.current = {
      ...gameRef.current,
      playerX: CANVAS_WIDTH / 2 - PLAYER_WIDTH / 2,
      bullets: [],
      alienBullets: [],
      aliens: initAliens(lvl),
      alienDirection: 1,
      alienMoveTimer: 0,
      lastShotTime: 0,
      keys: {},
    };
    setScore(0);
    setLives(3);
    setLevel(lvl);
    setGameState('PLAYING');
  };

  const startGame = () => {
    resetGame();
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      gameRef.current.keys[e.code] = true;
      if (e.code === 'KeyR') {
        // Restart functionality as requested
        resetGame(level); 
        // Note: In a real game we might just restart the current level or full game.
        // Here, resetting fully for clarity of "recoverable state".
        setGameState('PLAYING');
      }
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      gameRef.current.keys[e.code] = false;
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, [level]);

  useEffect(() => {
    if (gameState !== 'PLAYING') return;

    let animationFrameId: number;
    const ctx = canvasRef.current?.getContext('2d');
    if (!ctx) return;

    const update = () => {
      const state = gameRef.current;

      // Player movement
      if (state.keys['ArrowLeft']) state.playerX -= 5;
      if (state.keys['ArrowRight']) state.playerX += 5;
      state.playerX = Math.max(0, Math.min(CANVAS_WIDTH - PLAYER_WIDTH, state.playerX));

      // Shooting
      if (state.keys['Space']) {
        const now = Date.now();
        if (now - state.lastShotTime > 500) {
          state.bullets.push({
            x: state.playerX + PLAYER_WIDTH / 2 - 2,
            y: CANVAS_HEIGHT - 40,
            width: 4,
            height: 10,
            velocity: -BULLET_SPEED,
          });
          state.lastShotTime = now;
        }
      }

      // Update bullets
      state.bullets.forEach((b, i) => {
        b.y += b.velocity;
        if (b.y < 0) state.bullets.splice(i, 1);
      });

      state.alienBullets.forEach((b, i) => {
        b.y += b.velocity;
        if (b.y > CANVAS_HEIGHT) state.alienBullets.splice(i, 1);
      });

      // Alien movement logic
      let moveDown = false;
      state.aliens.forEach(a => {
        if (!a.alive) return;
        if ((state.alienDirection === 1 && a.x + ALIEN_WIDTH > CANVAS_WIDTH - 20) ||
            (state.alienDirection === -1 && a.x < 20)) {
          moveDown = true;
        }
      });

      if (moveDown) {
        state.alienDirection *= -1;
        state.aliens.forEach(a => { a.y += 20; });
      } else {
        state.aliens.forEach(a => { if (a.alive) a.x += state.alienDirection * 2; });
      }

      // Alien shooting
      if (Math.random() < 0.02) {
        const aliveAliens = state.aliens.filter(a => a.alive);
        if (aliveAliens.length > 0) {
          const shooter = aliveAliens[Math.floor(Math.random() * aliveAliens.length)];
          state.alienBullets.push({
            x: shooter.x + ALIEN_WIDTH / 2,
            y: shooter.y + ALIEN_HEIGHT,
            width: 4,
            height: 10,
            velocity: ALIEN_BULLET_SPEED,
          });
        }
      }

      // Collision detection: Bullet -> Alien
      state.bullets.forEach((b, bi) => {
        state.aliens.forEach((a, ai) => {
          if (a.alive && b.x < a.x + a.width && b.x + b.width > a.x && b.y < a.y + a.height && b.y + b.height > a.y) {
            a.alive = false;
            state.bullets.splice(bi, 1);
            setScore(s => s + 10 * (level));
          }
        });
      });

      // Collision detection: Alien Bullet -> Player
      state.alienBullets.forEach((b, bi) => {
        if (b.x < state.playerX + PLAYER_WIDTH && b.x + b.width > state.playerX && b.y < CANVAS_HEIGHT - 30 + PLAYER_HEIGHT && b.y + b.height > CANVAS_HEIGHT - 30) {
          state.alienBullets.splice(bi, 1);
          setLives(l => {
            if (l <= 1) {
              setGameState('GAME_OVER');
              return 0;
            }
            return l - 1;
          });
        }
      });

      // Collision detection: Alien -> Player
      state.aliens.forEach(a => {
        if (a.alive && a.y + a.height >= CANVAS_HEIGHT - 30) {
          setGameState('GAME_OVER');
        }
      });

      // Victory check
      if (state.aliens.every(a => !a.alive)) {
        setLevel(l => l + 1);
        resetGame(level + 1); // Advance level
        // To make a simple "Victory" state for the evidence, we might just transition to victory if it's a final level or just check.
        // But here let's say reaching level 5 is Victory.
        if (level >= 5) {
          setGameState('VICTORY');
        }
      }

      draw();
    };

    const draw = () => {
      ctx.fillStyle = '#000';
      ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

      // Player
      ctx.fillStyle = '#0f0';
      ctx.fillRect(gameRef.current.playerX, CANVAS_HEIGHT - 30, PLAYER_WIDTH, PLAYER_HEIGHT);

      // Aliens
      gameRef.current.aliens.forEach(a => {
        if (!a.alive) return;
        ctx.fillStyle = a.type === 0 ? '#f00' : a.type === 1 ? '#ff0' : '#0af';
        ctx.fillRect(a.x, a.y, a.width, a.height);
      });

      // Bullets
      ctx.fillStyle = '#fff';
      gameRef.current.bullets.forEach(b => ctx.fillRect(b.x, b.y, b.width, b.height));
      ctx.fillStyle = '#f0f';
      gameRef.current.alienBullets.forEach(b => ctx.fillRect(b.x, b.y, b.width, b.height));
    };

    const loop = () => {
      update();
      animationFrameId = requestAnimationFrame(loop);
    };

    animationFrameId = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(animationFrameId);
  }, [gameState, level]);

  // Snapshot for observability
  const getGameStateSnapshot = () => {
    return JSON.stringify({
      playerX: gameRef.current.playerX,
      score,
      lives,
      level,
      gameState,
      alienPositions: gameRef.current.aliens.filter(a => a.alive).map(a => ({ x: a.x, y: a.y })),
    });
  };

  return (
    <div className="min-h-screen bg-slate-900 text-white flex flex-col items-center justify-center p-4 font-mono">
      <h1 className="text-4xl font-bold mb-4 text-green-500 tracking-widest uppercase">Space Invaders</h1>
      
      <div className="relative border-4 border-slate-700 rounded-lg overflow-hidden shadow-2xl" 
           data-anvil-state={getGameStateSnapshot()}>
        <canvas 
          ref={canvasRef} 
          width={CANVAS_WIDTH} 
          height={CANVAS_HEIGHT} 
          className="bg-black block"
        />

        {gameState === 'START' && (
          <div className="absolute inset-0 bg-black/80 flex flex-col items-center justify-center text-center p-6">
            <h2 className="text-5xl font-bold mb-6 animate-pulse">READY?</h2>
            <p className="mb-8 text-slate-400">Use Left/Right Arrows to move, Space to shoot.</p>
            <button 
              onClick={startGame}
              data-anvil-action="primary"
              className="px-8 py-3 bg-green-600 hover:bg-green-500 transition-colors text-2xl font-bold rounded-full shadow-lg uppercase tracking-widest"
            >
              Start Mission
            </button>
          </div>
        )}

        {gameState === 'GAME_OVER' && (
          <div className="absolute inset-0 bg-red-900/80 flex flex-col items-center justify-center text-center p-6">
            <h2 className="text-6xl font-bold mb-4 text-white drop-shadow-md">MISSION FAILED</h2>
            <p className="text-2xl mb-8">Final Score: {score}</p>
            <button 
              onClick={() => resetGame(1)}
              data-anvil-action="restart"
              className="px-8 py-3 bg-white text-red-900 hover:bg-slate-200 transition-colors text-2xl font-bold rounded-full shadow-lg uppercase tracking-widest"
            >
              Try Again
            </button>
          </div>
        )}

        {gameState === 'VICTORY' && (
          <div className="absolute inset-0 bg-green-900/80 flex flex-col items-center justify-center text-center p-6">
            <h2 className="text-6xl font-bold mb-4 text-white drop-shadow-md">GALAXY SAVED!</h2>
            <p className="text-2xl mb-8">Final Score: {score}</p>
            <button 
              onClick={() => resetGame(1)}
              data-anvil-action="restart"
              className="px-8 py-3 bg-white text-green-900 hover:bg-slate-200 transition-colors text-2xl font-bold rounded-full shadow-lg uppercase tracking-widest"
            >
              Play Again
            </button>
          </div>
        )}

        {gameState === 'PLAYING' && (
          <div className="absolute top-0 left-0 right-0 p-4 flex justify-between items-center pointer-events-none">
            <div className="text-xl">Score: <span className="text-green-400">{score}</span></div>
            <div className="text-xl text-red-400">Lives: {lives}</div>
            <div className="text-xl">Level: <span className="text-blue-400">{level}</span></div>
          </div>
        )}
      </div>

      <div className="mt-6 text-slate-500 text-sm uppercase tracking-tighter flex gap-4">
         <span>Arrows to Move</span>
         <span>Space to Shoot</span>
         <span>R to Restart</span>
      </div>
    </div>
  );
}
