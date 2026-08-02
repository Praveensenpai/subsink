#!/bin/bash

CYAN='\033[0;36m'
GREEN='\033[0;32m'
PURPLE='\033[0;35m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${PURPLE}⛩️  Installing subsink (Japanese Anime SubSyncer)...${NC}\n"

BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"

REPO="Praveensenpai/subsink"
RELEASE_URL="https://github.com/${REPO}/releases/latest/download/subsink-linux-x86_64.tar.gz"

LOCAL_DIR=""
if [ -n "${BASH_SOURCE[0]}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
    LOCAL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" 2>/dev/null && pwd)"
fi

if [ -n "$LOCAL_DIR" ] && [ -f "$LOCAL_DIR/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
    VERSION=$(grep -m1 '^version' "$LOCAL_DIR/Cargo.toml" | cut -d '"' -f2 2>/dev/null || echo "latest")
    echo -e "${BLUE}📦 Local source detected. Building subsink v${VERSION} with Cargo...${NC}"
    cargo build --release --manifest-path "$LOCAL_DIR/Cargo.toml"
    cp "$LOCAL_DIR/target/release/subsink" "$BIN_DIR/subsink"
    INSTALLED_VER="v${VERSION}"
else
    LATEST_TAG=$(curl -4 -sSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep -o '"tag_name": "[^"]*"' | cut -d'"' -f4)
    [ -z "$LATEST_TAG" ] && LATEST_TAG="latest"
    echo -e "${BLUE}📦 Downloading subsink ${LATEST_TAG} pre-compiled binary from GitHub Releases...${NC}"
    TMP_DIR=$(mktemp -d)
    if curl -4 -fL --connect-timeout 10 --retry 3 -sS "$RELEASE_URL" -o "$TMP_DIR/subsink.tar.gz"; then
        tar -xzf "$TMP_DIR/subsink.tar.gz" -C "$TMP_DIR"
        if [ -f "$TMP_DIR/subsink" ]; then
            cp "$TMP_DIR/subsink" "$BIN_DIR/subsink"
        elif [ -f "$TMP_DIR/dist/subsink" ]; then
            cp "$TMP_DIR/dist/subsink" "$BIN_DIR/subsink"
        fi
        rm -rf "$TMP_DIR"
        INSTALLED_VER="${LATEST_TAG}"
    else
        rm -rf "$TMP_DIR"
        echo -e "${RED}❌ Failed to download pre-compiled release.${NC}"
        exit 1
    fi
fi

if [ ! -f "$BIN_DIR/subsink" ] || [ ! -s "$BIN_DIR/subsink" ]; then
    echo -e "${RED}❌ Error: Failed to install subsink binary!${NC}"
    exit 1
fi

chmod +x "$BIN_DIR/subsink"
echo -e "${GREEN}✔ Installed subsink ${INSTALLED_VER} to ${BIN_DIR}/subsink${NC}"

# Shell alias setup
SHELL_CONFIGS=("$HOME/.bashrc" "$HOME/.zshrc")
ALIAS_LINE="alias subsink='$HOME/.local/bin/subsink'"

for config in "${SHELL_CONFIGS[@]}"; do
    if [ -f "$config" ]; then
        if ! grep -q "alias subsink=" "$config" 2>/dev/null; then
            echo "" >> "$config"
            echo "$ALIAS_LINE" >> "$config"
            echo -e "${BLUE}📝 Added subsink alias to $config${NC}"
        fi
    fi
done

echo -e "\n${GREEN}${BOLD}🎉 subsink ${INSTALLED_VER} installation completed!${NC}"
echo -e "Run it anytime with: ${CYAN}subsink${NC}"
