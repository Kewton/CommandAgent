import { GameState, Entity, Vector2D, GameStatus } from './types';

const CANVAS_WIDTH = 800;
const CANVAS_HEIGHT = 600;
const PLAYER_SPEED = 5;
const BULLET_SPEED = 7;
const ALIEN_SPEED = 1;
const ALIEN_DROP = 20;

export function createEntity(type: 'PLAYER' | 'ALIEN' | 'BULLET', x: number, y: number, w: number, h: number): Entity {
  return {
    id: Math.random().toString(36).substr(2, 9),
    type,
    position: { x, y },
    size: { x: w, y: h },
    velocity: { x: 0, y: 0 },
  };
}

export function initGame(): GameState {
  const aliens: Entity[] = [];
  const rows = 5;
  const cols = 11;
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      aliens.push(createEntity('ALIEN', 100 + c * 50, 50 + r * 40, 30, 20));
    }
  }

  return {
    status: 'IDLE',
    player: createEntity('PLAYER', CANVAS_WIDTH / 2 - 20, CANVAS_HEIGHT - 50, 40, 20),
    aliens,
    bullets: [],
    score: 0,
    level: 1,
    alienDirection: 1,
  };
}

export function updateGame(state: GameState, input: { left: boolean; right: boolean; shoot: boolean }): GameState {
  if (state.status !== 'PLAYING') return state;

  const newState = { ...state, bullets: [...state.bullets], aliens: [...state.aliens] };

  // Player Movement
  if (input.left && newState.player.position.x > 0) {
    newState.player.position.x -= PLAYER_SPEED;
  }
  if (input.right && newState.player.position.x < CANVAS_WIDTH - newState.player.size.x) {
    newState.player.position.x += PLAYER_SPEED;
  }

  // Shooting
  if (input.shoot && newState.bullets.length < 3) {
    const bullet = createEntity('BULLET', newState.player.position.x + newState.player.size.x / 2 - 2, newState.player.position.y, 4, 10);
    bullet.velocity = { x: 0, y: -BULLET_SPEED };
    newState.bullets.push(bullet);
  }

  // Bullets Update
  newState.bullets = newState.bullets
    .map(b => ({ ...b, position: { x: b.position.x + b.velocity.x, y: b.position.y + b.velocity.y } }))
    .filter(b => b.position.y > 0);

  // Aliens Movement
  let hitBoundary = false;
  newState.aliens = newState.aliens.map(a => {
    const nextX = a.position.x + ALIEN_SPEED * state.alienDirection;
    if (nextX < 0 || nextX > CANVAS_WIDTH - a.size.x) hitBoundary = true;
    return { ...a, position: { ...a.position, x: nextX } };
  });

  if (hitBoundary) {
    newState.alienDirection = (state.alienDirection === 1 ? -1 : 1) as 1 | -1;
    newState.aliens = newState.aliens.map(a => ({ ...a, position: { ...a.position, y: a.position.y + ALIEN_DROP } }));
  }

  // Collisions
  const remainingAliens: Entity[] = [];
  let scoreIncrease = 0;
  
  for (const alien of newState.aliens) {
    let hit = false;
    newState.bullets = newState.bullets.filter(bullet => {
      if (
        bullet.position.x < alien.position.x + alien.size.x &&
        bullet.position.x + bullet.size.x > alien.position.x &&
        bullet.position.y < alien.position.y + alien.size.y &&
        bullet.position.y + bullet.size.y > alien.position.y
      ) {
        hit = true;
        return false;
      }
      return true;
    });
    if (!hit) remainingAliens.push(alien);
    else scoreIncrease += 10;
  }
  
  newState.aliens = remainingAliens;
  newState.score += scoreIncrease;

  // Game Over / Win
  if (newState.aliens.length === 0) {
    newState.status = 'WON';
  } else if (newState.aliens.some(a => a.position.y + a.size.y > newState.player.position.y)) {
    newState.status = 'GAMEOVER';
  }

  return newState;
}
