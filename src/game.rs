use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const BRICK_WIDTH: usize = 6;
const BRICK_GAP: usize = 1;
const BRICK_ROWS: usize = 10;
const BRICK_START_Y: f64 = 1.0;
const PADDLE_BASE_WIDTH: f64 = 9.0;
const PADDLE_SPEED: f64 = 3.0;
const BALL_SPEED_MIN: f64 = 0.6;
const BALL_SPEED_MAX: f64 = 2.0;
const GRACE_SPEED: f64 = 0.4;
const POWERUP_CHANCE: f64 = 0.05;
const POWERUP_FALL_SPEED: f64 = 0.35;
const INITIAL_LIVES: u8 = 3;

#[derive(PartialEq, Clone, Copy)]
pub enum GameStatus {
    Waiting,
    Playing,
    Paused,
    GameOver,
}

#[derive(Clone)]
pub struct Ball {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
}

pub struct Paddle {
    pub x: f64,
    pub width: f64,
}

#[derive(Clone, Copy)]
pub struct Brick {
    pub color_idx: usize,
    pub points: u32,
    pub hits: u8,
}

#[derive(Clone, Copy, PartialEq)]
pub enum PowerupKind {
    WidePaddle,
    MultiBall,
    SlowMo,
}

pub struct FallingPowerup {
    pub x: f64,
    pub y: f64,
    pub kind: PowerupKind,
}

pub struct ActiveEffect {
    pub kind: PowerupKind,
    pub remaining_ticks: u32,
}

pub struct GameState {
    pub status: GameStatus,
    pub balls: Vec<Ball>,
    pub paddle: Paddle,
    pub bricks: Vec<Vec<Option<Brick>>>,
    pub brick_cols: usize,
    pub score: u32,
    pub lives: u8,
    pub level: u32,
    pub powerups: Vec<FallingPowerup>,
    pub effects: Vec<ActiveEffect>,
    pub width: f64,
    pub height: f64,
    pub grace_ticks: u32,
    pub total_bricks: usize,
    pub bricks_destroyed: usize,
    pub combo: u32,
    pub combo_max: u32,
    pub daily_mode: bool,
    pub leaderboard_rank: Option<(u32, u32)>,
    pub leaderboard_top: Vec<(String, u32)>,
    pub player_best: Option<u32>,
    pub clipboard_msg_ticks: u32,
    rng: StdRng,
}

impl GameState {
    pub fn new(term_width: u16, term_height: u16, daily_mode: bool) -> Self {
        let w = (term_width - 2) as f64;
        let h = (term_height - 2) as f64;
        let rng = if daily_mode {
            StdRng::seed_from_u64(Self::date_seed())
        } else {
            StdRng::from_entropy()
        };
        let mut game = GameState {
            status: GameStatus::Waiting,
            balls: Vec::new(),
            paddle: Paddle {
                x: w / 2.0,
                width: PADDLE_BASE_WIDTH,
            },
            bricks: Vec::new(),
            brick_cols: 0,
            score: 0,
            lives: INITIAL_LIVES,
            level: 1,
            powerups: Vec::new(),
            effects: Vec::new(),
            width: w,
            height: h,
            grace_ticks: 0,
            total_bricks: 0,
            bricks_destroyed: 0,
            combo: 0,
            combo_max: 0,
            daily_mode,
            leaderboard_rank: None,
            leaderboard_top: Vec::new(),
            player_best: None,
            clipboard_msg_ticks: 0,
            rng,
        };
        game.init_level();
        game.spawn_ball();
        game
    }

    pub fn combo_multiplier(&self) -> u32 {
        (1 + self.combo / 5).min(5)
    }

    fn date_seed() -> u64 {
        let now = std::time::SystemTime::now();
        let since_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap();
        // Seed based on day number — same seed for everyone on the same day
        since_epoch.as_secs() / 86400
    }

    pub fn today_date_string() -> String {
        let now = std::time::SystemTime::now();
        let since_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap();
        let days = since_epoch.as_secs() / 86400;
        let z = days + 719468;
        let era = z / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!("{:04}-{:02}-{:02}", y, m, d)
    }

    pub fn resize(&mut self, term_width: u16, term_height: u16) {
        let new_w = (term_width - 2) as f64;
        let new_h = (term_height - 2) as f64;
        if (new_w - self.width).abs() > 0.5 || (new_h - self.height).abs() > 0.5 {
            let ratio_x = new_w / self.width;
            self.paddle.x *= ratio_x;
            for ball in &mut self.balls {
                ball.x = (ball.x * ratio_x).clamp(0.0, new_w - 1.0);
            }
            self.width = new_w;
            self.height = new_h;
            self.init_level();
        }
    }

    fn init_level(&mut self) {
        self.brick_cols = (self.width as usize) / (BRICK_WIDTH + BRICK_GAP);
        if self.brick_cols == 0 {
            self.brick_cols = 1;
        }

        let pattern = self.level_pattern();
        let mut count = 0;

        self.bricks = (0..BRICK_ROWS)
            .map(|row| {
                (0..self.brick_cols)
                    .map(|col| {
                        if pattern(row, col, self.brick_cols) {
                            count += 1;
                            Some(Brick {
                                color_idx: row,
                                points: ((BRICK_ROWS - row) * 10) as u32,
                                hits: self.brick_hits(row),
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect();
        self.total_bricks = count;
        self.bricks_destroyed = 0;
    }

    fn brick_hits(&self, row: usize) -> u8 {
        if self.level <= 3 {
            1
        } else if self.level <= 6 {
            // Top 3 rows get 2 hits
            if row < 3 { 2 } else { 1 }
        } else {
            // Top 3 rows get 3 hits, next 3 get 2 hits
            if row < 3 { 3 } else if row < 6 { 2 } else { 1 }
        }
    }

    fn level_pattern(&self) -> fn(usize, usize, usize) -> bool {
        match (self.level - 1) % 6 {
            0 => |_, _, _| true,                                          // Full
            1 => |row, col, cols| {                                       // Pyramid
                let margin = row / 2;
                col >= margin && col < cols - margin
            },
            2 => |row, col, _| (row + col) % 2 == 0,                     // Checkerboard
            3 => |row, col, cols| {                                       // Diamond
                let mid = cols / 2;
                let half_w = if row <= BRICK_ROWS / 2 { row + 1 } else { BRICK_ROWS - row };
                col >= mid.saturating_sub(half_w) && col < mid + half_w
            },
            4 => |row, _, _| row % 3 != 1,                               // Stripes (gaps)
            5 => |row, col, cols| {                                       // Inverted pyramid
                let margin = (BRICK_ROWS - 1 - row) / 2;
                col >= margin && col < cols - margin
            },
            _ => |_, _, _| true,
        }
    }

    pub fn current_speed(&self) -> f64 {
        let level_bonus = (self.level as f64 - 1.0) * 0.08;
        let progress = if self.total_bricks > 0 {
            self.bricks_destroyed as f64 / self.total_bricks as f64
        } else {
            0.0
        };
        // Quadratic curve: slow ramp early, fast ramp late
        let curve = progress * progress;
        let base = BALL_SPEED_MIN + curve * (BALL_SPEED_MAX - BALL_SPEED_MIN);
        base + level_bonus
    }

    fn spawn_ball(&mut self) {
        let speed = self.current_speed();
        self.balls.push(Ball {
            x: self.width / 2.0,
            y: self.height - 5.0,
            vx: speed * 0.8,
            vy: -speed * 0.5,
        });
    }

    pub fn move_paddle_left(&mut self) {
        if self.status != GameStatus::Playing {
            return;
        }
        let half = self.paddle.width / 2.0;
        self.paddle.x = (self.paddle.x - PADDLE_SPEED).max(half);
    }

    pub fn move_paddle_right(&mut self) {
        if self.status != GameStatus::Playing {
            return;
        }
        let half = self.paddle.width / 2.0;
        self.paddle.x = (self.paddle.x + PADDLE_SPEED).min(self.width - half);
    }

    fn start_grace_period(&mut self) {
        self.grace_ticks = 30; // 1s at 30fps
    }

    pub fn toggle_pause(&mut self) {
        match self.status {
            GameStatus::Playing => self.status = GameStatus::Paused,
            GameStatus::Paused => {
                self.status = GameStatus::Playing;
                self.start_grace_period();
            }
            _ => {}
        }
    }

    pub fn pause(&mut self) {
        if self.status == GameStatus::Playing {
            self.status = GameStatus::Paused;
        }
    }

    pub fn unpause(&mut self) {
        match self.status {
            GameStatus::Paused => {
                self.status = GameStatus::Playing;
                self.start_grace_period();
            }
            GameStatus::Waiting => {
                self.status = GameStatus::Playing;
                self.start_grace_period();
            }
            _ => {}
        }
    }

    pub fn start_or_restart(&mut self) {
        match self.status {
            GameStatus::Waiting => {
                self.status = GameStatus::Playing;
            }
            GameStatus::GameOver => {
                self.score = 0;
                self.combo = 0;
                self.combo_max = 0;
                self.leaderboard_rank = None;
                self.leaderboard_top.clear();
                self.player_best = None;
                self.clipboard_msg_ticks = 0;
                self.lives = INITIAL_LIVES;
                self.level = 1;
                self.effects.clear();
                self.powerups.clear();
                self.paddle.width = PADDLE_BASE_WIDTH;
                self.paddle.x = self.width / 2.0;
                if self.daily_mode {
                    self.rng = StdRng::seed_from_u64(Self::date_seed());
                }
                self.init_level();
                self.balls.clear();
                self.spawn_ball();
                self.status = GameStatus::Playing;
            }
            _ => {}
        }
    }

    pub fn update(&mut self) {
        self.update_effects();
        self.grace_ticks = self.grace_ticks.saturating_sub(1);
        self.clipboard_msg_ticks = self.clipboard_msg_ticks.saturating_sub(1);

        let speed_mult = if self.grace_ticks > 0 {
            GRACE_SPEED / self.current_speed().max(0.1)
        } else if self.effects.iter().any(|e| e.kind == PowerupKind::SlowMo) {
            0.5
        } else {
            1.0
        };

        let has_wide = self.effects.iter().any(|e| e.kind == PowerupKind::WidePaddle);
        self.paddle.width = if has_wide {
            PADDLE_BASE_WIDTH * 1.5
        } else {
            PADDLE_BASE_WIDTH
        };

        self.update_balls(speed_mult);
        self.check_brick_collisions();
        self.update_powerups();

        let remaining = self.bricks.iter().flatten().filter(|b| b.is_some()).count();
        if remaining == 0 {
            self.level += 1;
            self.init_level();
            self.balls.clear();
            self.spawn_ball();
            self.effects.clear();
            self.powerups.clear();
            self.paddle.width = PADDLE_BASE_WIDTH;
        }
    }

    fn update_effects(&mut self) {
        self.effects.retain_mut(|e| {
            e.remaining_ticks = e.remaining_ticks.saturating_sub(1);
            e.remaining_ticks > 0
        });
    }

    fn update_balls(&mut self, speed_mult: f64) {
        let paddle_y = self.height - 3.0;
        let current_speed = self.current_speed();
        let mut lost = Vec::new();

        for (i, ball) in self.balls.iter_mut().enumerate() {
            ball.x += ball.vx * speed_mult;
            ball.y += ball.vy * speed_mult;

            // Left/right walls
            if ball.x <= 0.0 {
                ball.x = 0.0;
                ball.vx = ball.vx.abs();
            } else if ball.x >= self.width - 1.0 {
                ball.x = self.width - 1.0;
                ball.vx = -ball.vx.abs();
            }

            // Top wall
            if ball.y <= 0.0 {
                ball.y = 0.0;
                ball.vy = ball.vy.abs();
            }

            // Paddle collision
            if ball.vy > 0.0 && ball.y >= paddle_y && ball.y < paddle_y + 1.0 {
                let half = self.paddle.width / 2.0;
                let left = self.paddle.x - half;
                let right = self.paddle.x + half;
                if ball.x >= left && ball.x <= right {
                    let hit_pos = ((ball.x - self.paddle.x) / half).clamp(-1.0, 1.0);
                    let speed = current_speed;
                    ball.vx = hit_pos * speed * 1.3;
                    ball.vy = -(speed * 0.5 + (1.0 - hit_pos.abs()) * speed * 0.2);
                    ball.y = paddle_y - 0.1;
                    self.combo = 0;
                }
            }

            // Ball lost
            if ball.y >= self.height - 1.0 {
                lost.push(i);
            }
        }

        for i in lost.into_iter().rev() {
            self.balls.remove(i);
        }

        if self.balls.is_empty() {
            self.lives = self.lives.saturating_sub(1);
            if self.lives == 0 {
                self.status = GameStatus::GameOver;
            } else {
                self.powerups.clear();
                self.effects.clear();
                self.paddle.width = PADDLE_BASE_WIDTH;
                self.spawn_ball();
            }
        }
    }

    pub fn brick_offset_x(&self) -> f64 {
        let grid_width = self.brick_cols * (BRICK_WIDTH + BRICK_GAP) - BRICK_GAP;
        (self.width - grid_width as f64) / 2.0
    }

    fn check_brick_collisions(&mut self) {
        let offset_x = self.brick_offset_x();
        let mut new_powerups = Vec::new();

        for ball in &mut self.balls {
            let mut hit = false;
            for (row_idx, row) in self.bricks.iter_mut().enumerate() {
                if hit {
                    break;
                }
                for (col_idx, brick_slot) in row.iter_mut().enumerate() {
                    if let Some(brick) = brick_slot {
                        let bx = offset_x + col_idx as f64 * (BRICK_WIDTH + BRICK_GAP) as f64;
                        let by = BRICK_START_Y + row_idx as f64;

                        if ball.x >= bx - 0.5
                            && ball.x <= bx + BRICK_WIDTH as f64 + 0.5
                            && ball.y >= by - 0.3
                            && ball.y <= by + 1.3
                        {
                            let center_x = bx + BRICK_WIDTH as f64 / 2.0;
                            let center_y = by + 0.5;
                            let dx = (ball.x - center_x) / (BRICK_WIDTH as f64 / 2.0);
                            let dy = (ball.y - center_y) / 0.5;

                            if dx.abs() > dy.abs() {
                                ball.vx = -ball.vx;
                            } else {
                                ball.vy = -ball.vy;
                            }

                            brick.hits -= 1;
                            if brick.hits == 0 {
                                let points = brick.points;
                                self.combo += 1;
                                if self.combo > self.combo_max {
                                    self.combo_max = self.combo;
                                }
                                let multiplier = (1 + self.combo / 5).min(5);
                                self.score += points * multiplier;
                                self.bricks_destroyed += 1;

                                // Maybe spawn powerup
                                if self.rng.gen::<f64>() < POWERUP_CHANCE {
                                    let kind = match self.rng.gen_range(0..3) {
                                        0 => PowerupKind::WidePaddle,
                                        1 => PowerupKind::MultiBall,
                                        _ => PowerupKind::SlowMo,
                                    };
                                    new_powerups.push(FallingPowerup {
                                        x: bx + BRICK_WIDTH as f64 / 2.0,
                                        y: by + 1.0,
                                        kind,
                                    });
                                }

                                *brick_slot = None;
                            }
                            hit = true;
                            break;
                        }
                    }
                }
            }
        }

        self.powerups.extend(new_powerups);
    }

    fn update_powerups(&mut self) {
        let paddle_y = self.height - 3.0;
        let paddle_left = self.paddle.x - self.paddle.width / 2.0;
        let paddle_right = self.paddle.x + self.paddle.width / 2.0;
        let height = self.height;
        let first_ball = self.balls.first().cloned();
        let ball_count = self.balls.len();

        let mut new_effects = Vec::new();
        let mut new_balls = Vec::new();

        self.powerups.retain_mut(|p| {
            p.y += POWERUP_FALL_SPEED;

            if p.y >= paddle_y && p.y < paddle_y + 1.5
                && p.x >= paddle_left
                && p.x <= paddle_right
            {
                match p.kind {
                    PowerupKind::WidePaddle => {
                        new_effects.push(ActiveEffect {
                            kind: PowerupKind::WidePaddle,
                            remaining_ticks: 300,
                        });
                    }
                    PowerupKind::MultiBall => {
                        if ball_count + new_balls.len() < 12 {
                            if let Some(ref ball) = first_ball {
                                new_balls.push(Ball {
                                    vx: -ball.vx,
                                    ..ball.clone()
                                });
                                new_balls.push(Ball {
                                    vx: ball.vx * 0.3,
                                    vy: -(ball.vx.hypot(ball.vy) * 0.6).min(-0.15),
                                    ..ball.clone()
                                });
                            }
                        }
                    }
                    PowerupKind::SlowMo => {
                        new_effects.push(ActiveEffect {
                            kind: PowerupKind::SlowMo,
                            remaining_ticks: 240,
                        });
                    }
                }
                return false;
            }

            p.y < height
        });

        self.effects.extend(new_effects);
        self.balls.extend(new_balls);
    }
}
