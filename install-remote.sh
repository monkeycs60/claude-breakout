#!/bin/bash
set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${YELLOW}╔══════════════════════════════════╗${NC}"
echo -e "${YELLOW}║     claude-breakout installer    ║${NC}"
echo -e "${YELLOW}╚══════════════════════════════════╝${NC}"
echo ""

# --- Check/install Rust ---
if ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}⚠ Rust not found. Installing via rustup...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}✗ Rust installation failed${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓${NC} Rust installed"
else
    echo -e "${GREEN}✓${NC} Rust found"
fi

# --- Check/install tmux ---
if ! command -v tmux &> /dev/null; then
    echo -e "${YELLOW}⚠ tmux not found. Installing...${NC}"
    if command -v apt-get &> /dev/null; then
        sudo apt-get install -y tmux
    elif command -v brew &> /dev/null; then
        brew install tmux
    elif command -v dnf &> /dev/null; then
        sudo dnf install -y tmux
    elif command -v pacman &> /dev/null; then
        sudo pacman -S --noconfirm tmux
    else
        echo -e "${YELLOW}  Could not auto-install tmux: https://github.com/tmux/tmux/wiki/Installing${NC}"
    fi
    command -v tmux &> /dev/null && echo -e "${GREEN}✓${NC} tmux installed"
else
    echo -e "${GREEN}✓${NC} tmux found"
fi

# --- Clone, build, install ---
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo ""
echo "Downloading claude-breakout..."
git clone --depth 1 https://github.com/monkeycs60/claude-breakout.git "$TMPDIR/claude-breakout" 2>&1 | tail -1
cd "$TMPDIR/claude-breakout"

echo "Building (this may take a minute on first run)..."
cargo build --release 2>&1 | tail -1

INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
cp target/release/claude-breakout "$INSTALL_DIR/"
echo -e "${GREEN}✓${NC} Binary installed"

# --- Create claudebreak launcher ---
cat > "$INSTALL_DIR/claudebreak" << 'LAUNCHER'
#!/bin/bash
AUTOFOCUS=true
POSITION="right"
SIZE=30
CLAUDE_ARGS=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --no-autofocus) AUTOFOCUS=false; shift ;;
        --left)         POSITION="left"; shift ;;
        --bottom)       POSITION="bottom"; shift ;;
        --size)         SIZE="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: claudebreak [OPTIONS] [-- CLAUDE_ARGS...]"
            echo ""
            echo "Options:"
            echo "  --no-autofocus  Don't auto-switch focus to game pane"
            echo "  --left          Game pane on the left (default: right)"
            echo "  --bottom        Game pane on the bottom"
            echo "  --size PERCENT  Game pane size in % (default: 30)"
            echo ""
            echo "Everything after -- is passed to claude:"
            echo "  claudebreak -- --dangerously-skip-permissions"
            echo "  claudebreak -- -p 'fix the tests'"
            echo "  claudebreak --left -- --model sonnet"
            exit 0 ;;
        --)  shift; CLAUDE_ARGS="$*"; break ;;
        *) shift ;;
    esac
done

if ! command -v tmux &> /dev/null; then
    echo "tmux is required. Install: sudo apt install tmux (or) brew install tmux"
    exit 1
fi

tmux kill-session -t claudebreak 2>/dev/null || true
rm -f /tmp/claude-breakout-no-autofocus /tmp/claude-breakout-game-pane /tmp/claude-breakout-claude-pane

if [ "$AUTOFOCUS" = "false" ]; then
    touch /tmp/claude-breakout-no-autofocus
fi

tmux new-session -d -s claudebreak "claude $CLAUDE_ARGS"
CLAUDE_PANE=$(tmux display-message -t claudebreak -p '#{pane_id}')
echo "$CLAUDE_PANE" > /tmp/claude-breakout-claude-pane

case $POSITION in
    right)  GAME_PANE=$(tmux split-window -h -p "$SIZE" -t claudebreak -P -F '#{pane_id}' "claude-breakout") ;;
    left)   GAME_PANE=$(tmux split-window -hb -p "$SIZE" -t claudebreak -P -F '#{pane_id}' "claude-breakout") ;;
    bottom) GAME_PANE=$(tmux split-window -v -p "$SIZE" -t claudebreak -P -F '#{pane_id}' "claude-breakout") ;;
esac
echo "$GAME_PANE" > /tmp/claude-breakout-game-pane

tmux select-pane -t "$CLAUDE_PANE"
tmux attach-session -t claudebreak
LAUNCHER
chmod +x "$INSTALL_DIR/claudebreak"
echo -e "${GREEN}✓${NC} claudebreak launcher installed"

# --- Configure Claude Code hooks ---
python3 << 'PYEOF'
import json, os

settings_file = os.path.expanduser("~/.claude/settings.json")
os.makedirs(os.path.dirname(settings_file), exist_ok=True)

settings = {}
if os.path.exists(settings_file):
    try:
        with open(settings_file) as f:
            settings = json.load(f)
    except (json.JSONDecodeError, IOError):
        settings = {}

hooks = settings.setdefault("hooks", {})

usr1_cmd = 'kill -USR1 $(cat ${XDG_RUNTIME_DIR:-/tmp}/claude-breakout.pid 2>/dev/null) 2>/dev/null; [ ! -f /tmp/claude-breakout-no-autofocus ] && tmux select-pane -t $(cat /tmp/claude-breakout-game-pane 2>/dev/null) 2>/dev/null; true'
usr2_cmd = 'kill -USR2 $(cat ${XDG_RUNTIME_DIR:-/tmp}/claude-breakout.pid 2>/dev/null) 2>/dev/null; [ ! -f /tmp/claude-breakout-no-autofocus ] && tmux select-pane -t $(cat /tmp/claude-breakout-claude-pane 2>/dev/null) 2>/dev/null; true'

new_hooks = {
    "UserPromptSubmit": {"hooks": [{"type": "command", "command": usr1_cmd, "async": True}]},
    "Stop": {"hooks": [{"type": "command", "command": usr2_cmd, "async": True}]},
    "PermissionRequest": {"hooks": [{"type": "command", "command": usr2_cmd, "async": True}]},
    "Notification": {"hooks": [{"type": "command", "command": usr2_cmd, "async": True}]},
}

for event_name, hook_entry in new_hooks.items():
    event_hooks = hooks.setdefault(event_name, [])
    already = any(
        any("claude-breakout" in h.get("command", "") for h in e.get("hooks", []))
        for e in event_hooks
    )
    if not already:
        event_hooks.append(hook_entry)

with open(settings_file, "w") as f:
    json.dump(settings, f, indent=2)
PYEOF
echo -e "${GREEN}✓${NC} Claude Code hooks configured"

# --- Check PATH ---
echo ""
if echo "$PATH" | grep -q "$HOME/.local/bin"; then
    echo -e "${GREEN}✓${NC} ~/.local/bin is in your PATH"
else
    # Add to PATH permanently
    SHELL_RC="$HOME/.bashrc"
    [ -n "$ZSH_VERSION" ] && SHELL_RC="$HOME/.zshrc"
    [ -f "$HOME/.zshrc" ] && SHELL_RC="$HOME/.zshrc"
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$SHELL_RC"
    export PATH="$HOME/.local/bin:$PATH"
    echo -e "${GREEN}✓${NC} Added ~/.local/bin to PATH (restart terminal or: source $SHELL_RC)"
fi

# --- Done ---
echo ""
echo -e "${GREEN}══════════════════════════════════════${NC}"
echo -e "${GREEN} claude-breakout installed!${NC}"
echo -e "${GREEN}══════════════════════════════════════${NC}"
echo ""
echo "  claudebreak              Launch Claude Code + Breakout"
echo "  claude-breakout          Just the game"
echo "  claude-breakout --daily  Daily challenge (same game for everyone)"
echo "  claude-breakout --scores Show leaderboard"
echo ""
echo "  ← →  Move  |  Space  Pause  |  Enter  Start  |  S  Share  |  Q  Quit"
echo ""
