"use client";

import React, { useEffect, useRef, useState } from "react";

type GameStatus = "START" | "PLAYING" | "GAME_OVER" | "VICTORY";

interface Brick {
  x: number;
  y: number;
  status: number; // 1 for active, 0 for destroyed
}

export default function BreakoutGame() {
  // Game Configuration
  const CANVAS_WIDTH = 800;
  const CANVAS_HEIGHT = 600;
  const PADDLE_WIDTH = 100;
  const PADDLE_HEIGHT = 20;
  const BALL_RADIUS = 10;
  const BRICK_ROWS = 5;
  const BRICK_COLS = 8;
  const BRICK_PADDING = 10;
  const BRICK_OFFSET_TOP = 60;
  const BRICK_OFFSET_LEFT = 35;
  const BRICK_WIDTH = (CANVAS_WIDTH - BRICK_OFFSET_LEFT * 2) / BRICK_COLS - BRICK_PADDING;
  const BRICK_HEIGHT = 25;

  // State
  const [gameState, setGameState] = useState<GameStatus>("START");
  const [score, setScore] = useState(0);
  const [lives, setLives] = useState(3);
  const [paddleX, setPaddleX] = useState((CANVAS_WIDTH - PADDLE_WIDTH) / 2);

  // Refs for game loop (to avoid React re-render lag in canvas)
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const requestRef = useRef<number>();
  
  // Mutable game state refs
  const ballPos = useRef({ x: CANVAS_WIDTH / 2, y: CANVAS_HEIGHT - 50 });
  const ballVel = useRef({ dx: 4, dy: -4 });
  const paddleXRef = useRef((CANVAS_WIDTH - PADDLE_WIDTH) / 2);
  const bricksRef = useRef<Brick[]>([]);
  const gameActiveRef = useRef(false);

  // Initialize Bricks
  const initBricks = () => {
    const bricks: Brick[] = [];
    for (let r = 0; r < BRICK_ROWS; r++) {
      for (let c = 0; c < BRICK_COLS; c++) {
        bricks.push({
          x: c * (BRICK_WIDTH + BRICK_PADDING) + BRICK_OFFSET_LEFT,
          y: r * (BRICK_HEIGHT + BRICK_PADDING) + BRICK_OFFSET_TOP,
          status: 1,
        });
      }
    }
    bricksRef.current = bricks;
  };

  // Reset game entities to starting positions
  const resetEntities = () => {
    ballPos.current = { x: CANVAS_WIDTH / 2, y: CANVAS_HEIGHT - 50 };
    ballVel.current = { dx: 4, dy: -4 };
    paddleXRef.current = (CANVAS_WIDTH - PADDLE_WIDTH) / 2;
    setPaddleX((CANVAS_WIDTH - PADDLE_WIDTH) / 2);
  };

  // Comprehensive Restart Function
  const restartGame = () => {
    setScore(0);
    setLives(3);
    initBricks();
    resetEntities();
    setGameState("PLAYING");
    gameActiveRef.current = true;
  };

  const startGame = () => {
    initBricks();
    resetEntities();
    setScore(0);
    setLives(3);
    setGameState("PLAYING");
    gameActiveRef.current = true;
  };

  // Input Handlers
  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!gameActiveRef.current) return;
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const root = document.documentElement;
      const mouseX = e.clientX - rect.left - root.scrollLeft;
      const nextX = Math.max(0, Math.min(CANVAS_WIDTH - PADDLE_WIDTH, mouseX - PADDLE_WIDTH / 2));
      paddleXRef.current = nextX;
      setPaddleX(nextX);
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "r" || e.key === "R") {
        restartGame();
      }
      if (!gameActiveRef.current) return;
      if (e.key === "ArrowLeft") {
        const nextX = Math.max(0, paddleXRef.current - 20);
        paddleXRef.current = nextX;
        setPaddleX(nextX);
      } else if (e.key === "ArrowRight") {
        const nextX = Math.min(CANVAS_WIDTH - PADDLE_WIDTH, paddleXRef.current + 20);
        paddleXRef.current = nextX;
        setPaddleX(nextX);
      }
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  // Game Loop
  useEffect(() => {
    const update = () => {
      if (!gameActiveRef.current) return;

      // Ball movement
      ballPos.current.x += ballVel.current.dx;
      ballPos.current.y += ballVel.current.dy;

      // Wall collisions (Left/Right)
      if (ballPos.current.x + BALL_RADIUS > CANVAS_WIDTH || ballPos.current.x - BALL_RADIUS < 0) {
        ballVel.current.dx *= -1;
      }
      // Top collision
      if (ballPos.current.y - BALL_RADIUS < 0) {
        ballVel.current.dy *= -1;
      }
      // Bottom collision (Paddle or Life loss)
      if (ballPos.current.y + BALL_RADIUS > CANVAS_HEIGHT) {
        if (
          ballPos.current.x > paddleXRef.current &&
          ballPos.current.x < paddleXRef.current + PADDLE_WIDTH
        ) {
          ballVel.current.dy *= -1;
          // Add slight randomness/angle based on hit position
          const hitPoint = (ballPos.current.x - (paddleXRef.current + PADDLE_WIDTH / 2)) / (PADDLE_WIDTH / 2);
          ballVel.current.dx = hitPoint * 5;
        } else {
          setLives((prev) => {
            const newLives = prev - 1;
            if (newLives <= 0) {
              setGameState("GAME_OVER");
              gameActiveRef.current = false;
            } else {
              resetEntities();
            }
            return newLives;
          });
        }
      }

      // Brick collisions
      bricksRef.current.forEach((brick) => {
        if (brick.status === 1) {
          if (
            ballPos.current.x > brick.x &&
            ballPos.current.x < brick.x + BRICK_WIDTH &&
            ballPos.current.y > brick.y &&
            ballPos.current.y < brick.y + BRICK_HEIGHT
          ) {
            ballVel.current.dy *= -1;
            brick.status = 0;
            setScore((s) => s + 10);
          }
        }
      });

      // Victory check
      if (bricksRef.current.every((b) => b.status === 0)) {
        setGameState("VICTORY");
        gameActiveRef.current = false;
      }
    };

    const draw = () => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;

      // Background
      ctx.fillStyle = "#0f172a";
      ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

      // Bricks
      bricksRef.current.forEach((brick) => {
        if (brick.status === 1) {
          const hue = (brick.y / BRICK_OFFSET_TOP) * 60; // gradient based on row
          ctx.fillStyle = `hsl(${hue + 200}, 70%, 50%)`;
          ctx.fillRect(brick.x, brick.y, BRICK_WIDTH, BRICK_HEIGHT);
          // Highlight effect
          ctx.strokeStyle = "rgba(255,255,255,0.3)";
          ctx.strokeRect(brick.x, brick.y, BRICK_WIDTH, BRICK_HEIGHT);
        }
      });

      // Paddle
      const grad = ctx.createLinearGradient(paddleXRef.current, 0, paddleXRef.current + PADDLE_WIDTH, 0);
      grad.addColorStop(0, "#3b82f6");
      grad.addColorStop(1, "#60a5fa");
      ctx.fillStyle = grad;
      ctx.beginPath();
      ctx.roundRect(paddleXRef.current, CANVAS_HEIGHT - PADDLE_HEIGHT - 10, PADDLE_WIDTH, PADDLE_HEIGHT, 5);
      ctx.fill();

      // Ball
      ctx.fillStyle = "#facc15";
      ctx.beginPath();
      ctx.arc(ballPos.current.x, ballPos.current.y, BALL_RADIUS, 0, Math.PI * 2);
      ctx.fill();
      // Glow effect for ball
      ctx.shadowBlur = 15;
      ctx.shadowColor = "#facc15";
      ctx.stroke();
      ctx.shadowBlur = 0;

      update();
      requestRef.current = requestAnimationFrame(draw);
    };

    requestRef.current = requestAnimationFrame(draw);
    return () => {
      if (requestRef.current) cancelAnimationFrame(requestRef.current);
    };
  }, []);

  return (
    <div 
      className="min-h-screen bg-slate-950 flex flex-col items-center justify-center p-4 font-sans text-white"
      data-anvil-state={JSON.stringify({ paddleX, score, lives, status: gameState })}
    >
      <div className="mb-6 text-center">
        <h1 className="text-5xl font-black italic tracking-tighter mb-2 bg-clip-text text-transparent bg-gradient-to-r from-blue-400 to-purple-500 uppercase">
          Neon Breakout
        </h1>
        <div className="flex gap-8 justify-center text-xl font-mono">
          <span>SCORE: <span className="text-yellow-400">{score}</span></span>
          <span>LIVES: <span className="text-red-400">{lives}</span></span>
        </div>
      </div>

      <div className="relative shadow-2xl shadow-blue-500/20 rounded-lg overflow-hidden border-4 border-slate-800">
        <canvas 
          ref={canvasRef} 
          width={CANVAS_WIDTH} 
          height={CANVAS_HEIGHT} 
          className="block"
        />

        {/* Overlays */}
        {(gameState === "START" || gameState === "GAME_OVER" || gameState === "VICTORY") && (
          <div className="absolute inset-0 flex items-center justify-center bg-slate-900/80 backdrop-blur-sm">
            <div className="text-center p-8 rounded-2xl bg-slate-800 border border-slate-700 shadow-xl max-w-md mx-4">
              {gameState === "START" && (
                <>
                  <h2 className="text-3xl font-bold mb-4">Ready to Break?</h2>
                  <p className="text-slate-400 mb-6">Move mouse or use arrows to control the paddle. Destroy all bricks!</p>
                  <button 
                    onClick={startGame}
                    data-anvil-action="primary"
                    className="px-8 py-3 bg-blue-600 hover:bg-blue-500 transition-colors rounded-full font-bold text-lg uppercase tracking-widest shadow-lg shadow-blue-900/50"
                  >
                    Start Game
                  </button>
                </>
              )}
              {gameState === "GAME_OVER" && (
                <>
                  <h2 className="text-4xl font-black text-red-500 mb-4 uppercase">Game Over</h2>
                  <p className="text-slate-300 mb-6 text-xl">Final Score: <span className="font-bold">{score}</span></p>
                  <button 
                    onClick={restartGame}
                    data-anvil-action="restart"
                    className="px-8 py-3 bg-red-600 hover:bg-red-500 transition-colors rounded-full font-bold text-lg uppercase tracking-widest shadow-lg shadow-red-900/50"
                  >
                    Try Again
                  </button>
                </>
              )}
              {gameState === "VICTORY" && (
                <>
                  <h2 className="text-4xl font-black text-green-500 mb-4 uppercase">Victory!</h2>
                  <p className="text-slate-300 mb-6 text-xl">You cleared the board!</p>
                  <button 
                    onClick={restartGame}
                    data-anvil-action="restart"
                    className="px-8 py-3 bg-green-600 hover:bg-green-500 transition-colors rounded-full font-bold text-lg uppercase tracking-widest shadow-lg shadow-green-900/50"
                  >
                    Play Again
                  </button>
                </>
              )}
            </div>
          </div>
        )}

        {/* In-Game Restart Affordance */}
        {gameState === "PLAYING" && (
          <div className="absolute top-4 right-4">
            <button 
              onClick={restartGame}
              data-anvil-action="restart"
              className="px-3 py-1 bg-slate-700/50 hover:bg-slate-600 text-xs font-medium rounded border border-slate-500 transition-all uppercase tracking-tighter"
            >
              Restart (R)
            </button>
          </div>
        )}
      </div>

      <div className="mt-8 text-slate-500 text-sm flex gap-4">
        <span>← → / Mouse to Move</span>
        <span>|</span>
        <span>R to Restart</span>
      </div>
    </div>
  );
}
