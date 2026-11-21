"use client";

import React, { useEffect, useRef, useState } from 'react';
import { initGame, updateGame } from '@/lib/game-logic';
import { GameState } from '@/lib/types';

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  color: string;
}

export default function GamePage() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [gameState, setGameState] = useState<GameState>(initGame());
  const [keys, setKeys] = useState({ left: false, right: false, shoot: false });
  const [particles, setParticles] = useState<Particle[]>([]);
  const [shake, setShake] = useState(false);
  const [stars, setStars] = useState<{x: number, y: number, size: number, speed: number}[]>([]);
  
  const requestRef = useRef<number>();
  const shakeTimerRef = useRef<number>(0);

  // Initialize stars for background
  useEffect(() => {
    const newStars = Array.from({ length: 100 }, () => ({
      x: Math.random() * 800,
      y: Math.random() * 600,
      size: Math.random() * 2,
      speed: Math.random() * 0.5 + 0.1
    }));
    setStars(newStars);
  }, []);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'ArrowLeft') setKeys(prev => ({ ...prev, left: true }));
      if (e.key === 'ArrowRight') setKeys(prev => ({ ...prev, right: true }));
      if (e.key === ' ' || e.key === 'Control') setKeys(prev => ({ ...prev, shoot: true }));
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.key === 'ArrowLeft') setKeys(prev => ({ ...prev, left: false }));
      if (e.key === 'ArrowRight') setKeys(prev => ({ ...prev, right: false }));
      if (e.key === ' ' || e.key === 'Control') setKeys(prev => ({ ...prev, shoot: false }));
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, []);

  const spawnParticles = (x: number, y: number, color: string) => {
    const newParticles: Particle[] = Array.from({ length: 10 }, () => ({
      x, y,
      vx: (Math.random() - 0.5) * 4,
      vy: (Math.random() - 0.5) * 4,
      life: 1.0,
      color
    }));
    setParticles(prev => [...prev, ...newParticles]);
    setShake(true);
    shakeTimerRef.current = 12; // frames
  };

  const gameLoop = () => {
    setGameState(prev => {
      const next = updateGame(prev, keys);
      
      // Detection for alien death to trigger particles
      if (prev.status === 'PLAYING' && next.aliens.length < prev.aliens.length) {
        // Simplification: spawn at center of the void for this loop update
        // In a more complex engine, updateGame would return event logs
        spawnParticles(400, 200, '#ff00ff');
      }
      
      return next;
    });

    setParticles(prev => 
      prev.map(p => ({ ...p, x: p.x + p.vx, y: p.y + p.vy, life: p.life - 0.02 }))
          .filter(p => p.life > 0)
    );

    if (shakeTimerRef.current > 0) {
      shakeTimerRef.current--;
      if (shakeTimerRef.current === 0) setShake(false);
    }

    requestRef.current = requestAnimationFrame(gameLoop);
  };

  useEffect(() => {
    requestRef.current = requestAnimationFrame(gameLoop);
    return () => {
      if (requestRef.current) cancelAnimationFrame(requestRef.current);
    };
  }, [keys]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.fillStyle = 'black';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Draw Dynamic Stars
    ctx.fillStyle = 'white';
    stars.forEach(s => {
      ctx.beginPath();
      ctx.arc(s.x, s.y, s.size, 0, Math.PI * 2);
      ctx.fill();
    });

    // Draw Player
    ctx.fillStyle = '#00ffcc';
    ctx.shadowBlur = 15;
    ctx.shadowColor = '#00ffcc';
    ctx.fillRect(gameState.player.position.x, gameState.player.position.y, gameState.player.size.x, gameState.player.size.y);

    // Draw Aliens
    ctx.fillStyle = '#ff00ff';
    ctx.shadowColor = '#ff00ff';
    gameState.aliens.forEach(a => {
      ctx.fillRect(a.position.x, a.position.y, a.size.x, a.size.y);
    });

    // Draw Bullets
    ctx.fillStyle = '#ffff00';
    ctx.shadowColor = '#ffff00';
    gameState.bullets.forEach(b => {
      ctx.fillRect(b.position.x, b.position.y, b.size.x, b.size.y);
    });

    // Draw Particles
    particles.forEach(p => {
      ctx.globalAlpha = p.life;
      ctx.fillStyle = p.color;
      ctx.shadowBlur = 5;
      ctx.shadowColor = p.color;
      ctx.fillRect(p.x, p.y, 2, 2);
    });
    ctx.globalAlpha = 1.0;
    ctx.shadowBlur = 0;

  }, [gameState, particles, stars]);

  const handleStart = () => {
    setGameState(prev => ({ ...prev, status: 'PLAYING' }));
  };

  const handleRestart = () => {
    setGameState(initGame());
    setParticles([]);
    setShake(false);
  };

  return (
    <div className="relative w-full h-screen flex items-center justify-center bg-black overflow-hidden">
      <div 
        className={`relative ${shake ? 'screen-shake' : ''}`}
        data-anvil-state={JSON.stringify({ 
          score: gameState.score, 
          level: gameState.level, 
          status: gameState.status, 
          shake: shake 
        })}
      >
        <canvas 
          ref={canvasRef} 
          width={800} 
          height={600} 
          className="border-4 border-gray-800 rounded-lg shadow-2xl bg-black"
        />

        {gameState.status === 'IDLE' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/70 text-white backdrop-blur-sm">
            <h1 className="text-6xl font-bold mb-8 arcade-glow text-cyan-400">SPACE INVADERS</h1>
            <button 
              onClick={handleStart}
              data-anvil-action="primary"
              className="px-8 py-4 bg-cyan-500 hover:bg-cyan-400 text-black font-bold text-2xl rounded-full transition-all transform hover:scale-110 active:scale-95 shadow-[0_0_20px_rgba(0,255,204,0.6)]"
            >
              START MISSION
            </button>
            <p className="mt-6 text-gray-400 uppercase tracking-widest text-sm">Use Arrows to Move • Space to Fire</p>
          </div>
        )}

        {(gameState.status === 'WON' || gameState.status === 'GAMEOVER') && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/80 text-white backdrop-blur-md">
            <h2 className={`text-6xl font-bold mb-4 arcade-glow ${gameState.status === 'WON' ? 'text-green-400' : 'text-red-500'}`}>
              {gameState.status === 'WON' ? 'VICTORY' : 'MISSION FAILED'}
            </h2>
            <p className="text-2xl mb-8">Final Score: {gameState.score}</p>
            <button 
              onClick={handleRestart}
              data-anvil-action="restart"
              className="px-8 py-4 bg-white text-black font-bold text-2xl rounded-full transition-all transform hover:scale-110 active:scale-95 shadow-[0_0_20px_rgba(255,255,255,0.4)]"
            >
              TRY AGAIN
            </button>
          </div>
        )}

        <div className="absolute top-4 left-4 text-2xl font-bold text-cyan-400 arcade-glow">
          SCORE: {gameState.score}
        </div>
        <div className="absolute top-4 right-4 text-2xl font-bold text-cyan-400 arcade-glow">
          LEVEL: {gameState.level}
        </div>
      </div>
    </div>
  );
}
