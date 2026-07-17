"use client";

import React, { useEffect, useRef, useState } from 'react';

interface Brick {
  x: number;
  y: number;
  status: number;
  color: string;
}

interface GameStateSnapshot {
  paddleX: number;
  score: number;
  lives: number;
  status: 'START' | 'PLAYING' | 'GAMEOVER' | 'VICTORY';
  ballX: number;
  ballY: number;
}

export default function BreakoutGame() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [gameState, setGameState] = useState<'START' | 'PLAYING' | 'GAMEOVER' | 'VICTORY'>('START');
  const [score, setScore] = useState(0);
  const [lives, setLives] = useState(3);
  const [snapshot, setSnapshot] = useState<GameStateSnapshot>({
    paddleX: 0,
    score: 0,
    lives: 3,
    status: 'START',
    ballX: 0,
    ballY: 0,
  });

  const gameVars = useRef({
    paddleX: 0,
    ballX: 0,
    ballY: 0,
    ballDX: 0,
    ballDY: 0,
    bricks: [] as Brick[],
    rightKeyDown: false,
    leftKeyDown: false,
  });

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const PADDLE_WIDTH = 100;
    const PADDLE_HEIGHT = 15;
    const BALL_RADIUS = 8;
    const BRICK_ROWS = 5;
    const BRICK_COLS = 8;
    const BRICK_WIDTH = 70;
    const BRICK_HEIGHT = 20;
    const BRICK_PADDING = 10;
    const BRICK_OFFSET_TOP = 50;
    const BRICK_OFFSET_LEFT = 30;

    const COLORS = ['#ef4444', '#f97316', '#f59e0b', '#10b981', '#3b82f6'];

    const initGame = () => {
      gameVars.current.paddleX = (canvas.width - PADDLE_WIDTH) / 2;
      gameVars.current.ballX = canvas.width / 2;
      gameVars.current.ballY = canvas.height - 30;
      gameVars.current.ballDX = 4;
      gameVars.current.ballDY = -4;
      
      const bricks: Brick[] = [];
      for (let c = 0; c < BRICK_COLS; c++) {
        for (let r = 0; r < BRICK_ROWS; r++) {
          bricks.push({
            x: c * (BRICK_WIDTH + BRICK_PADDING) + BRICK_OFFSET_LEFT,
            y: r * (BRICK_HEIGHT + BRICK_PADDING) + BRICK_OFFSET_TOP,
            status: 1,
            color: COLORS[r] || '#888',
          });
        }
      }
      gameVars.current.bricks = bricks;
    };

    const resetBall = () => {
      gameVars.current.ballX = canvas.width / 2;
      gameVars.current.ballY = canvas.height - 30;
      gameVars.current.ballDX = 4 * (Math.random() > 0.5 ? 1 : -1);
      gameVars.current.ballDY = -4;
      gameVars.current.paddleX = (canvas.width - PADDLE_WIDTH) / 2;
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'ArrowRight') gameVars.current.rightKeyDown = true;
      if (e.key === 'ArrowLeft') gameVars.current.leftKeyDown = true;
      if (e.key.toLowerCase() === 'r') {
        restartGame();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.key === 'ArrowRight') gameVars.current.rightKeyDown = false;
      if (e.key === 'ArrowLeft') gameVars.current.leftKeyDown = false;
    };

    const handleMouseMove = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      const root = document.documentElement;
      const mouseX = e.clientX - rect.left - root.scrollLeft;
      gameVars.current.paddleX = Math.max(0, Math.min(canvas.width - PADDLE_WIDTH, mouseX - PADDLE_WIDTH / 2));
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    window.addEventListener('mousemove', handleMouseMove);

    let animationFrameId: number;

    const update = () => {
      if (gameState !== 'PLAYING') return;

      const { paddleX, ballX, ballY, ballDX, ballDY, bricks } = gameVars.current;
      let newBallX = ballX + gameVars.current.ballDX;
      let newBallY = ballY + gameVars.current.ballDY;
      let newBallDX = gameVars.current.ballDX;
      let newBallDY = gameVars.current.ballDY;
      let newPaddleX = paddleX;

      if (gameVars.current.rightKeyDown) newPaddleX += 7;
      if (gameVars.current.leftKeyDown) newPaddleX -= 7;
      newPaddleX = Math.max(0, Math.min(canvas.width - PADDLE_WIDTH, newPaddleX));

      // Wall Collisions
      if (newBallX + BALL_RADIUS > canvas.width || newBallX - BALL_RADIUS < 0) {
        newBallDX = -newBallDX;
      }
      if (newBallY - BALL_RADIUS < 0) {
        newBallDY = -newBallDY;
      }

      // Paddle Collision
      if (
        newBallY + BALL_RADIUS > canvas.height - PADDLE_HEIGHT &&
        newBallX > newPaddleX &&
        newBallX < newPaddleX + PADDLE_WIDTH
      ) {
        newBallDY = -Math.abs(newBallDY);
        // Add some English based on where it hits the paddle
        const hitPos = (newBallX - newPaddleX) / PADDLE_WIDTH;
        newBallDX = 10 * (hitPos - 0.5);
      }

      // Bottom Collision
      if (newBallY + BALL_RADIUS > canvas.height) {
        setLives(prev => {
          if (prev <= 1) {
            setGameState('GAMEOVER');
            return 0;
          }
          resetBall();
          return prev - 1;
        });
      }

      // Brick Collision
      let collided = false;
      bricks.forEach(brick => {
        if (brick.status === 1) {
          if (
            newBallX > brick.x &&
            newBallX < brick.x + BRICK_WIDTH &&
            newBallY > brick.y &&
            newBallY < brick.y + BRICK_HEIGHT
          ) {
            brick.status = 0;
            newBallDY = -newBallDY;
            collided = true;
            setScore(s => s + 10);
          }
        }
      });

      if (collided) {
        // Check for victory
        if (bricks.every(b => b.status === 0)) {
          setGameState('VICTORY');
        }
      }

      gameVars.current.paddleX = newPaddleX;
      gameVars.current.ballX = newBallX;
      gameVars.current.ballY = newBallY;
      gameVars.current.ballDX = newBallDX;
      gameVars.current.ballDY = newBallDY;

      // Update snapshot for anvil observability
      setSnapshot({
        paddleX: newPaddleX,
        score: score,
        lives: lives,
        status: gameState,
        ballX: newBallX,
        ballY: newBallY,
      });
    };

    const draw = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      // Paddle
      ctx.fillStyle = '#3b82f6';
      ctx.fillRect(gameVars.current.paddleX, canvas.height - PADDLE_HEIGHT, PADDLE_WIDTH, PADDLE_HEIGHT);

      // Ball
      ctx.beginPath();
      ctx.arc(gameVars.current.ballX, gameVars.current.ballY, BALL_RADIUS, 0, Math.PI * 2);
      ctx.fillStyle = '#fff';
      ctx.fill();
      ctx.closePath();

      // Bricks
      gameVars.current.bricks.forEach(brick => {
        if (brick.status === 1) {
          ctx.fillStyle = brick.color;
          ctx.fillRect(brick.x, brick.y, BRICK_WIDTH, BRICK_HEIGHT);
          ctx.strokeStyle = 'rgba(0,0,0,0.2)';
          ctx.strokeRect(brick.x, brick.y, BRICK_WIDTH, BRICK_HEIGHT);
        }
      });
    };

    const loop = () => {
      update();
      draw();
      animationFrameId = requestAnimationFrame(loop);
    };

    loop();

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
      window.removeEventListener('mousemove', handleMouseMove);
      cancelAnimationFrame(animationFrameId);
    };
  }, [gameState, score, lives]);

  const startGame = () => {
    initGame();
    setScore(0);
    setLives(3);
    setGameState('PLAYING');
  };

  const restartGame = () => {
    initGame();
    setScore(0);
    setLives(3);
    setGameState('PLAYING');
  };

  return (
    <div className="min-h-screen bg-slate-900 flex flex-col items-center justify-center p-4 font-sans text-white"
         data-anvil-state={JSON.stringify(snapshot)}>
      <div className="mb-4 flex gap-8 text-2xl font-bold uppercase tracking-widest">
        <div>Score: <span className="text-blue-400">{score}</span></div>
        <div>Lives: <span className="text-red-400">{lives}</span></div>
      </div>

      <div className="relative group">
        <canvas
          ref={canvasRef}
          width={640}
          height={480}
          className="bg-slate-800 rounded-xl shadow-2xl border-4 border-slate-700 cursor-none"
        />

        {gameState === 'START' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-slate-900/80 rounded-xl backdrop-blur-sm">
            <h1 className="text-6xl font-black mb-8 italic text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-purple-500">
              NEON BREAKOUT
            </h1>
            <button
              onClick={startGame}
              data-anvil-action="primary"
              className="px-8 py-4 bg-blue-600 hover:bg-blue-500 text-white rounded-full text-2xl font-bold transition-all transform hover:scale-110 active:scale-95 shadow-lg shadow-blue-500/50"
            >
              START GAME
            </button>
            <p className="mt-6 text-slate-400">Use Mouse or Arrow Keys to move</p>
          </div>
        )}

        {gameState === 'GAMEOVER' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-red-900/60 rounded-xl backdrop-blur-sm">
            <h2 className="text-7xl font-black mb-4 text-white drop-shadow-lg">GAME OVER</h2>
            <p className="text-2xl mb-8 text-slate-200">Final Score: {score}</p>
            <button
              onClick={restartGame}
              data-anvil-action="restart"
              className="px-8 py-4 bg-white text-red-600 hover:bg-slate-100 rounded-full text-2xl font-bold transition-all transform hover:scale-110 active:scale-95"
            >
              TRY AGAIN
            </button>
          </div>
        )}

        {gameState === 'VICTORY' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-green-900/60 rounded-xl backdrop-blur-sm">
            <h2 className="text-7xl font-black mb-4 text-white drop-shadow-lg">VICTORY!</h2>
            <p className="text-2xl mb-8 text-slate-200">You cleared the board!</p>
            <button
              onClick={restartGame}
              data-anvil-action="restart"
              className="px-8 py-4 bg-white text-green-600 hover:bg-slate-100 rounded-full text-2xl font-bold transition-all transform hover:scale-110 active:scale-95"
            >
              PLAY AGAIN
            </button>
          </div>
        )}
      </div>

      {gameState === 'PLAYING' && (
        <div className="mt-6">
          <button
            onClick={restartGame}
            data-anvil-action="restart"
            className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-slate-300 rounded-md text-sm transition-colors"
          >
            Restart Game (R)
          </button>
        </div>
      )}
    </div>
  );
}
