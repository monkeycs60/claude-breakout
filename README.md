# claude-breakout 🧱

A terminal Breakout game that you play **while waiting for Claude Code** to finish processing.

The game auto-pauses when Claude responds and auto-resumes when you submit a new prompt. No more staring at a spinner.

<!-- TODO: Add GIF here -->
<!-- ![demo](./demo.gif) -->

## How it works

```
┌─────────────────────────────────┬──────────────────┐
│                                 │   ██████ ██████  │
│     Claude Code                 │   ██████ ██████  │
│                                 │   ██████ ██████  │
│  ⏳ Thinking...                 │       ●          │
│                                 │     ━━━━━━━      │
│                                 │  Score: 420  ♥♥♥ │
└─────────────────────────────────┴──────────────────┘
```

Uses Claude Code's [hook system](https://docs.anthropic.com/en/docs/claude-code/hooks) to detect when Claude starts/stops processing:

- **`UserPromptSubmit`** hook → sends `SIGUSR1` → game **unpauses**
- **`Stop`** hook → sends `SIGUSR2` → game **pauses**

The game runs in a separate tmux pane. Zero interference with your workflow.

## Install

```bash
git clone https://github.com/YOUR_USERNAME/claude-breakout.git
cd claude-breakout
./install.sh
```

The install script:
1. Builds the binary (requires [Rust](https://rustup.rs))
2. Installs to `~/.local/bin/`
3. Configures Claude Code hooks automatically
4. Creates the `claudebreak` launcher

## Usage

```bash
# Launch Claude Code + Breakout side by side
claudebreak

# Options
claudebreak --no-autofocus    # Don't auto-switch focus to game
claudebreak --bottom          # Game pane on the bottom
claudebreak --left            # Game pane on the left
claudebreak --size 40         # Game pane takes 40% of terminal

# Or just the game standalone
claude-breakout
```

## Controls

| Key | Action |
|-----|--------|
| `← →` | Move paddle |
| `Space` | Pause / Resume |
| `Enter` | Start / Restart |
| `Q` | Quit |

## Features

- **Auto pause/resume** via Claude Code hooks
- **Powerups** (rare ~5% drop rate):
  - `W` Wide paddle (10s)
  - `M` Multi-ball (3 balls!)
  - `S` Slow-mo (8s)
- **Progressive difficulty** — ball speeds up each level
- **Responsive** — adapts to terminal/pane size
- **Lightweight** — single binary, ~30 FPS, minimal CPU

## Requirements

- [Rust](https://rustup.rs) (to build)
- [tmux](https://github.com/tmux/tmux) (for side-by-side mode)
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) (for auto pause/resume)

## Uninstall

```bash
rm ~/.local/bin/claude-breakout ~/.local/bin/claudebreak
# Remove hooks from ~/.claude/settings.json (the "UserPromptSubmit" and "Stop" entries containing "claude-breakout")
```

## License

MIT
