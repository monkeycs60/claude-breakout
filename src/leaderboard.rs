use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

const DEFAULT_API_URL: &str = "https://breakout-api.workers.dev";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScoreEntry {
    pub player: String,
    pub score: u32,
    pub level: u32,
    pub combo_max: u32,
    pub mode: String,
    pub date: String,
}

#[derive(Deserialize, Debug)]
pub struct SubmitResponse {
    pub rank: u32,
    pub total: u32,
}

#[derive(Deserialize, Debug)]
pub struct LeaderboardResponse {
    pub scores: Vec<ScoreEntry>,
}

fn api_url() -> String {
    if let Ok(url) = std::env::var("BREAKOUT_API_URL") {
        return url;
    }
    let config_file = data_dir().join("config.json");
    if let Ok(data) = std::fs::read_to_string(&config_file) {
        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(url) = config.get("api_url").and_then(|v| v.as_str()) {
                return url.to_string();
            }
        }
    }
    DEFAULT_API_URL.to_string()
}

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".claude-breakout")
}

pub fn get_player_name() -> String {
    let config_file = data_dir().join("config.json");
    if let Ok(data) = std::fs::read_to_string(&config_file) {
        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(name) = config.get("player").and_then(|v| v.as_str()) {
                return name.to_string();
            }
        }
    }
    std::env::var("USER").unwrap_or_else(|_| "anon".to_string())
}

pub fn set_player_name(name: &str) {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).ok();
    let config_file = dir.join("config.json");

    // Read existing config or start fresh
    let mut config: serde_json::Value = std::fs::read_to_string(&config_file)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    config["player"] = serde_json::json!(name);
    std::fs::write(&config_file, serde_json::to_string_pretty(&config).unwrap()).ok();
}

pub fn submit_score(entry: &ScoreEntry) -> Result<SubmitResponse, String> {
    let url = format!("{}/api/scores", api_url());
    ureq::post(&url)
        .send_json(serde_json::to_value(entry).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?
        .into_json::<SubmitResponse>()
        .map_err(|e| e.to_string())
}

pub fn fetch_leaderboard(mode: &str, date: Option<&str>) -> Result<Vec<ScoreEntry>, String> {
    let mut url = format!("{}/api/leaderboard/{}", api_url(), mode);
    if let Some(d) = date {
        url.push_str(&format!("?date={}", d));
    }
    ureq::get(&url)
        .call()
        .map_err(|e| e.to_string())?
        .into_json::<LeaderboardResponse>()
        .map_err(|e| e.to_string())
        .map(|r| r.scores)
}

pub fn share_text(score: u32, level: u32, combo_max: u32, mode: &str, date: &str) -> String {
    let mode_text = if mode == "daily" {
        format!("Daily {}", date)
    } else {
        "Free Play".to_string()
    };
    format!(
        "\u{1f3d3} claude-breakout | {} | Score: {} | Lvl {} | Combo x{}\nCan you beat me? \u{2192} github.com/monkeycs60/claude-breakout",
        mode_text, score, level, combo_max
    )
}

pub fn copy_to_clipboard(text: &str) -> bool {
    // Try xclip (Linux)
    if let Ok(mut child) = std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        return child.wait().map(|s| s.success()).unwrap_or(false);
    }
    // Try xsel (Linux)
    if let Ok(mut child) = std::process::Command::new("xsel")
        .args(["--clipboard", "--input"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        return child.wait().map(|s| s.success()).unwrap_or(false);
    }
    // Try pbcopy (macOS)
    if let Ok(mut child) = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        return child.wait().map(|s| s.success()).unwrap_or(false);
    }
    false
}

pub fn print_leaderboard(mode: &str, date: Option<&str>) {
    let label = if mode == "daily" {
        format!("Daily {}", date.unwrap_or("today"))
    } else {
        "All-Time Free Play".to_string()
    };
    println!("\n  \u{1f3d3} claude-breakout — {} Leaderboard\n", label);

    match fetch_leaderboard(mode, date) {
        Ok(scores) if scores.is_empty() => {
            println!("  No scores yet. Be the first!");
        }
        Ok(scores) => {
            println!(
                "  {:<4} {:<16} {:>7} {:>5} {:>6}",
                "Rank", "Player", "Score", "Lvl", "Combo"
            );
            println!("  {}", "-".repeat(42));
            for (i, s) in scores.iter().enumerate() {
                println!(
                    "  {:<4} {:<16} {:>7} {:>5} {:>5}x",
                    format!("#{}", i + 1),
                    s.player,
                    s.score,
                    s.level,
                    s.combo_max,
                );
            }
        }
        Err(e) => {
            eprintln!("  Could not fetch leaderboard: {}", e);
            eprintln!("  (Is the API deployed? Set BREAKOUT_API_URL env var)");
        }
    }
    println!();
}
