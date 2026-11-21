export type GameState = 'START' | 'PLAYING' | 'GAMEOVER' | 'VICTORY';

export interface Entity {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Player extends Entity {
  speed: number;
}

export interface Alien extends Entity {
  type: 'standard' | 'elite' | 'boss';
  points: number;
}

export interface Projectile extends Entity {
  velocity: number;
  owner: 'player' | 'alien';
}

export interface GameWorld {
  player: Player;
  aliens: Alien[];
  projectiles: Projectile[];
  score: number;
  level: number;
  status: GameState;
  alienDirection: number; // 1 for right, -1 for left
  alienMoveTimer: number;
}

export const INITIAL_WORLD: GameWorld = {
  player: { id: 'player', x: 400, y: 550, width: 40, height: 20, speed: 5 },
  aliens: [],
  projectiles: [],
  score: 0,
  level: 1,
  status: 'START',
  alienDirection: 1,
  alienMoveTimer: 0,
};

export function createAlienGrid(level: number): Alien[] {
  const aliens: Alien[] = [];
  const cols = 8;
  const rows = 3 + Math.min(level, 2);
  const spacingX = 60;
  const spacingY = 40;
  const startX = 100;
  const startY = 50;

  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      aliens.push({
        id: `alien-${r}-${c}`,
        x: startX + c * spacingX,
        y: startY + r * spacingY,
        width: 30,
        height: 20,
        type: r === 0 ? 'elite' : 'standard',
        points: r === 0 ? 20 : 10,
      });
    }
  }
  return aliens;
}

export function updateGame(world: GameWorld, input: { left: boolean; right: boolean; shoot: boolean }): GameWorld {
  if (world.status !== 'PLAYING') return world;

  const nextWorld = { ...world, projectiles: [...world.projectiles] };

  // 1. Player Movement
  const player = { ...nextWorld.player };
  if (input.left && player.x > 0) player.x -= player.speed;
  if (input.right && player.x < 800 - player.width) player.x += player.speed;
  nextWorld.player = player;

  // 2. Projectiles Movement
  nextWorld.projectiles = nextWorld.projectiles
    .map(p => ({ ...p, y: p.y + p.velocity }))
    .filter(p => p.y > -50 && p.y < 650);

  // 3. Alien Movement
  let movedDown = false;
  const aliens = nextWorld.aliens.map(a => ({ ...a }));
  
  // Check if any alien hit the edge
  const hitEdge = aliens.some(a => (world.alienDirection === 1 && a.x + a.width > 780) || (world.alienDirection === -1 && a.x < 20));

  if (hitEdge) {
    nextWorld.alienDirection *= -1;
    movedDown = true;
  }

  // Update alien positions
  const moveSpeed = 1 + world.level * 0.2;
  nextWorld.aliens = aliens.map(a => ({
    ...a,
    x: a.x + world.alienDirection * moveSpeed,
    y: movedDown ? a.y + 20 : a.y,
  }));

  // 4. Collision Detection
  const remainingAliens: Alien[] = [];
  const remainingProjectiles: Projectile[] = [];
  let newScore = nextWorld.score;
  let gameOver = false;

  nextWorld.projectiles.forEach(p => {
    if (p.owner === 'player') {
      let hit = false;
      nextWorld.aliens.forEach(a => {
        if (
          p.x < a.x + a.width &&
          p.x + p.width > a.x &&
          p.y < a.y + a.height &&
          p.y + p.height > a.y
        ) {
          hit = true;
          newScore += a.points;
        }
      });
      if (!hit) remainingProjectiles.push(p);
    } else {
      // Alien projectile vs Player
      if (
        p.x < player.x + player.width &&
        p.x + p.width > player.x &&
        p.y < player.y + player.height &&
        p.y + p.height > player.y
      ) {
        gameOver = true;
      }
      remainingProjectiles.push(p);
    }
  });

  // Filter out destroyed aliens
  nextWorld.aliens = nextWorld.aliens.filter(a => {
    const isDestroyed = nextWorld.projectiles.some(p => 
      p.owner === 'player' && 
      p.x < a.x + a.width && 
      p.x + p.width > a.x && 
      p.y < a.y + a.height && 
      p.y + p.height > a.y
    );
    return !isDestroyed;
  });

  nextWorld.projectiles = remainingProjectiles;
  nextWorld.score = newScore;

  // 5. Win/Loss Conditions
  if (gameOver || nextWorld.aliens.some(a => a.y + a.height > player.y)) {
    nextWorld.status = 'GAMEOVER';
  } else if (nextWorld.aliens.length === 0) {
    nextWorld.status = 'VICTORY';
  }

  return nextWorld;
}

export function fireProjectile(world: GameWorld, owner: 'player' | 'alien', x: number, y: number): GameWorld {
  const newProjectile: Projectile = {
    id: `proj-${Date.now()}-${Math.random()}`,
    x: x + 18, 
    y: y,
    width: 4,
    height: 10,
    velocity: owner === 'player' ? -7 : 4,
    owner: owner,
  };

  return {
    ...world,
    projectiles: [...world.projectiles, newProjectile],
  };
}
