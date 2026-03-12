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

# --- Check dependencies ---
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}✗ Rust/Cargo not found${NC}"
    echo "  Install from https://rustup.rs"
    exit 1
fi
echo -e "${GREEN}✓${NC} Rust/Cargo found"

if ! command -v tmux &> /dev/null; then
    echo -e "${YELLOW}⚠ tmux not found (needed for side-by-side mode)${NC}"
    echo ""
    # Detect package manager and install
    if command -v apt-get &> /dev/null; then
        echo "  Installing tmux via apt..."
        sudo apt-get install -y tmux && echo -e "${GREEN}✓${NC} tmux installed"
    elif command -v brew &> /dev/null; then
        echo "  Installing tmux via Homebrew..."
        brew install tmux && echo -e "${GREEN}✓${NC} tmux installed"
    elif command -v dnf &> /dev/null; then
        echo "  Installing tmux via dnf..."
        sudo dnf install -y tmux && echo -e "${GREEN}✓${NC} tmux installed"
    elif command -v pacman &> /dev/null; then
        echo "  Installing tmux via pacman..."
        sudo pacman -S --noconfirm tmux && echo -e "${GREEN}✓${NC} tmux installed"
    else
        echo -e "${YELLOW}  Could not auto-install tmux. Install it manually:${NC}"
        echo "  https://github.com/tmux/tmux/wiki/Installing"
    fi
else
    echo -e "${GREEN}✓${NC} tmux found"
fi

# --- Build ---
echo ""
echo "Building claude-breakout (release)..."
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"
cargo build --release 2>&1

BINARY="$SCRIPT_DIR/target/release/claude-breakout"
if [ ! -f "$BINARY" ]; then
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi
echo -e "${GREEN}✓${NC} Build successful"

# --- Install binary ---
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
cp "$BINARY" "$INSTALL_DIR/claude-breakout"
chmod +x "$INSTALL_DIR/claude-breakout"
echo -e "${GREEN}✓${NC} Binary installed to $INSTALL_DIR/claude-breakout"

# --- Create claudebreak launcher ---
cat > "$INSTALL_DIR/claudebreak" << 'LAUNCHER'
#!/bin/bash
# claudebreak — Launch Claude Code with breakout side pane
# Usage: claudebreak [--no-autofocus] [--left] [--bottom] [--size PERCENT]

AUTOFOCUS=true
POSITION="right"
SIZE=30

while [[ $# -gt 0 ]]; do
    case $1 in
        --no-autofocus) AUTOFOCUS=false; shift ;;
        --left)         POSITION="left"; shift ;;
        --bottom)       POSITION="bottom"; shift ;;
        --size)         SIZE="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: claudebreak [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --no-autofocus  Don't auto-switch focus to game pane"
            echo "  --left          Game pane on the left (default: right)"
            echo "  --bottom        Game pane on the bottom"
            echo "  --size PERCENT  Game pane size in % (default: 30)"
            exit 0 ;;
        *) shift ;;
    esac
done

if ! command -v tmux &> /dev/null; then
    echo "tmux is required. Install: sudo apt install tmux (or) brew install tmux"
    exit 1
fi

# Cleanup
tmux kill-session -t claudebreak 2>/dev/null || true
rm -f /tmp/claude-breakout-no-autofocus /tmp/claude-breakout-game-pane /tmp/claude-breakout-claude-pane

# Autofocus config
if [ "$AUTOFOCUS" = "false" ]; then
    touch /tmp/claude-breakout-no-autofocus
fi

# Create session with Claude Code
tmux new-session -d -s claudebreak "claude"

# Save Claude pane ID
CLAUDE_PANE=$(tmux display-message -t claudebreak -p '#{pane_id}')
echo "$CLAUDE_PANE" > /tmp/claude-breakout-claude-pane

# Add game pane and capture its ID
case $POSITION in
    right)  GAME_PANE=$(tmux split-window -h -p "$SIZE" -t claudebreak -P -F '#{pane_id}' "claude-breakout") ;;
    left)   GAME_PANE=$(tmux split-window -hb -p "$SIZE" -t claudebreak -P -F '#{pane_id}' "claude-breakout") ;;
    bottom) GAME_PANE=$(tmux split-window -v -p "$SIZE" -t claudebreak -P -F '#{pane_id}' "claude-breakout") ;;
esac
echo "$GAME_PANE" > /tmp/claude-breakout-game-pane

# Focus on Claude Code pane
tmux select-pane -t "$CLAUDE_PANE"

tmux attach-session -t claudebreak
LAUNCHER
chmod +x "$INSTALL_DIR/claudebreak"
echo -e "${GREEN}✓${NC} claudebreak launcher installed"

# --- Configure Claude Code hooks ---
echo ""
echo "Configuring Claude Code hooks..."

SETTINGS_DIR="$HOME/.claude"
SETTINGS_FILE="$SETTINGS_DIR/settings.json"
mkdir -p "$SETTINGS_DIR"

# Python script to safely merge hooks into existing settings
python3 << 'PYEOF'
import json
import os

settings_file = os.path.expanduser("~/.claude/settings.json")

# Load existing settings or start fresh
settings = {}
if os.path.exists(settings_file):
    try:
        with open(settings_file) as f:
            settings = json.load(f)
    except (json.JSONDecodeError, IOError):
        settings = {}

hooks = settings.setdefault("hooks", {})

# Signal + tmux autofocus commands
usr1_cmd = 'kill -USR1 $(cat /tmp/claude-breakout.pid 2>/dev/null) 2>/dev/null; [ ! -f /tmp/claude-breakout-no-autofocus ] && tmux select-pane -t $(cat /tmp/claude-breakout-game-pane 2>/dev/null) 2>/dev/null; true'
usr2_cmd = 'kill -USR2 $(cat /tmp/claude-breakout.pid 2>/dev/null) 2>/dev/null; [ ! -f /tmp/claude-breakout-no-autofocus ] && tmux select-pane -t $(cat /tmp/claude-breakout-claude-pane 2>/dev/null) 2>/dev/null; true'

# Hook entries to add
new_hooks = {
    "UserPromptSubmit": {
        "hooks": [{"type": "command", "command": usr1_cmd, "async": True}]
    },
    "Stop": {
        "hooks": [{"type": "command", "command": usr2_cmd, "async": True}]
    },
    "PermissionRequest": {
        "hooks": [{"type": "command", "command": usr2_cmd, "async": True}]
    },
    "Notification": {
        "hooks": [{"type": "command", "command": usr2_cmd, "async": True}]
    },
}

# Merge: append to existing hook arrays without duplicating
for event_name, hook_entry in new_hooks.items():
    event_hooks = hooks.setdefault(event_name, [])

    # Check if our hook is already there
    already_exists = any(
        any("claude-breakout" in h.get("command", "") for h in entry.get("hooks", []))
        for entry in event_hooks
    )

    if not already_exists:
        event_hooks.append(hook_entry)

with open(settings_file, "w") as f:
    json.dump(settings, f, indent=2)

print("Hooks configured successfully")
PYEOF

echo -e "${GREEN}✓${NC} Claude Code hooks configured"

# --- Check PATH ---
echo ""
if echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo -e "${GREEN}✓${NC} $INSTALL_DIR is in your PATH"
else
    echo -e "${YELLOW}⚠${NC} Add $INSTALL_DIR to your PATH:"
    echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
    echo "  source ~/.bashrc"
fi

# --- Done ---
echo ""
echo -e "${GREEN}══════════════════════════════════════${NC}"
echo -e "${GREEN} Installation complete!${NC}"
echo -e "${GREEN}══════════════════════════════════════${NC}"
echo ""
echo "Usage:"
echo "  claudebreak          Launch Claude Code + Breakout side by side"
echo "  claude-breakout      Launch just the game standalone"
echo ""
echo "Controls:"
echo "  ← →     Move paddle"
echo "  SPACE    Pause/Resume"
echo "  ENTER    Start game / Restart after game over"
echo "  Q        Quit"
echo ""
echo "The game auto-pauses when Claude finishes and"
echo "auto-resumes when you submit a new prompt!"
