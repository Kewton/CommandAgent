"use client";

import React, { useEffect, useRef, useState } from 'react';
import { updateGame, initGame, GameState, Entity } from '@/lib/game-logic';
import { GameConfig } from '@/lib/types';

const CONFIG: GameConfig = {
  canvasWidth: 800,
  canvasHeight: 600,
  playerSpeed: 5,
  projectileSpeed: 7,
  alienSpeed: 1,
  alienDropDistance: 20,
  alienRows: 5,
  alienCols: 11,
};

export default function SpaceInvaders() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [gameState, setGameState] = useState<'idle' | 'playing' | 'game-over'>('idle');
  const [score, setScore] = useState(0);
  const [lives, setLives] = useState(3);
  
  const gameRef = useRef<GameState | null>(null);
  const requestRef = useRef<number>();

  const startGame = () => {
    gameRef.current = initGame(CONFIG);
    setScore(0);
    setLives(3);
    setGameState('playing');
  };

  const restartGame = () => {
    startGame();
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!gameRef.current) return;
      
      if (e.code === 'ArrowLeft') gameRef.current.player.movingLeft = true;
      if (e.code === 'ArrowRight') gameRef.current.player.movingRight = true;
      if (e.code === 'Space') gameRef.current.canShoot = true;
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (!gameRef.current) return;
      
      if (e.code === 'ArrowLeft') gameRef.current.player.movingLeft = false;
      if (e.code === 'ArrowRight') gameRef.current.player.movingRight = false;
      if (e.code === 'Space') gameRef.current.canShoot = false;
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, []);

  useEffect(() => {
    const loop = () => {
      if (gameState === 'playing' && gameRef.current && canvasRef.current) {
        const ctx = canvasRef.current.getContext('2d');
        if (ctx) {
          // Update state
          const newState = updateGame(gameRef.current, CONFIG);
          gameRef.current = newState;

          // Sync Score/Lives (simplified)
          setScore(newState.score);
          
          // Collision with player or bottom boundary
          if (newState.gameOver) {
            setGameState('game-over');
          }

          // Render
          ctx.fillStyle = '#000';
          ctx.fillRect(0, 0, CONFIG.canvasWidth, CONFIG.canvasHeight);

          // Player
          ctx.fillStyle = '#00FF00';
          ctx.fillRect(
            newState.player.x, 
            newState.player.y, 
            40, 20
          );

          // Projectiles
          ctx.fillStyle = '#FFFF00';
          newState.projectiles.forEach(p => {
            ctx.fillRect(p.x, p.y, 4, 10);
          });

          // Aliens
          newState.aliens.forEach(a => {
            ctx.fillStyle = a.type === 'boss' ? '#FF00FF' : '#FF0000';
            ctx.fillRect(a.x, a.y, 30, 20);
          });
        }
      }
      requestRef.current = requestAnimationFrame(loop);
    };

    requestRef.current = requestAnimationFrame(loop);
    return () => {
      if (requestRef.current) cancelAnimationFrame(requestRef.current);
    };
  }, [gameState]);

  return (
    <div 
      className="min-h-screen bg-slate-900 text-white flex flex-col items-center justify-center p-4 font-mono"
      data-anvil-state={JSON.stringify({ gameState, score })}
    >
      <div className="mb-4 flex justify-between w-full max-w-[800px] text-2xl uppercase tracking-widest">
        <div>Score: {score}</div>
        <div className="text-green-400">Space Invaders</div>
      </div>

      <div className="relative border-4 border-slate-700 shadow-2xl rounded-lg overflow-hidden">
        <canvas 
          ref={canvasRef} 
          width={CONFIG.canvasWidth} 
          height={CONFIG.canvasHeight} 
          className="bg-black"
        />

        {gameState === 'idle' && (
          <div className="absolute inset-0 bg-black/80 flex flex-col items-center justify-center backdrop-blur-sm">
            <h1 className="text-6xl font-bold mb-8 animate-pulse text-green-500">READY?</h1>
            <button 
              onClick={startGame}
              data-anvil-action="primary"
              className="px-8 py-4 bg-green-600 hover:bg-green-500 text-white text-2xl font-bold rounded-full transition-all transform hover:scale-110 active:scale-95 border-b-4 border-green-800"
            >
              START GAME
            </button>
          </div>
        )}

        {gameState === 'game-over' && (
          <div className="absolute inset-0 bg-red-900/80 flex flex-col items-center justify-center backdrop-blur-sm">
            <h1 className="text-6xl font-bold mb-4 text-white">GAME OVER</h1>
            <p className="text-2xl mb-8">Final Score: {score}</p>
            <button 
              onClick={restartGame}
              data-anvil-action="restart"
              className="px-8 py-4 bg-white text-red-900 text-2xl font-bold rounded-full transition-all transform hover:scale-110 active:scale-95 border-b-4 border-slate-400"
            >
              TRY AGAIN
            </button>
          </div>
        )}
      </div>

      <div className="mt-6 text-slate-400 text-sm flex gap-4">
        <span>← Left</span>
        <span>→ Right</span>
        <span>SPACE Shoot</span>
      </div>
    </div>
  );
}
