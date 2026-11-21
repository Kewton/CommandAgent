export type Vector2D = { x: number; y: number };

export type GameStatus = 'IDLE' | 'PLAYING' | 'WON' | 'GAMEOVER';

export type EntityType = 'PLAYER' | 'ALIEN' | 'BULLET';

export interface Entity {
  id: string;
  type: EntityType;
  position: Vector2D;
  size: Vector2D;
  velocity: Vector2D;
}

export interface GameState {
  status: GameStatus;
  player: Entity;
  aliens: Entity[];
  bullets: Entity[];
  score: number;
  level: number;
  alienDirection: 1 | -1;
}
