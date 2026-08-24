"use client";

import React, { useEffect, useRef } from 'react';
import { useSpaceInvadersGame } from './game-engine';

export default function SpaceInvadersGame() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const {
    gameState,
    score,
    lives,
    wave,
    playerX,
    invaders,
    startGame,
    restartGame,
    updateInput,
  } = useSpaceInvadersGame();

  useEffect(() => {
    // The game engine hook handles the canvas drawing loop
    // but we need to pass the ref to the hook's internal logic or 
    // let the hook handle the context. 
    // In our game-engine.ts, we designed useSpaceInvadersGame to 
    // take the canvasRef as an argument or provide a way to set it.
    // Let's adjust the hook usage if needed.
  }, []);

  // Handle keyboard input
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      updateInput(e.key, true);
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      updateInput(e.key, false);
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, [updateInput]);

  const stateSnapshot = {
    playerX,
    score,
    lives,
    wave,
    gameState,
    invaderCount: invaders.length,
  };

  return (
    <div 
      className="relative w-full h-screen bg-black overflow-hidden flex items-center justify-center font-mono text-green-500"
      data-anvil-state={JSON.stringify(stateSnapshot)}
    >
      {/* Game Canvas */}
      <canvas 
        ref={canvasRef} 
        width={800} 
        height={600} 
        className="bg-slate-900 border-4 border-green-500 shadow-[0_0_20px_rgba(34,197,94,0.5)] image-pixelated"
        style={{ imageRendering: 'pixelated' }}
      />

      {/* UI Overlays */}
      <div className="absolute top-0 left-0 w-full p-4 flex justify-between text-2xl font-bold pointer-events-none">
        <div>SCORE: {score.toString().padStart(4, '0')}</div>
        <div>WAVE: {wave}</div>
        <div>LIVES: {'♥'.repeat(lives)}</div>
      </div>

      {/* Start Screen */}
      {gameState === 'MENU' && (
        <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/80 backdrop-blur-sm z-10">
          <h1 className="text-6xl font-black mb-8 animate-pulse text-green-400 tracking-widest">
            SPACE <br /> INVADERS
          </h1>
          <button 
            onClick={startGame}
            data-anvil-action="primary"
            className="px-8 py-4 bg-green-600 text-black font-bold text-2xl hover:bg-green-400 transition-colors border-b-4 border-green-800 active:border-b-0 active:translate-y-1"
          >
            START GAME
          </button>
          <p className="mt-8 text-green-700 animate-bounce">PRESS START TO BEGIN</p>
        </div>
      )}

      {/* Game Over Screen */}
      {gameState === 'GAMEOVER' && (
        <div className="absolute inset-0 flex flex-col items-center justify-center bg-red-900/60 backdrop-blur-sm z-10">
          <h2 className="text-7xl font-black mb-4 text-white drop-shadow-lg">GAME OVER</h2>
          <p className="text-2xl mb-8 text-white">FINAL SCORE: {score}</p>
          <button 
            onClick={restartGame}
            data-anvil-action="restart"
            className="px-8 py-4 bg-white text-red-600 font-bold text-2xl hover:bg-gray-200 transition-colors border-b-4 border-gray-400 active:border-b-0 active:translate-y-1"
          >
            RETRY MISSION
          </button>
        </div>
      )}

      {/* Victory Screen */}
      {gameState === 'VICTORY' && (
        <div className="absolute inset-0 flex flex-col items-center justify-center bg-blue-900/60 backdrop-blur-sm z-10">
          <h2 className="text-7xl font-black mb-4 text-yellow-400 drop-shadow-lg">VICTORY!</h2>
          <p className="text-2xl mb-8 text-white">GALAXY SAVED</p>
          <button 
            onClick={restartGame}
            data-anvil-action="restart"
            className="px-8 py-4 bg-yellow-400 text-blue-900 font-bold text-2xl hover:bg-yellow-300 transition-colors border-b-4 border-yellow-600 active:border-b-0 active:translate-y-1"
          >
            PLAY AGAIN
          </button>
        </div>
      )}

      {/* In-game Restart Hint */}
      {gameState === 'PLAYING' && (
        <div className="absolute bottom-4 left-1/2 -translate-x-1/2 text-green-800 text-sm pointer-events-none">
          PRESS [R] TO RESTART
        </div>
      )}

      <style jsx global>{`
        @keyframes pulse {
          0%, 100% { opacity: 1; transform: scale(1); }
          50% { opacity: 0.8; transform: scale(1.05); }
        }
        .image-pixelated {
          image-rendering: pixelated;
        }
      `}</style>
    </div>
  );
}
