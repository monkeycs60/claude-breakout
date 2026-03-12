mod game;
mod ui;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{fs, process};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use signal_hook::consts::{SIGUSR1, SIGUSR2};

use game::GameState;

const TICK_RATE: Duration = Duration::from_millis(33); // ~30 FPS
const PID_FILE: &str = "/tmp/claude-breakout.pid";

fn main() -> io::Result<()> {
    // Write PID file so hooks can signal us
    fs::write(PID_FILE, process::id().to_string())?;

    // Register Unix signal handlers
    let sigusr1 = Arc::new(AtomicBool::new(false));
    let sigusr2 = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGUSR1, Arc::clone(&sigusr1))
        .expect("Failed to register SIGUSR1");
    signal_hook::flag::register(SIGUSR2, Arc::clone(&sigusr2))
        .expect("Failed to register SIGUSR2");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let size = terminal.size()?;
    let mut game = GameState::new(size.width, size.height);

    let result = run_loop(&mut terminal, &mut game, &sigusr1, &sigusr2);

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    let _ = fs::remove_file(PID_FILE);

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    game: &mut GameState,
    sigusr1: &AtomicBool,
    sigusr2: &AtomicBool,
) -> io::Result<()> {
    let mut last_tick = Instant::now();

    loop {
        // Check signals from Claude Code hooks
        if sigusr1.swap(false, Ordering::Relaxed) {
            game.unpause();
        }
        if sigusr2.swap(false, Ordering::Relaxed) {
            game.pause();
        }

        // Poll for input events
        let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                    KeyCode::Left => game.move_paddle_left(),
                    KeyCode::Right => game.move_paddle_right(),
                    KeyCode::Char(' ') => game.toggle_pause(),
                    KeyCode::Enter => game.start_or_restart(),
                    _ => {}
                },
                Event::Resize(w, h) => {
                    game.resize(w, h);
                }
                _ => {}
            }
        }

        // Update game state on tick
        if last_tick.elapsed() >= TICK_RATE {
            if game.status == game::GameStatus::Playing {
                let size = terminal.size()?;
                game.resize(size.width, size.height);
                game.update();
            }
            last_tick = Instant::now();
        }

        // Render
        terminal.draw(|frame| ui::render(frame, game))?;
    }
}
