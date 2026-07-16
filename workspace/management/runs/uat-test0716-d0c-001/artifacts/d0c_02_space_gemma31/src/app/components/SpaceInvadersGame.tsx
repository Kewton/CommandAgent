"use client";
import React, { useEffect, useRef, useState } from 'react';

type GameStatus = 'MENU' | 'PLAYING' | 'GAMEOVER' | 'VICTORY';

interface Enemy {
  x: number;
  y: number;
  alive: boolean;
}

interface Projectile {
  x: number;
  y: number;
}

const CANVAS_WIDTH = 800;
const CANVAS_HEIGHT = 600;
const PLAYER_WIDTH = 40;
const PLAYER_HEIGHT = 20;
const ENEMY_ROWS = 5;
const ENEMY_COLS = 11;
const ENEMY_WIDTH = 30;
const ENEMY_HEIGHT = 20;
const ENEMY_PADDING = 15;
const PROJECTILE_SPEED = 7;
const PLAYER_SPEED = 5;
const ENEMY_SPEED_START = 1;

export default function SpaceInvadersGame() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [gameStatus, setGameStatus] = useState<GameStatus>('MENU');
  const [score, setScore] = useState(0);
  const [playerX, setPlayerX] = useState(CANVAS_WIDTH / 2 - PLAYER_WIDTH / 2);
  const [enemyCount, setEnemyCount] = useState(0);

  // Game state refs for high-performance loop
  const gameStateRef = useRef({
    playerX: CANVAS_WIDTH / 2 - PLAYER_WIDTH / 2,
    projectiles: [] as Projectile[],
    enemies: [] as Enemy[],
    enemyDirection: 1,
    enemyStepDown: false,
    lastShotTime: 0,
    score: 0,
    status: 'MENU' as GameStatus,
  });

  const keysPressed = useRef<Set<string>>(new Set());

  const initGame = () => {
    const enemies: Enemy[] = [];
    for (let r = 0; r < ENEMY_ROWS; r++) {
      for (let c = 0; c < ENEMY_COLS; c++) {
        enemies.push({
          x: c * (ENEMY_WIDTH + ENEMY_PADDING) + 50,
          y: r * (ENEMY_HEIGHT + ENEMY_PADDING) + 50,
          alive: true,
        });
      }
    }

    gameStateRef.current = {
      playerX: CANVAS_WIDTH / 2 - PLAYER_WIDTH / 2,
      projectiles: [],
      enemies,
      enemyDirection: 1,
      enemyStepDown: false,
      lastShotTime: 0,
      score: 0,
      status: 'PLAYING',
    };

    setGameStatus('PLAYING');
    setScore(0);
    setPlayerX(CANVAS_WIDTH / 2 - PLAYER_WIDTH / 2);
    setEnemyCount(enemies.length);
  };

  const restartGame = () => {
    initGame();
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      keysPressed.current.add(e.code);
      if (e.code === 'KeyR') {
        restartGame();
      }
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      keysPressed.current.delete(e.code);
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let animationFrameId: number;

    const update = () => {
      const state = gameStateRef.current;
      if (state.status !== 'PLAYING') return;

      // Player Movement
      if (keysPressed.current.has('ArrowLeft')) {
        state.playerX = Math.max(0, state.playerX - PLAYER_SPEED);
      }
      if (keysPressed.current.has('ArrowRight')) {
        state.playerX = Math.min(CANVAS_WIDTH - PLAYER_WIDTH, state.playerX + PLAYER_SPEED);
      }

      // Shooting
      if (keysPressed.current.has('Space')) {
        const now = Date.now();
        if (now - state.lastShotTime > 400) {
          state.projectiles.push({ x: state.playerX + PLAYER_WIDTH / 2, y: CANVAS_HEIGHT - 30 });
          state.lastShotTime = now;
        }
      }

      // Projectiles movement & collision
      state.projectiles = state.projectiles.filter(p => p.y > 0);
      state.projectiles.forEach(p => {
        p.y -= PROJECTILE_SPEED;
        state.enemies.forEach(e => {
          if (e.alive && p.x > e.x && p.x < e.x + ENEMY_WIDTH && p.y > e.y && p.y < e.y + ENEMY_HEIGHT) {
            e.alive = false;
            p.y = -10; // mark for removal
            state.score += 10;
          }
        });
      });

      // Enemies movement
      let hitWall = false;
      const aliveEnemies = state.enemies.filter(e => e.alive);
      if (aliveEnemies.length === 0) {
        state.status = 'VICTORY';
        setGameStatus('VICTORY');
        return;
      }

      aliveEnemies.forEach(e => {
        e.x += ENEMY_SPEED_START * state.enemyDirection;
        if (e.x <= 0 || e.x >= CANVAS_WIDTH - ENEMY_WIDTH) {
          hitWall = true;
        }
        if (e.y + ENEMY_HEIGHT >= CANVAS_HEIGHT - 40) {
          state.status = 'GAMEOVER';
          setGameStatus('GAMEOVER');
        }
      });

      if (hitWall) {
        state.enemyDirection *= -1;
        state.enemies.forEach(e => { e.y += 20; });
      }

      // Sync observability state to React state to trigger re-renders for the probe
      setPlayerX(state.playerX);
      setScore(state.score);
      setEnemyCount(aliveEnemies.length);
    };

    const draw = () => {
      const state = gameStateRef.current;
      ctx.fillStyle = '#020617';
      ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

      // Draw Player
      ctx.fillStyle = '#4ade80';
      ctx.fillRect(state.playerX, CANVAS_HEIGHT - 30, PLAYER_WIDTH, PLAYER_HEIGHT);

      // Draw Enemies
      ctx.fillStyle = '#f87171';
      state.enemies.forEach(e => {
        if (e.alive) ctx.fillRect(e.x, e.y, ENEMY_WIDTH, ENEMY_HEIGHT);
      });

      // Draw Projectiles
      ctx.fillStyle = '#fbbf24';
      state.projectiles.forEach(p => {
        ctx.fillRect(p.x - 2, p.y, 4, 10);
      });

      // UI Overlay within Canvas (Optional, but we use HTML)
    };

    const loop = () => {
      update();
      draw();
      animationFrameId = requestAnimationFrame(loop);
    };

    loop();
    return () => cancelAnimationFrame(animationFrameId);
  }, []);

  return (
    <div 
      className="flex flex-col items-center justify-center gap-4 text-white font-mono"
      data-anvil-state={JSON.stringify({ playerX, score, gameStatus, enemyCount })}
    >
      <div className="text-2xl mb-2">SCORE: {score}</div>
      
      <div className="relative border-4 border-slate-700 rounded-lg overflow-hidden shadow-2xl bg-slate-950">
        <canvas 
          ref={canvasRef} 
          width={CANVAS_WIDTH} 
          height={CANVAS_HEIGHT} 
          className="block"
        />

        {gameStatus === 'MENU' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/70 backdrop-blur-sm">
            <h1 className="text-5xl font-bold mb-8 text-green-400 tracking-widest uppercase">Space Invaders</h1>
            <button 
              onClick={initGame} 
              data-anvil-action="primary"
              className="px-8 py-3 bg-green-600 hover:bg-green-500 text-white rounded-full text-xl transition-all transform hover:scale-105 font-bold shadow-[0_0_15px_rgba(74,222,128,0.5)]"
            >
              START MISSION
            </button>
            <p className="mt-6 text-slate-400">Use Arrow Keys to Move & Space to Shoot</p>
          </div>
        )}

        {gameStatus === 'GAMEOVER' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-red-900/70 backdrop-blur-sm">
            <h1 className="text-6xl font-bold mb-4 text-white uppercase italic">Mission Failed</h1>
            <p className="text-2xl mb-8">The aliens have taken over!</p>
            <button 
              onClick={restartGame} 
              data-anvil-action="restart"
              className="px-8 py-3 bg-white text-red-900 rounded-full text-xl transition-all transform hover:scale-105 font-bold shadow-lg"
            >
              TRY AGAIN
            </button>
          </div>
        )}

        {gameStatus === 'VICTORY' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-green-900/70 backdrop-blur-sm">
            <h1 className="text-6xl font-bold mb-4 text-white uppercase italic">Victory!</h1>
            <p className="text-2xl mb-8">Earth is safe once again!</p>
            <button 
              onClick={restartGame} 
              data-anvil-action="restart"
              className="px-8 py-3 bg-white text-green-900 rounded-full text-xl transition-all transform hover:scale-105 font-bold shadow-lg"
            >
              PLAY AGAIN
            </button>
          </div>
        )}
      </div>
      <div className="text-slate-500 text-sm">Press 'R' at any time to restart the mission</div>
    </div>
  );
}
