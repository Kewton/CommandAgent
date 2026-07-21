"use client";

import React, { useEffect, useRef, useState } from "react";

type GameState = "MENU" | "PLAYING" | "GAME_OVER" | "VICTORY";

interface Brick {
  x: number;
  y: number;
  status: number; // 1 = present, 0 = broken
  color: string;
}

export default function BlockBreaker() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [gameState, setGameState] = useState<GameState>("MENU");
  const [score, setScore] = useState(0);
  const [lives, setLives] = useState(3);
  const [paddleX, setPaddleX] = useState(0);

  // Game Constants
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
  const BRICK_WIDTH = (CANVAS_WIDTH - BRICK_OFFSET_LEFT * 2 - BRICK_PADDING * (BRICK_COLS - 1)) / BRICK_COLS;
  const BRICK_HEIGHT = 20;

  // Game Variables (using refs to avoid React render cycle for the game loop)
  const ballPos = useRef({ x: CANVAS_WIDTH / 2, y: CANVAS_HEIGHT - 30 });
  const ballVel = useRef({ dx: 4, dy: -4 });
  const paddlePos = useRef({ x: (CANVAS_WIDTH - PADDLE_WIDTH) / 2 });
  const bricks = useRef<Brick[]>([]);
  const requestRef = useRef<number>();

  const initBricks = () => {
    const colors = ["#ef4444", "#f97316", "#eab308", "#22c55e", "#3b82f6"];
    const b: Brick[] = [];
    for (let r = 0; r < BRICK_ROWS; r++) {
      for (let c = 0; c < BRICK_COLS; c++) {
        b.push({
          x: c * (BRICK_WIDTH + BRICK_PADDING) + BRICK_OFFSET_LEFT,
          y: r * (BRICK_HEIGHT + BRICK_PADDING) + BRICK_OFFSET_TOP,
          status: 1,
          color: colors[r],
        });
      }
    }
    bricks.current = b;
  };

  const resetBallAndPaddle = () => {
    ballPos.current = { x: CANVAS_WIDTH / 2, y: CANVAS_HEIGHT - 30 };
    ballVel.current = { dx: 4, dy: -4 };
    paddlePos.current = { x: (CANVAS_WIDTH - PADDLE_WIDTH) / 2 };
  };

  const startGame = () => {
    setScore(0);
    setLives(3);
    initBricks();
    resetBallAndPaddle();
    setGameState("PLAYING");
  };

  const restartGame = () => {
    startGame();
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === "r") {
        restartGame();
      }
    };

    const handleMouseMove = (e: MouseEvent) => {
      if (!canvasRef.current) return;
      const rect = canvasRef.current.getBoundingClientRect();
      const root = document.documentElement;
      const mouseX = e.clientX - rect.left - root.scrollLeft;
      paddlePos.current.x = Math.max(0, Math.min(CANVAS_WIDTH - PADDLE_WIDTH, mouseX - PADDLE_WIDTH / 2));
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("mousemove", handleMouseMove);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("mousemove", handleMouseMove);
    };
  }, []);

  // Game Loop
  const update = () => {
    if (gameState !== "PLAYING") return;

    // Move Ball
    ballPos.current.x += ballVel.current.dx;
    ballPos.current.y += ballVel.current.dy;

    // Wall Collisions
    if (ballPos.current.x + BALL_RADIUS > CANVAS_WIDTH || ballPos.current.x - BALL_RADIUS < 0) {
      ballVel.current.dx *= -1;
    }
    if (ballPos.current.y - BALL_RADIUS < 0) {
      ballVel.current.dy *= -1;
    }

    // Paddle Collision
    if (
      ballPos.current.y + BALL_RADIUS > CANVAS_HEIGHT - PADDLE_HEIGHT &&
      ballPos.current.x > paddlePos.current.x &&
      ballPos.current.x < paddlePos.current.x + PADDLE_WIDTH
    ) {
      ballVel.current.dy = -Math.abs(ballVel.current.dy);
      // Add some angle based on where it hits the paddle
      const hitPoint = (ballPos.current.x - (paddlePos.current.x + PADDLE_WIDTH / 2)) / (PADDLE_WIDTH / 2);
      ballVel.current.dx = hitPoint * 5;
    }

    // Bottom Collision (Life lost)
    if (ballPos.current.y + BALL_RADIUS > CANVAS_HEIGHT) {
      setLives((prev) => {
        const newLives = prev - 1;
        if (newLives <= 0) {
          setGameState("GAME_OVER");
        } else {
          resetBallAndPaddle();
        }
        return newLives;
      });
    }

    // Brick Collision
    bricks.current.forEach((brick) => {
      if (brick.status === 1) {
        if (
          ballPos.current.x > brick.x &&
          ballPos.current.x < brick.x + BRICK_WIDTH &&
          ballPos.current.y > brick.y &&
          ballPos.current.y < brick.y + BRICK_HEIGHT
        ) {
          brick.status = 0;
          ballVel.current.dy *= -1;
          setScore((prev) => prev + 10);
        }
      }
    });

    // Victory Check
    if (bricks.current.every((b) => b.status === 0)) {
      setGameState("VICTORY");
    }

    // Update visual state for Anvil hooks
    setPaddleX(paddlePos.current.x);
  };

  const draw = (ctx: CanvasRenderingContext2D) => {
    ctx.clearRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

    // Background Gradient
    const grad = ctx.createLinearGradient(0, 0, 0, CANVAS_HEIGHT);
    grad.addColorStop(0, "#0f172a");
    grad.addColorStop(1, "#1e293b");
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

    // Bricks
    bricks.current.forEach((brick) => {
      if (brick.status === 1) {
        ctx.fillStyle = brick.color;
        ctx.beginPath();
        ctx.roundRect(brick.x, brick.y, BRICK_WIDTH, BRICK_HEIGHT, 4);
        ctx.fill();
        // Shine effect
        ctx.fillStyle = "rgba(255, 255, 255, 0.3)";
        ctx.fillRect(brick.x + 2, brick.y + 2, BRICK_WIDTH - 4, 4);
      }
    });

    // Paddle
    ctx.fillStyle = "#f8fafc";
    ctx.beginPath();
    ctx.roundRect(paddlePos.current.x, CANVAS_HEIGHT - PADDLE_HEIGHT, PADDLE_WIDTH, PADDLE_HEIGHT, 10);
    ctx.fill();

    // Ball
    ctx.fillStyle = "#fbbf24";
    ctx.beginPath();
    ctx.arc(ballPos.current.x, ballPos.current.y, BALL_RADIUS, 0, Math.PI * 2);
    ctx.fill();
    // Ball glow
    ctx.shadowBlur = 15;
    ctx.shadowColor = "#fbbf24";
    ctx.stroke();
    ctx.shadowBlur = 0;
  };

  const loop = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    update();
    draw(ctx);
    requestRef.current = requestAnimationFrame(loop);
  };

  useEffect(() => {
    requestRef.current = requestAnimationFrame(loop);
    return () => {
      if (requestRef.current) cancelAnimationFrame(requestRef.current);
    };
  }, [gameState]);

  return (
    <div className="min-h-screen bg-slate-900 text-white flex flex-col items-center justify-center p-4 font-sans" 
         data-anvil-state={JSON.stringify({ score, lives, paddleX, gameState })}>
      
      <div className="mb-6 text-center">
        <h1 className="text-5xl font-black tracking-tighter italic text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-emerald-400 uppercase mb-2">
          Neon Breakout
        </h1>
        <div className="flex gap-8 justify-center text-xl font-mono">
          <span>SCORE: <span className="text-yellow-400">{score}</span></span>
          <span>LIVES: <span className="text-red-400">{lives}</span></span>
        </div>
      </div>

      <div className="relative group">
        <canvas
          ref={canvasRef}
          width={CANVAS_WIDTH}
          height={CANVAS_HEIGHT}
          className="border-4 border-slate-700 rounded-xl shadow-2xl bg-slate-800 cursor-none"
        />

        {gameState === "MENU" && (
          <div className="absolute inset-0 flex items-center justify-center bg-slate-900/80 backdrop-blur-sm rounded-lg">
            <button 
              onClick={startGame}
              data-anvil-action="primary"
              className="px-12 py-4 bg-blue-600 hover:bg-blue-500 text-white text-2xl font-bold rounded-full transition-all transform hover:scale-110 active:scale-95 shadow-[0_0_20px_rgba(37,99,235,0.5)]"
            >
              START GAME
            </button>
          </div>
        )}

        {gameState === "GAME_OVER" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-red-900/80 backdrop-blur-sm rounded-lg">
            <h2 className="text-6xl font-black text-white mb-6 drop-shadow-md">GAME OVER</h2>
            <button 
              onClick={restartGame}
              data-anvil-action="restart"
              className="px-8 py-3 bg-white text-red-900 text-xl font-bold rounded-full transition-all transform hover:scale-110 active:scale-95"
            >
              TRY AGAIN
            </button>
          </div>
        )}

        {gameState === "VICTORY" && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-emerald-900/80 backdrop-blur-sm rounded-lg">
            <h2 className="text-6xl font-black text-white mb-6 drop-shadow-md">VICTORY!</h2>
            <p className="text-2xl mb-6">Final Score: {score}</p>
            <button 
              onClick={restartGame}
              data-anvil-action="restart"
              className="px-8 py-3 bg-white text-emerald-900 text-xl font-bold rounded-full transition-all transform hover:scale-110 active:scale-95"
            >
              PLAY AGAIN
            </button>
          </div>
        )}
      </div>

      <div className="mt-8 text-slate-400 text-sm flex gap-6">
        <span>Move Mouse to control paddle</span>
        <span>Press <kbd className="px-2 py-1 bg-slate-700 rounded text-white">R</kbd> to restart</span>
      </div>
    </div>
  );
}
