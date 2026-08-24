
export interface Point {
  x: number;
  y: number;
}

export interface GameState {
  score: number;
  lives: number;
  wave: number;
  status: 'MENU' | 'PLAYING' | 'GAMEOVER' | 'VICTORY';
  playerX: number;
}

export class Bullet {
  x: number;
  y: number;
  speed: number;
  isEnemy: boolean;

  constructor(x: number, y: number, speed: number, isEnemy: boolean) {
    this.x = x;
    this.y = y;
    this.speed = speed;
    this.isEnemy = isEnemy;
  }

  update() {
    this.y += this.speed;
  }
}

export class Invader {
  x: number;
  y: number;
  width: number;
  height: number;
  alive: boolean = true;

  constructor(x: number, y: number) {
    this.x = x;
    this.y = y;
    this.width = 30;
    this.height = 20;
  }

  update(dx: number, dy: number) {
    this.x += dx;
    this.y += dy;
  }
}

export class GameEngine {
  canvasWidth: number = 800;
  canvasHeight: number = 600;
  
  playerX: number = 400;
  playerY: number = 550;
  playerWidth: number = 40;
  playerHeight: number = 20;
  
  invaders: Invader[] = [];
  bullets: Bullet[] = [];
  
  invaderDirection: number = 1; 
  invaderSpeed: number = 1;
  invaderStepY: number = 20;
  
  score: number = 0;
  lives: number = 3;
  wave: number = 1;
  status: GameState['status'] = 'MENU';
  
  lastShootTime: number = 0;
  shootDelay: number = 400;

  constructor() {
    this.initInvaders();
  }

  initInvaders() {
    this.invaders = [];
    const rows = 5;
    const cols = 11;
    const spacing = 45;
    const offsetX = (this.canvasWidth - (cols * spacing)) / 2;
    const offsetY = 50;

    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        this.invaders.push(new Invader(offsetX + c * spacing, offsetY + r * spacing));
      }
    }
  }

  start() {
    this.status = 'PLAYING';
    this.score = 0;
    this.lives = 3;
    this.wave = 1;
    this.initInvaders();
    this.bullets = [];
  }

  restart() {
    this.start();
  }

  update(keys: Set<string>, timestamp: number) {
    if (this.status !== 'PLAYING') return;

    // Player movement
    if (keys.has('ArrowLeft')) this.playerX -= 5;
    if (keys.has('ArrowRight')) this.playerX += 5;
    this.playerX = Math.max(0, Math.min(this.canvasWidth - this.playerWidth, this.playerX));

    // Shooting
    if (keys.has(' ') && timestamp - this.lastShootTime > this.shootDelay) {
      this.bullets.push(new Bullet(this.playerX + this.playerWidth / 2, this.playerY, -7, false));
      this.lastShootTime = timestamp;
    }

    // Invaders movement
    let hitWall = false;
    for (const inv of this.invaders) {
      if (!inv.alive) continue;
      if ((inv.x + inv.width >= this.canvasWidth && this.invaderDirection > 0) || 
          (inv.x <= 0 && this.invaderDirection < 0)) {
        hitWall = true;
        break;
      }
    }

    if (hitWall) {
      this.invaderDirection *= -1;
      for (const inv of this.invaders) {
        inv.update(0, this.invaderStepY);
      }
      this.invaderSpeed += 0.1;
    } else {
      for (const inv of this.invaders) {
        inv.update(this.invaderSpeed * this.invaderDirection, 0);
      }
    }

    // Enemy shooting
    if (Math.random() < 0.01) {
      const aliveInvaders = this.invaders.filter(i => i.alive);
      if (aliveInvaders.length > 0) {
        const shooter = aliveInvaders[Math.floor(Math.random() * aliveInvaders.length)];
        this.bullets.push(new Bullet(shooter.x + shooter.width / 2, shooter.y + shooter.height, 4, true));
      }
    }

    // Bullet updates and collision
    for (let i = this.bullets.length - 1; i >= 0; i--) {
      const b = this.bullets[i];
      b.update();

      if (b.y < 0 || b.y > this.canvasHeight) {
        this.bullets.splice(i, 1);
        continue;
      }

      if (!b.isEnemy) {
        // Player bullet vs Invader
        for (const inv of this.invaders) {
          if (inv.alive && b.x >= inv.x && b.x <= inv.x + inv.width && b.y >= inv.y && b.y <= inv.y + inv.height) {
            inv.alive = false;
            this.score += 10;
            this.bullets.splice(i, 1);
            break;
          }
        }
      } else {
        // Enemy bullet vs Player
        if (b.x >= this.playerX && b.x <= this.playerX + this.playerWidth && b.y >= this.playerY && b.y <= this.playerY + this.playerHeight) {
          this.lives--;
          this.bullets.splice(i, 1);
          if (this.lives <= 0) {
            this.status = 'GAMEOVER';
          }
          continue;
        }
      }
    }

    // Check Victory
    if (this.invaders.every(inv => !inv.alive)) {
      this.wave++;
      this.initInvaders();
      this.invaderSpeed += 0.5;
      // Simplified: just keep playing, but maybe victory if wave 5 reached?
      if (this.wave > 5) {
        this.status = 'VICTORY';
      }
    }

    // Check Loss by contact
    for (const inv of this.invaders) {
      if (inv.alive && inv.y + inv.height >= this.playerY) {
        this.status = 'GAMEOVER';
      }
    }
  }

  getState(): GameState {
    return {
      score: this.score,
      lives: this.lives,
      wave: this.wave,
      status: this.status,
      playerX: this.playerX,
    };
  }
}
