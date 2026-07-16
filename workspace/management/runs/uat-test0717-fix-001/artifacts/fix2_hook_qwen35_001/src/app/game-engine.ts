// ============================================================
// Space Invaders Game Engine — src/app/game-engine.ts
// Core game logic: game loop, player ship, alien grid,
// projectiles, collision detection, score tracking,
// and game state management.
// ============================================================

// ---------- Constants ----------
const CANVAS_W = 800;
const CANVAS_H = 600;
const PLAYER_W = 40;
const PLAYER_H = 24;
const PLAYER_SPEED = 5;
const BULLET_W = 4;
const BULLET_H = 12;
const BULLET_SPEED = 7;
const ALIEN_COLS = 11;
const ALIEN_ROWS = 5;
const ALIEN_W = 32;
const ALIEN_H = 24;
const ALIEN_PAD = 12;
const ALIEN_STEP = 1; // pixels per frame for movement
const ALIEN_DROP = 20;
const ALIEN_SHOOT_CHANCE = 0.003; // per frame per alien
const PLAYER_BULLET_MAX = 2; // max simultaneous player bullets
const PLAYER_Y = CANVAS_H - 50;
const PLAYER_X_CENTER = CANVAS_W / 2;
const SCORE_PER_ALIEN = 100;
const WIN_SCORE = ALIEN_COLS * ALIEN_ROWS * SCORE_PER_ALIEN;
const ALIEN_BASE_SPEED = 0.5;

// ---------- Types ----------
export interface Player {
  x: number;
  y: number;
  lives: number;
}

export interface Alien {
  id: number;
  x: number;
  y: number;
  alive: boolean;
  row: number;
  col: number;
}

export interface Bullet {
  x: number;
  y: number;
  dy: number; // direction: -1 for up, 1 for down
  active: boolean;
  source: "player" | "alien";
}

export interface Particle {
  x: number;
  y: number;
  dx: number;
  dy: number;
  life: number;
  maxLife: number;
  color: string;
  size: number;
}

export type GameState = "idle" | "playing" | "paused" | "gameover" | "victory";

export interface GameData {
  player: Player;
  aliens: Alien[];
  bullets: Bullet[];
  particles: Particle[];
  score: number;
  lives: number;
  direction: number; // -1 left, 1 right
  alienSpeed: number;
  frameCount: number;
  state: GameState;
  highScore: number;
  wave: number;
  alienMoveTimer: number;
  keys: Record<string, boolean>;
}

// ---------- Helpers ----------
function createAliens(wave: number): Alien[] {
  const aliens: Alien[] = [];
  const startX = (CANVAS_W - (ALIEN_COLS * (ALIEN_W + ALIEN_PAD))) / 2;
  for (let row = 0; row < ALIEN_ROWS; row++) {
    for (let col = 0; col < ALIEN_COLS; col++) {
      aliens.push({
        id: aliens.length,
        x: startX + col * (ALIEN_W + ALIEN_PAD),
        y: 60 + row * (ALIEN_H + ALIEN_PAD),
        alive: true,
        row,
        col,
      });
    }
  }
  return aliens;
}

function createPlayer(): Player {
  return {
    x: PLAYER_X_CENTER - PLAYER_W / 2,
    y: PLAYER_Y,
    lives: 3,
  };
}

function createBullet(x: number, y: number, dy: number, source: "player" | "alien"): Bullet {
  return { x, y, dy, active: true, source };
}

function spawnParticles(x: number, y: number, count: number, color: string): Particle[] {
  const particles: Particle[] = [];
  for (let i = 0; i < count; i++) {
    const angle = Math.random() * Math.PI * 2;
    const speed = 1 + Math.random() * 3;
    particles.push({
      x,
      y,
      dx: Math.cos(angle) * speed,
      dy: Math.sin(angle) * speed,
      life: 30 + Math.random() * 20,
      maxLife: 50,
      color,
      size: 2 + Math.random() * 3,
    });
  }
  return particles;
}

function rectsOverlap(
  ax: number, ay: number, aw: number, ah: number,
  bx: number, by: number, bw: number, bh: number
): boolean {
  return ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by;
}

// ---------- Public API ----------

export function initGame(): GameData {
  return {
    player: createPlayer(),
    aliens: createAliens(1),
    bullets: [],
    particles: [],
    score: 0,
    lives: 3,
    direction: 1,
    alienSpeed: ALIEN_BASE_SPEED,
    frameCount: 0,
    state: "idle",
    highScore: 0,
    wave: 1,
    alienMoveTimer: 0,
    keys: {},
  };
}

export function resetGameData(gd: GameData): GameData {
  gd.player = createPlayer();
  gd.aliens = createAliens(gd.wave);
  gd.bullets = [];
  gd.particles = [];
  gd.score = 0;
  gd.lives = 3;
  gd.direction = 1;
  gd.alienSpeed = ALIEN_BASE_SPEED;
  gd.frameCount = 0;
  gd.state = "idle";
  gd.wave = 1;
  gd.alienMoveTimer = 0;
  return gd;
}

export function startGame(gd: GameData): GameData {
  gd.state = "playing";
  return gd;
}

export function restartGame(gd: GameData): GameData {
  resetGameData(gd);
  gd.state = "playing";
  return gd;
}

export function pauseGame(gd: GameData): GameData {
  gd.state = gd.state === "playing" ? "paused" : "playing";
  return gd;
}

export function nextWave(gd: GameData): GameData {
  gd.wave++;
  gd.aliens = createAliens(gd.wave);
  gd.bullets = [];
  gd.alienSpeed = ALIEN_BASE_SPEED + gd.wave * 0.15;
  gd.direction = 1;
  gd.alienMoveTimer = 0;
  return gd;
}

export function onKeyDown(key: string, gd: GameData): GameData {
  gd.keys[key] = true;
  if (key === " " || key === "Space") {
    // Space to start/restart
    if (gd.state === "idle" || gd.state === "gameover") {
      if (gd.state === "gameover") {
        restartGame(gd);
      } else {
        startGame(gd);
      }
    }
  }
  if (key === "p" || key === "P" || key === "Escape") {
    pauseGame(gd);
  }
  return gd;
}

export function onKeyUp(key: string, gd: GameData): GameData {
  gd.keys[key] = false;
  return gd;
}

export function updateGame(gd: GameData): GameData {
  if (gd.state !== "playing") return gd;

  gd.frameCount++;

  // --- Player movement ---
  if (gd.keys["ArrowLeft"] || gd.keys["a"]) {
    gd.player.x = Math.max(0, gd.player.x - PLAYER_SPEED);
  }
  if (gd.keys["ArrowRight"] || gd.keys["d"]) {
    gd.player.x = Math.min(CANVAS_W - PLAYER_W, gd.player.x + PLAYER_SPEED);
  }

  // --- Player shooting ---
  if ((gd.keys[" "] || gd.keys["ArrowUp"] || gd.keys["w"]) && gd.frameCount % 12 === 0) {
    const activeBullets = gd.bullets.filter((b) => b.active && b.source === "player");
    if (activeBullets.length < PLAYER_BULLET_MAX) {
      gd.bullets.push(createBullet(
        gd.player.x + PLAYER_W / 2 - BULLET_W / 2,
        gd.player.y - BULLET_H,
        -BULLET_SPEED,
        "player"
      ));
    }
  }

  // --- Update bullets ---
  for (const bullet of gd.bullets) {
    if (!bullet.active) continue;
    bullet.y += bullet.dy;
    if (bullet.y < 0 || bullet.y > CANVAS_H) {
      bullet.active = false;
    }
  }

  // --- Alien movement ---
  gd.alienMoveTimer++;
  const aliveAliens = gd.aliens.filter((a) => a.alive);
  if (aliveAliens.length === 0) {
    nextWave(gd);
    return gd;
  }

  // Check if aliens need to change direction or drop
  let shouldDrop = false;
  for (const alien of aliveAliens) {
    const nextX = alien.x + gd.direction * gd.alienSpeed * 8;
    if (nextX <= 0 || nextX + ALIEN_W >= CANVAS_W) {
      shouldDrop = true;
      break;
    }
  }

  if (shouldDrop) {
    gd.direction *= -1;
    for (const alien of aliveAliens) {
      alien.x += gd.direction * gd.alienSpeed * 8;
      alien.y += ALIEN_DROP;
    }
  } else {
    for (const alien of aliveAliens) {
      alien.x += gd.direction * gd.alienSpeed * 8;
    }
  }

  // --- Alien shooting ---
  for (const alien of aliveAliens) {
    if (Math.random() < ALIEN_SHOOT_CHANCE * (1 + gd.wave * 0.2)) {
      gd.bullets.push(createBullet(
        alien.x + ALIEN_W / 2 - BULLET_W / 2,
        alien.y + ALIEN_H,
        BULLET_SPEED * 0.6,
        "alien"
      ));
    }
  }

  // --- Collision detection ---
  for (const bullet of gd.bullets) {
    if (!bullet.active) continue;

    if (bullet.source === "player") {
      for (const alien of aliveAliens) {
        if (rectsOverlap(
          bullet.x, bullet.y, BULLET_W, BULLET_H,
          alien.x, alien.y, ALIEN_W, ALIEN_H
        )) {
          bullet.active = false;
          alien.alive = false;
          gd.score += SCORE_PER_ALIEN;
          gd.particles.push(...spawnParticles(
            alien.x + ALIEN_W / 2,
            alien.y + ALIEN_H / 2,
            12,
            `hsl(${120 + Math.random() * 60}, 100%, 60%)`
          ));
          break;
        }
      }
    } else if (bullet.source === "alien") {
      // Check collision with player
      if (rectsOverlap(
        bullet.x, bullet.y, BULLET_W, BULLET_H,
        gd.player.x, gd.player.y, PLAYER_W, PLAYER_H
      )) {
        bullet.active = false;
        gd.lives--;
        gd.particles.push(...spawnParticles(
          gd.player.x + PLAYER_W / 2,
          gd.player.y + PLAYER_H / 2,
          15,
          `hsl(0, 100%, 60%)`
        ));
        if (gd.lives <= 0) {
          gd.state = "gameover";
          if (gd.score > gd.highScore) {
            gd.highScore = gd.score;
          }
        }
      }
    }
  }

  // --- Update particles ---
  for (const p of gd.particles) {
    p.x += p.dx;
    p.y += p.dy;
    p.life--;
    p.dy += 0.05; // gravity
  }
  gd.particles = gd.particles.filter((p) => p.life > 0);

  // --- Clean up dead bullets ---
  gd.bullets = gd.bullets.filter((b) => b.active);

  // --- Check if all aliens dead (wave cleared) ---
  if (gd.aliens.filter((a) => a.alive).length === 0 && gd.state === "playing") {
    nextWave(gd);
  }

  // --- Check if aliens reached player level ---
  for (const alien of aliveAliens) {
    if (alien.y + ALIEN_H >= gd.player.y) {
      gd.state = "gameover";
      if (gd.score > gd.highScore) {
        gd.highScore = gd.score;
      }
      break;
    }
  }

  return gd;
}

export function getRenderData(gd: GameData) {
  return {
    canvasWidth: CANVAS_W,
    canvasHeight: CANVAS_H,
    player: gd.player,
    aliens: gd.aliens.filter((a) => a.alive),
    bullets: gd.bullets.filter((b) => b.active),
    particles: gd.particles,
    score: gd.score,
    lives: gd.lives,
    state: gd.state,
    highScore: gd.highScore,
    wave: gd.wave,
  };
}
