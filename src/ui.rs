use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
    Frame,
};

use crate::game::{GameState, GameStatus, PowerupKind};

const BRICK_WIDTH: usize = 6;
const BRICK_GAP: usize = 1;
const BRICK_START_Y: u16 = 1;

const BRICK_COLORS: [Color; 10] = [
    Color::Red,
    Color::Red,
    Color::LightRed,
    Color::Magenta,
    Color::Yellow,
    Color::Yellow,
    Color::Green,
    Color::Green,
    Color::Cyan,
    Color::Blue,
];

pub fn render(frame: &mut Frame, game: &GameState) {
    let area = frame.area();

    if area.width < 20 || area.height < 10 {
        let msg = Paragraph::new("Too small!\n20x10 min")
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    // Draw border + game elements as a single widget
    frame.render_widget(GameView { game }, area);

    // Draw overlays on top
    match game.status {
        GameStatus::Waiting => {
            render_popup(
                frame,
                area,
                "CLAUDE-BREAKOUT",
                &[
                    "",
                    "Waiting for Claude...",
                    "",
                    "Press ENTER to play",
                    "Press Q to quit",
                ],
            );
        }
        GameStatus::Paused => {
            render_popup(
                frame,
                area,
                "PAUSED",
                &[
                    "",
                    "Claude has finished!",
                    "",
                    "SPACE to continue",
                    "Q to quit",
                ],
            );
        }
        GameStatus::GameOver => {
            let score = format!("Score: {}", game.score);
            let level = format!("Level: {}", game.level);
            render_popup(
                frame,
                area,
                "GAME OVER",
                &["", &score, &level, "", "ENTER to play again", "Q to quit"],
            );
        }
        GameStatus::Playing => {}
    }
}

struct GameView<'a> {
    game: &'a GameState,
}

impl<'a> Widget for GameView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = format!(" claude-breakout | Lvl {} ", self.game.level);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, buf);

        let game = self.game;

        render_bricks(buf, game, inner);

        if game.status == GameStatus::Playing || game.status == GameStatus::Paused {
            render_balls(buf, game, inner);
            render_paddle(buf, game, inner);
            render_falling_powerups(buf, game, inner);
        }

        render_status_bar(buf, game, area);
        render_effects_bar(buf, game, area);
    }
}

fn render_bricks(buf: &mut Buffer, game: &GameState, area: Rect) {
    let offset_x = game.brick_offset_x();

    for (row_idx, row) in game.bricks.iter().enumerate() {
        let y = area.y + BRICK_START_Y + row_idx as u16;
        if y >= area.y + area.height {
            break;
        }

        for (col_idx, brick) in row.iter().enumerate() {
            if let Some(b) = brick {
                let x_start = offset_x + col_idx as f64 * (BRICK_WIDTH + BRICK_GAP) as f64;
                let color = BRICK_COLORS[b.color_idx % BRICK_COLORS.len()];
                let brick_str: String = "█".repeat(BRICK_WIDTH);

                let sx = area.x + x_start as u16;
                if sx + BRICK_WIDTH as u16 <= area.x + area.width && y < area.y + area.height {
                    buf.set_string(sx, y, &brick_str, Style::default().fg(color));
                }
            }
        }
    }
}

fn render_balls(buf: &mut Buffer, game: &GameState, area: Rect) {
    let style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    for ball in &game.balls {
        let x = area.x + ball.x.round() as u16;
        let y = area.y + ball.y.round() as u16;
        if x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height {
            buf.set_string(x, y, "●", style);
        }
    }
}

fn render_paddle(buf: &mut Buffer, game: &GameState, area: Rect) {
    let paddle_y = area.y + area.height - 2;
    let paddle_left = (game.paddle.x - game.paddle.width / 2.0).round() as u16 + area.x;
    let paddle_width = game.paddle.width.round() as u16;

    if paddle_y < area.y + area.height {
        let paddle_str = "━".repeat(paddle_width as usize);
        let style = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);
        let clamped_x = paddle_left.max(area.x);
        buf.set_string(clamped_x, paddle_y, &paddle_str, style);
    }
}

fn render_falling_powerups(buf: &mut Buffer, game: &GameState, area: Rect) {
    for p in &game.powerups {
        let x = area.x + p.x.round() as u16;
        let y = area.y + p.y.round() as u16;
        if x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height {
            let (ch, color) = match p.kind {
                PowerupKind::WidePaddle => ("◆W", Color::Magenta),
                PowerupKind::MultiBall => ("◆M", Color::Cyan),
                PowerupKind::SlowMo => ("◆S", Color::Yellow),
            };
            buf.set_string(
                x,
                y,
                ch,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            );
        }
    }
}

fn render_status_bar(buf: &mut Buffer, game: &GameState, area: Rect) {
    let y = area.y + area.height - 1;
    let hearts: String = (0..game.lives).map(|_| '♥').collect();
    let empty: String = (0..(INITIAL_LIVES.saturating_sub(game.lives)))
        .map(|_| '♡')
        .collect();
    let status = format!(" Score: {:>5}  {}{}  ", game.score, hearts, empty);
    buf.set_string(
        area.x + 1,
        y,
        &status,
        Style::default().fg(Color::White),
    );
}

const INITIAL_LIVES: u8 = 3;

fn render_effects_bar(buf: &mut Buffer, game: &GameState, area: Rect) {
    if game.effects.is_empty() {
        return;
    }

    let y = area.y + area.height - 1;
    let mut parts: Vec<String> = Vec::new();

    for effect in &game.effects {
        let (label, _color) = match effect.kind {
            PowerupKind::WidePaddle => ("W", Color::Magenta),
            PowerupKind::MultiBall => ("M", Color::Cyan),
            PowerupKind::SlowMo => ("S", Color::Yellow),
        };
        let secs = effect.remaining_ticks / 30;
        parts.push(format!("{}:{}s", label, secs));
    }

    let text = parts.join(" ");
    let x = area.x + area.width - 1 - text.len() as u16;
    buf.set_string(x, y, &text, Style::default().fg(Color::Yellow));
}

fn render_popup(frame: &mut Frame, area: Rect, title: &str, lines: &[&str]) {
    let max_line = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let content_width = max_line.max(title.len()) + 4;
    let width = (content_width as u16 + 2).min(area.width);
    let height = (lines.len() as u16 + 4).min(area.height);

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let mut text_lines: Vec<Line> = Vec::new();
    text_lines.push(Line::from(Span::styled(
        title,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    for line in lines {
        text_lines.push(Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::White),
        )));
    }

    let popup = Paragraph::new(text_lines)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );

    frame.render_widget(popup, popup_area);
}
