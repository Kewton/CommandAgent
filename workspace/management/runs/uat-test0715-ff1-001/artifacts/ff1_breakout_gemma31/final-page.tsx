"use client";

import React, { useState, useEffect, useRef } from "react";

// Constants
const CANVAS_WIDTH = 800;
const CANVAS_HEIGHT = 600;
const PADDLE_WIDTH = 100;
const PADDLE_HEIGHT = 20;
const BALL_RADIUS = 8;
const BRICK_ROWS = 5;
const BRICK_COLS = 8;
const BRICK_PADDING = 10;
const BRICK_OFFSET_TOP = 60;
const BRICK_OFFSET_LEFT = 30;
const BRICK_WIDTH = (CANVAS_WIDTH - BRICK_OFFSET_LEFT * 2 - BRICK_COLS * BRICK_PADDING) / BRICK_COLS;
const BRICK_HEIGHT = 20;

type GameState = "MENU" | "PLAYING" | "GAME_OVER" | "VICTORY";

interface Brick {
  x: number;
  y: number;
  status: number; // 1 = active, 0 = broken
}

export default function BreakoutGame() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [gameState, setGameState] = useState<GameState>("MENU");
  const [score, setScore] = useState(0);
  const [lives, setLives] = useState(3);
  const [paddleX, setPaddleX] = useState((CANVAS_WIDTH - PADDLE_WIDTH) / 2);

  // Game engine refs to avoid React render cycle for physics
  const gameRef = useRef({
    ballX: CANVAS_WIDTH / 2,
    ballY: CANVAS_HEIGHT - 50,
    ballDX: 4,
    ballDY: -4,
    paddleXInternal: (CANVAS_WIDTH - PADDLE_WIDTH) / 2,
    bricks: [] as Brick[],
    requestAnimationFrameId: 0,
  });

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
    return bricks;
  };

  const startGame = () => {
    gameRef.current.ballX = CANVAS_WIDTH / 2;
    gameRef.current.ballY = CANVAS_HEIGHT - 50;
    gameRef.current.ballDX = 4;
    gameRef.current.ballDY = -4;
    gameRef.current.paddleXInternal = (CANVAS_WIDTH - PADDLE_WIDTH) / 2;
    gameRef.current.bricks = initBricks();
    setScore(0);
    setLives(3);
    setPaddleX((CANVAS_WIDTH - PADDLE_WIDTH) / 2);
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
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const update = () => {
      if (gameState !== "PLAYING") return;

      const g = gameRef.current;

      // Ball movement
      g.ballX += g.ballDX;
      g.ballY += g.ballDY;

      // Wall collisions
      if (g.ballX + BALL_RADIUS > CANVAS_WIDTH || g.ballX - BALL_RADIUS < 0) {
        g.ballDX *= -1;
      }
      if (g.ballY - BALL_RADIUS < 0) {
        g.ballDY *= -1;
      }

      // Paddle collision
      if (
        g.ballY + BALL_RADIUS > CANVAS_HEIGHT - PADDLE_HEIGHT &&
        g.ballX > g.paddleXInternal &&
        g.ballX < g.paddleXInternal + PADDLE_WIDTH
      ) {
        g.ballDY *= -1;
        // Add slight variance based on where it hit the paddle
        const hitPoint = (g.ballX - (g.paddleXInternal + PADDLE_WIDTH / 2)) / (PADDLE_WIDTH / 2);
        g.ballDX += hitPoint * 1;
      }

      // Bottom collision (lose life)
      if (g.ballY + BALL_RADIUS > CANVAS_HEIGHT) {
        setLives((prev) => {
          const next = prev - 1;
          if (next <= 0) {
            setGameState("GAME_OVER");
          }
          return next;
        });
        g.ballX = CANVAS_WIDTH / 2;
        g.ballY = CANVAS_HEIGHT - 50;
        g.ballDY = -4;
      }

      // Brick collisions
      let bricksLeft = 0;
      g.bricks.forEach((brick) => {
        if (brick.status === 1) {
          bricksLeft++;
          if (
            g.ballX > brick.x &&
            g.ballX < brick.x + BRICK_WIDTH &&
            g.ballY > brick.y &&
            g.ballY < brick.y + BRICK_HEIGHT
          ) {
            brick.status = 0;
            g.ballDY *= -1;
            setScore((s) => s + 10);
          }
        }
      });

      if (bricksLeft === 0) {
        setGameState("VICTORY");
      }

      // Mirror physics internal state to React state for observability/rendering
      setPaddleX(g.paddleXInternal);
    };

    const draw = () => {
      ctx.clearRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

      if (gameState === "MENU") {
        ctx.fillStyle = "white";
        ctx.font = "30px Arial";
        ctx.textAlign = "center";
        ctx.fillText("NEON BREAKOUT", CANVAS_WIDTH / 2, CANVAS_HEIGHT / 2 - 20);
        ctx.font = "20px Arial";
        ctx.fillText("Press Start to Play", CANVAS_WIDTH / 2, CANVAS_HEIGHT / 2 + 20);
      } else {
        // Draw Bricks
        gameRef.current.bricks.forEach((brick) => {
          if (brick.status === 1) {
            ctx.fillStyle = "#00f2ff";
            ctx.fillRect(brick.x, brick.y, BRICK_WIDTH, BRICK_HEIGHT);
            ctx.strokeStyle = "#fff";
            ctx.strokeRect(brick.x, brick.y, BRICK_WIDTH, BRICK_HEIGHT);
          }
        });

        // Draw Paddle
        ctx.fillStyle = "#ff00ff";
        ctx.fillRect(gameRef.current.paddleXInternal, CANVAS_HEIGHT - PADDLE_HEIGHT, PADDLE_WIDTH, PADDLE_HEIGHT);

        // Draw Ball
        ctx.beginPath();
        ctx.arc(gameRef.current.ballX, gameRef.current.ballY, BALL_RADIUS, 0, Math.PI * 2);
        ctx.fillStyle = "#ffff00";
        ctx.fill();
        ctx.closePath();

        // Draw UI
        ctx.fillStyle = "white";
        ctx.font = "20px Arial";
        ctx.textAlign = "left";
        ctx.fillText(`Score: ${score}`, 20, 30);
        ctx.textAlign = "right";
        ctx.fillText(`Lives: ${lives}`, CANVAS_WIDTH - 20, 30);
      }

      if (gameState === "GAME_OVER") {
        ctx.fillStyle = "rgba(0,0,0,0.7)";
        ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);
        ctx.fillStyle = "red";
        ctx.font = "50px Arial";
        ctx.textAlign = "center";
        ctx.fillText("GAME OVER", CANVAS_WIDTH / 2, CANVAS_HEIGHT / 2);
      } else if (gameState === "VICTORY") {
        ctx.fillStyle = "rgba(0,0,0,0.7)";
        ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);
        ctx.fillStyle = "#00ff00";
        ctx.font = "50px Arial";
        ctx.textAlign = "center";
        ctx.fillText("VICTORY!", CANVAS_WIDTH / 2, CANVAS_HEIGHT / 2);
      }
    };

    const loop = () => {
      update();
      draw();
      gameRef.current.requestAnimationFrameId = requestAnimationFrame(loop);
    };

    gameRef.current.requestAnimationFrameId = requestAnimationFrame(loop);

    return () => cancelAnimationFrame(gameRef.current.requestAnimationFrameId);
  }, [gameState, score, lives]);

  const handleMouseMove = (e: React.MouseEvent | React.TouchEvent) => {
    if (gameState !== "PLAYING") return;
    let clientX: number;
    if ("touches" in e) {
      clientX = e.touches[0].clientX;
    } else {
      clientX = (e as React.MouseEvent).clientX;
    }

    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;

    const x = clientX - rect.left;
    const newPaddleX = Math.max(0, Math.min(CANVAS_WIDTH - PADDLE_WIDTH, x - PADDLE_WIDTH / 2));
    gameRef.current.paddleXInternal = newPaddleX;
  };

  return (
    <div className="min-h-screen bg-slate-900 flex flex-col items-center justify-center p-4 text-white font-sans">
      <h1 className="text-5xl font-black mb-8 italic tracking-tighter text-transparent bg-clip-text bg-gradient-to-r from-cyan-400 to-fuchsia-500">
        NEON BREAKOUT
      </h1>

      <div 
        className="relative shadow-2xl border-4 border-slate-700 rounded-lg overflow-hidden"
        data-anvil-state={JSON.stringify({ 
            score, 
            lives, 
            paddleX, 
            gameState 
        })}
      >
        <canvas
          ref={canvasRef}
          width={CANVAS_WIDTH}
          height={CANVAS_HEIGHT}
          onMouseMove={handleMouseMove}
          onTouchMove={handleMouseMove}
          className="bg-black cursor-none"
        />

        {gameState === "MENU" && (
          <div className="absolute inset-0 flex items-center justify-center bg-black/60">
            <button 
              onClick={startGame} 
              data-anvil-action="primary"
              className="px-8 py-4 bg-cyan-500 hover:bg-cyan-400 text-black font-bold rounded-full transition-all transform hover:scale-110 shadow-[0_0_20px_rgba(6,182,212,0.5)]"
            >
              START GAME
            </button>
          </div>
        )}

        {(gameState === "GAME_OVER" || gameState === "VICTORY") && (
          <div className="absolute inset-0 flex items-center justify-center bg-black/60">
            <button 
              onClick={restartGame} 
              data-anvil-action="restart"
              className="px-8 py-4 bg-fuchsia-600 hover:bg-fuchsia-500 text-white font-bold rounded-full transition-all transform hover:scale-110 shadow-[0_0_20px_rgba(192,38,211,0.5)]"
            >
              PLAY AGAIN
            </button>
          </div>
        )}
      </div>

      <div className="mt-8 text-slate-400 text-center space-y-2">
        <p>Move mouse or touch to control paddle</p>
        <p className="text-sm italic opacity-50">Press 'R' at any time to restart</p>
      </div>
    </div>
  );
}
