#!/usr/bin/env bash
set -e
BINARY="arcfetch"
INSTALL_DIR="${HOME}/.local/bin"

command -v cargo >/dev/null 2>&1 || {
    echo "error: cargo not found"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
}

echo "building arcfetch (release)..."
cargo build --release

mkdir -p "${INSTALL_DIR}"
install -Dm755 "target/release/${BINARY}" "${INSTALL_DIR}/${BINARY}"
echo ""
echo "installed → ${INSTALL_DIR}/${BINARY}"
echo ""
echo "add to PATH if needed:"
echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
echo ""
echo "quick config setup:"
echo "  mkdir -p ~/.config/arcfetch"
echo "  echo 'accent = mauve' > ~/.config/arcfetch/config"
echo ""
echo "add to ~/.zshrc / ~/.bashrc / config.fish:"
echo "  arcfetch"
echo "  arcfetch --logo pi --accent mauve"
