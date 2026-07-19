#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# AI-OS Platform — Development Environment Setup
# Target: Arch Linux
# Usage:  ./scripts/setup.sh
# Safety: Idempotent — safe to run multiple times
# ============================================================

RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${CYAN}[SETUP]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC} $*"; }
err()   { echo -e "${RED}[ERR]${NC} $*" >&2; }

# --- Ensure we are on Arch Linux ---
[[ -f /etc/arch-release ]] || { err "This script is for Arch Linux only."; exit 1; }

# --- 1. Initialize pacman keyring (fresh installs) ---
if [[ ! -d /etc/pacman.d/gnupg ]]; then
    info "Initializing pacman keyring..."
    sudo pacman-key --init
    sudo pacman-key --populate archlinux
fi

# --- 2. Full system update ---
info "Updating system..."
sudo pacman -Syu --noconfirm

# --- 3. Install packages ---
PACKAGES=(
    # Essential system
    base-devel git openssh

    # Language runtimes & compilers
    gcc clang cmake make rust go python python-pip nodejs npm

    # Developer utilities
    curl wget unzip zip tmux tree htop jq ripgrep fd vim nano
)

info "Installing packages..."
sudo pacman -S --noconfirm --needed "${PACKAGES[@]}"

# --- 4. Configure Git ---
if [[ ! -f "$HOME/.gitconfig" ]]; then
    info "Configuring Git..."
    cat > "$HOME/.gitconfig" << 'GITEOF'
[user]
	name = Arch AI-OS Developer
	email = dev@ai-os.local

[core]
	editor = vim
	autocrlf = input
	safecrlf = warn
	whitespace = trailing-space,space-before-tab

[init]
	defaultBranch = main

[push]
	default = simple
	autoSetupRemote = true

[fetch]
	prune = true

[status]
	short = true

[diff]
	renames = copies
	submodule = log

[merge]
	conflictstyle = diff3

[alias]
	st = status -sb
	co = checkout
	br = branch
	ci = commit
	cia = commit --amend
	amendre = commit --amend --reuse-message=HEAD
	lg = log --oneline --graph --decorate --all
	lga = log --oneline --graph --decorate --all --branches --tags --remotes
	last = log -1 HEAD
	unstage = reset HEAD --
	discard = checkout --
	diffc = diff --cached
	dc = diff --cached
	cleanup = clean -fd
	undo = reset --soft HEAD~1
	squash = merge --squash
	who = shortlog -sn --
	root = rev-parse --show-toplevel
GITEOF
fi

# --- 5. Configure SSH ---
mkdir -p "$HOME/.ssh"
chmod 700 "$HOME/.ssh"
if [[ ! -f "$HOME/.ssh/config" ]]; then
    info "Configuring SSH..."
    cat > "$HOME/.ssh/config" << 'SSHEOF'
Host *
	ServerAliveInterval 60
	ServerAliveCountMax 3
	TCPKeepAlive yes
	IdentitiesOnly yes
	PasswordAuthentication no
	ChallengeResponseAuthentication no
	HashKnownHosts yes
SSHEOF
    chmod 600 "$HOME/.ssh/config"
fi

# --- 6. Shell configuration (bashrc) ---
if ! grep -q "ai-os" "$HOME/.bashrc" 2>/dev/null; then
    info "Configuring shell environment..."
    cat >> "$HOME/.bashrc" << 'BASHEOF'

# --- AI-OS aliases ---
alias dev='cd ~/ai-os'
alias c='clear'
alias x='exit'
alias gc='git commit -S'
alias gs='git status -sb'
alias gl='git log --oneline --graph --decorate --all'
alias gp='git push'
alias gpm='git push origin main'
BASHEOF
fi

# --- 7. Create project structure ---
PROJECT_DIR="$HOME/ai-os"
if [[ ! -d "$PROJECT_DIR" ]]; then
    info "Creating project structure..."
    mkdir -p "$PROJECT_DIR"/{core,brain,memory,runtime,perception,execution,services,system,docs,scripts,tools,tests,configs,assets,.github}

    # Placeholder files
    for d in core brain memory runtime perception execution services system docs scripts tools tests configs assets .github; do
        touch "$PROJECT_DIR/$d/.gitkeep"
    done

    # Root files
    cat > "$PROJECT_DIR/README.md" << 'EOF'
# AI-OS Platform
An AI-native Operating Platform built on Arch Linux.
EOF

    cat > "$PROJECT_DIR/.gitignore" << 'EOF'
.DS_Store
Thumbs.db
*.swp
*.swo
*~
target/
build/
dist/
__pycache__/
node_modules/
.vscode/
.idea/
.env
*.log
EOF

    # Initialize Git
    cd "$PROJECT_DIR"
    git init
    git checkout -b main
    git add -A
    git commit -m "Initial commit: AI-OS project scaffold"
fi

# --- 8. Verify ---
echo ""
info "=== Verification ==="
for cmd in git gcc clang cmake make rustc cargo go python pip node npm curl wget unzip zip tmux tree htop jq rg fd ssh vim nano; do
    if command -v "$cmd" &>/dev/null; then
        ok "$cmd found"
    else
        err "$cmd NOT found"
    fi
done

echo ""
ok "AI-OS development environment setup complete."
