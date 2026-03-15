mod game;
mod leaderboard;
mod ui;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
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

use game::{GameState, GameStatus};

const TICK_RATE: Duration = Duration::from_millis(33); // ~30 FPS
fn pid_file_path() -> String {
    std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| "/tmp".to_string())
        + "/claude-breakout.pid"
}
const VERSION: &str = env!("CARGO_PKG_VERSION");

struct Args {
    daily: bool,
    scores: bool,
    name: Option<String>,
    version: bool,
    api_url: Option<String>,
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut result = Args {
        daily: false,
        scores: false,
        name: None,
        version: false,
        api_url: None,
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--daily" | "-d" => result.daily = true,
            "--scores" | "-s" => result.scores = true,
            "--version" | "-V" => result.version = true,
            "--name" | "-n" => {
                i += 1;
                if i < args.len() {
                    result.name = Some(args[i].clone());
                }
            }
            "--api-url" => {
                i += 1;
                if i < args.len() {
                    result.api_url = Some(args[i].clone());
                }
            }
            "--help" | "-h" => {
                println!("claude-breakout v{}", VERSION);
                println!();
                println!("Usage: claude-breakout [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -d, --daily       Daily challenge (same seed for everyone today)");
                println!("  -s, --scores      Show leaderboard and exit");
                println!("  -n, --name NAME   Set player name (saved to config)");
                println!("  --api-url URL     Override leaderboard API URL");
                println!("  -V, --version     Show version");
                println!("  -h, --help        Show this help");
                process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }
    result
}

fn main() -> io::Result<()> {
    let args = parse_args();

    if args.version {
        println!("claude-breakout v{}", VERSION);
        return Ok(());
    }

    // Set API URL override if provided
    if let Some(url) = &args.api_url {
        std::env::set_var("BREAKOUT_API_URL", url);
    }

    // Set player name if provided
    if let Some(name) = &args.name {
        leaderboard::set_player_name(name);
    }

    // Show leaderboard mode
    if args.scores {
        let date = GameState::today_date_string();
        leaderboard::print_leaderboard("daily", Some(&date));
        leaderboard::print_leaderboard("freeplay", None);
        return Ok(());
    }

    // Install panic handler to restore terminal
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        default_hook(info);
    }));

    // Write PID file so hooks can signal us
    let pid_file = pid_file_path();
    fs::write(&pid_file, process::id().to_string())?;

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
    let mut game = GameState::new(size.width, size.height, args.daily);

    let result = run_loop(&mut terminal, &mut game, &sigusr1, &sigusr2);

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    let _ = fs::remove_file(&pid_file);

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    game: &mut GameState,
    sigusr1: &AtomicBool,
    sigusr2: &AtomicBool,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    let mut score_rx: Option<mpsc::Receiver<(u32, u32)>> = None;
    let mut was_game_over = false;
    let player_name = leaderboard::get_player_name();

    loop {
        // Check signals from Claude Code hooks
        if sigusr1.swap(false, Ordering::Relaxed) {
            game.unpause();
        }
        if sigusr2.swap(false, Ordering::Relaxed) {
            game.pause();
        }

        // Check for async score submission result
        if let Some(ref rx) = score_rx {
            if let Ok((rank, total)) = rx.try_recv() {
                game.leaderboard_rank = Some((rank, total));
            }
        }

        // Submit score when game just ended
        if game.status == GameStatus::GameOver && !was_game_over {
            was_game_over = true;
            let entry = leaderboard::ScoreEntry {
                player: player_name.clone(),
                score: game.score,
                level: game.level,
                combo_max: game.combo_max,
                mode: if game.daily_mode {
                    "daily".to_string()
                } else {
                    "freeplay".to_string()
                },
                date: GameState::today_date_string(),
            };
            let (tx, rx) = mpsc::channel();
            score_rx = Some(rx);
            std::thread::spawn(move || {
                if let Ok(resp) = leaderboard::submit_score(&entry) {
                    let _ = tx.send((resp.rank, resp.total));
                }
            });
        }
        if game.status != GameStatus::GameOver {
            was_game_over = false;
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
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        if game.status == GameStatus::GameOver {
                            let text = leaderboard::share_text(
                                game.score,
                                game.level,
                                game.combo_max,
                                if game.daily_mode { "daily" } else { "freeplay" },
                                &GameState::today_date_string(),
                            );
                            leaderboard::copy_to_clipboard(&text);
                            game.clipboard_msg_ticks = 60; // 2 seconds at 30fps
                        }
                    }
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
