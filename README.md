# claude-breakout 🧱

A terminal Breakout game that you play **while waiting for Claude Code** to finish processing.

The game auto-pauses when Claude responds and auto-resumes when you submit a new prompt. No more staring at a spinner.

![demo](./demo.gif)

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

Zero interference with your workflow.

## Install

One command. Installs everything (Rust, tmux, binary, hooks):

```bash
curl -fsSL https://raw.githubusercontent.com/monkeycs60/claude-breakout/master/install-remote.sh | bash
```

That's it. The script handles:
1. Installing Rust if missing (via [rustup](https://rustup.rs))
2. Installing tmux if missing (via your package manager)
3. Building the game binary
4. Configuring Claude Code hooks
5. Creating the `claudebreak` launcher
6. Adding `~/.local/bin` to your PATH

## Usage

### With tmux (recommended)

```bash
claudebreak                      # Side by side: Claude Code + Breakout

# Options
claudebreak --no-autofocus       # Don't auto-switch focus between panes
claudebreak --bottom             # Game pane on the bottom
claudebreak --left               # Game pane on the left
claudebreak --size 40            # Game pane takes 40% of terminal
```

Focus auto-switches to the game when you submit a prompt, and back to Claude Code when it finishes.

### Without tmux

Just open two terminals:
1. Run `claude` (Claude Code) in one
2. Run `claude-breakout` in the other

The hooks still work — the game will auto-pause/resume via Unix signals regardless of your terminal setup. You just won't get auto-focus switching.

### Standalone (no Claude Code)

```bash
claude-breakout                   # Just play the game!
```

Press `Enter` to start, `Space` to pause. Works without any hooks.

## Controls

| Key | Action |
|-----|--------|
| `← →` | Move paddle |
| `Space` | Pause / Resume |
| `Enter` | Start / Restart |
| `Q` | Quit |

## Features

- **Auto pause/resume** via Claude Code hooks
- **Auto-focus** — tmux switches between game and Claude panes automatically
- **Progressive difficulty** — ball starts slow, accelerates as you destroy bricks
- **Powerups** (rare ~5% drop rate):
  - `W` Wide paddle (10s)
  - `M` Multi-ball (3 balls!)
  - `S` Slow-mo (8s)
- **Grace period** — ball slows down for 1s after resuming so you don't get blindsided
- **Responsive** — adapts to terminal/pane size
- **Lightweight** — single 611KB binary, ~30 FPS, minimal CPU

## Requirements

- [Rust](https://rustup.rs) (to build)
- [tmux](https://github.com/tmux/tmux) (optional — for side-by-side mode + auto-focus)
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) (optional — for auto pause/resume)

## Uninstall

```bash
rm ~/.local/bin/claude-breakout ~/.local/bin/claudebreak
# Remove hooks from ~/.claude/settings.json
# (delete the "UserPromptSubmit" and "Stop" entries containing "claude-breakout")
```

## License

MIT
