'use client';

import React, { useEffect, useRef, useState } from 'react';

type GameStatus = 'START' | 'PLAYING' | 'GAMEOVER' | 'VICTORY';

interface Brick {
  x: number;
  y: number;
  status: number; // 1 for active, 0 for broken
  color: string;
}

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  color: string;
}

export default function BlockBreaker() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [status, setStatus] = useState<GameStatus>('START');
  const [score, setScore] = useState(0);
  const [lives, setLives] = useState(3);
  const [paddleX, setPaddleX] = useState(0);

  // Game constants
  const CANVAS_WIDTH = 800;
  const CANVAS_HEIGHT = 600;
  const PADDLE_WIDTH = 100;
  const PADDLE_HEIGHT = 20;
  const BALL_RADIUS = 8;
  const BRICK_ROWS = 5;
  const BRICK_COLS = 8;
  const BRICK_PADDING = 10;
  const BRICK_OFFSET_TOP = 60;
  const BRICK_OFFSET_LEFT = 35;
  const BRICK_WIDTH = (CANVAS_WIDTH - BRICK_OFFSET_LEFT * 2) / BRICK_COLS - BRICK_PADDING;
  const BRICK_HEIGHT = 20;

  // Game state refs to avoid closure issues in the loop
  const gameState = useRef({
    ballX: CANVAS_WIDTH / 2,
    ballY: CANVAS_HEIGHT - 50,
    ballDX: 4,
    ballDY: -4,
    paddleX: (CANVAS_WIDTH - PADDLE_WIDTH) / 2,
    bricks: [] as Brick[],
    particles: [] as Particle[],
    score: 0,
    lives: 3,
  });

  const requestRef = useRef<number>();

  const initBricks = () => {
    const colors = ['#ef4444', '#f97316', '#eab308', '#22c55e', '#3b82f6'];
    const bricks: Brick[] = [];
    for (let r = 0; r < BRICK_ROWS; r++) {
      for (let c = 0; c < BRICK_COLS; c++) {
        bricks.push({
          x: c * (BRICK_WIDTH + BRICK_PADDING) + BRICK_OFFSET_LEFT,
          y: r * (BRICK_HEIGHT + BRICK_PADDING) + BRICK_OFFSET_TOP,
          status: 1,
          color: colors[r],
        });
      }
    }
    gameState.current.bricks = bricks;
  };

  const createParticles = (x: number, y: number, color: string) => {
    for (let i = 0; i < 8; i++) {
      gameState.current.particles.push({
        x,
        y,
        vx: (Math.random() - 0.5) * 4,
        vy: (Math.random() - 0.5) * 4,
        life: 1.0,
        color,
      });
    }
  };

  const resetBall = () => {
    gameState.current.ballX = CANVAS_WIDTH / 2;
    gameState.current.ballY = CANVAS_HEIGHT - 50;
    gameState.current.ballDX = 4 * (Math.random() > 0.5 ? 1 : -1);
    gameState.current.ballDY = -4;
  };

  const startGame = () => {
    gameState.current.score = 0;
    gameState.current.lives = 3;
    setScore(0);
    setLives(3);
    initBricks();
    resetBall();
    setStatus('PLAYING');
  };

  const restartGame = () => {
    startGame();
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'r' || e.key === 'R') {
        restartGame();
      }
    };

    const handleMouseMove = (e: MouseEvent) => {
      if (!canvasRef.current) return;
      const rect = canvasRef.current.getBoundingClientRect();
      const relativeX = e.clientX - rect.left;
      if (relativeX >= 0 && relativeX <= CANVAS_WIDTH) {
        const nextX = Math.max(0, Math.min(CANVAS_WIDTH - PADDLE_WIDTH, relativeX - PADDLE_WIDTH / 2));
        gameState.current.paddleX = nextX;
        setPaddleX(nextX);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('mousemove', handleMouseMove);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('mousemove', handleMouseMove);
      if (requestRef.current) cancelAnimationFrame(requestRef.current);
    };
  }, []);

  const update = () => {
    if (status !== 'PLAYING') return;

    const state = gameState.current;

    // Move ball
    state.ballX += state.ballDX;
    state.ballY += state.ballDY;

    // Wall collisions
    if (state.ballX + BALL_RADIUS > CANVAS_WIDTH || state.ballX - BALL_RADIUS < 0) {
      state.ballDX *= -1;
    }
    if (state.ballY - BALL_RADIUS < 0) {
      state.ballDY *= -1;
    }

    // Paddle collision
    if (
      state.ballY + BALL_RADIUS > CANVAS_HEIGHT - PADDLE_HEIGHT &&
      state.ballX > state.paddleX &&
      state.ballX < state.paddleX + PADDLE_WIDTH
    ) {
      // Adjust bounce angle based on where it hits the paddle
      const hitPoint = (state.ballX - (state.paddleX + PADDLE_WIDTH / 2)) / (PADDLE_WIDTH / 2);
      state.ballDX = hitPoint * 5;
      state.ballDY *= -1;
    }

    // Bottom collision
    if (state.ballY + BALL_RADIUS > CANVAS_HEIGHT) {
      state.lives--;
      setLives(state.lives);
      if (state.lives <= 0) {
        setStatus('GAMEOVER');
      } else {
        resetBall();
      }
    }

    // Brick collisions
    state.bricks.forEach((brick) => {
      if (brick.status === 1) {
        if (
          state.ballX > brick.x &&
          state.ballX < brick.x + BRICK_WIDTH &&
          state.ballY > brick.y &&
          state.ballY < brick.y + BRICK_HEIGHT
        ) {
          brick.status = 0;
          state.ballDY *= -1;
          state.score += 10;
          setScore(state.score);
          createParticles(brick.x + BRICK_WIDTH / 2, brick.y + BRICK_HEIGHT / 2, brick.color);

          // Check for victory
          if (state.bricks.every((b) => b.status === 0)) {
            setStatus('VICTORY');
          }
        }
      }
    });

    // Update particles
    state.particles = state.particles.filter((p) => p.life > 0);
    state.particles.forEach((p) => {
      p.x += p.vx;
      p.y += p.vy;
      p.life -= 0.02;
    });
  };

  const draw = (ctx: CanvasRenderingContext2D) => {
    const state = gameState.current;

    // Clear canvas
    ctx.fillStyle = '#0f172a';
    ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

    // Draw bricks
    state.bricks.forEach((brick) => {
      if (brick.status === 1) {
        ctx.fillStyle = brick.color;
        ctx.shadowBlur = 10;
        ctx.shadowColor = brick.color;
        ctx.fillRect(brick.x, brick.y, BRICK_WIDTH, BRICK_HEIGHT);
        ctx.shadowBlur = 0;
      }
    });

    // Draw paddle
    ctx.fillStyle = '#f8fafc';
    ctx.shadowBlur = 15;
    ctx.shadowColor = '#fff';
    ctx.fillRect(state.paddleX, CANVAS_HEIGHT - PADDLE_HEIGHT, PADDLE_WIDTH, PADDLE_HEIGHT);
    ctx.shadowBlur = 0;

    // Draw ball
    ctx.beginPath();
    ctx.arc(state.ballX, state.ballY, BALL_RADIUS, 0, Math.PI * 2);
    ctx.fillStyle = '#fff';
    ctx.shadowBlur = 10;
    ctx.shadowColor = '#fff';
    ctx.fill();
    ctx.closePath();
    ctx.shadowBlur = 0;

    // Draw particles
    state.particles.forEach((p) => {
      ctx.globalAlpha = p.life;
      ctx.fillStyle = p.color;
      ctx.fillRect(p.x, p.y, 3, 3);
    });
    ctx.globalAlpha = 1.0;
  };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const loop = () => {
      update();
      draw(ctx);
      requestRef.current = requestAnimationFrame(loop);
    };

    requestRef.current = requestAnimationFrame(loop);

    return () => {
      if (requestRef.current) cancelAnimationFrame(requestRef.current);
    };
  }, [status]);

  return (
    <div 
      className="flex flex-col items-center justify-center min-h-screen bg-slate-950 text-white font-sans overflow-hidden"
      data-anvil-state={JSON.stringify({ paddleX, score, lives, status })}
    >
      <div className="mb-4 text-center">
        <h1 className="text-5xl font-black tracking-tighter italic text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-purple-600 uppercase mb-2">
          Neon Breakout
        </h1>
        <div className="flex gap-8 text-xl font-mono">
          <span>SCORE: {score}</span>
          <span>LIVES: {lives}</span>
        </div>
      </div>

      <div className="relative group">
        <canvas
          ref={canvasRef}
          width={CANVAS_WIDTH}
          height={CANVAS_HEIGHT}
          className="border-4 border-slate-800 rounded-lg shadow-2xl cursor-none"
        />

        {status === 'START' && (
          <div className="absolute inset-0 flex items-center justify-center bg-slate-900/80 backdrop-blur-sm rounded-lg">
            <button
              onClick={startGame}
              data-anvil-action="primary"
              className="px-8 py-4 bg-blue-600 hover:bg-blue-500 text-white text-2xl font-bold rounded-full transition-all transform hover:scale-110 shadow-[0_0_20px_rgba(37,99,235,0.5)]"
            >
              START GAME
            </button>
          </div>
        )}

        {status === 'GAMEOVER' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-red-900/80 backdrop-blur-sm rounded-lg">
            <h2 className="text-6xl font-black mb-6 text-white drop-shadow-lg uppercase italic">Game Over</h2>
            <button
              onClick={restartGame}
              data-anvil-action="restart"
              className="px-8 py-4 bg-white text-red-600 text-2xl font-bold rounded-full transition-all transform hover:scale-110 shadow-lg"
            >
              TRY AGAIN
            </button>
          </div>
        )}

        {status === 'VICTORY' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-green-900/80 backdrop-blur-sm rounded-lg">
            <h2 className="text-6xl font-black mb-6 text-white drop-shadow-lg uppercase italic">Victory!</h2>
            <button
              onClick={restartGame}
              data-anvil-action="restart"
              className="px-8 py-4 bg-white text-green-600 text-2xl font-bold rounded-full transition-all transform hover:scale-110 shadow-lg"
            >
              PLAY AGAIN
            </button>
          </div>
        )}
      </div>

      <div className="mt-6 text-slate-400 font-mono text-sm">
        MOVE MOUSE TO CONTROL PADDLE | PRESS 'R' TO RESTART
      </div>
    </div>
  );
}
