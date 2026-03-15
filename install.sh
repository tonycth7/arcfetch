#!/usr/bin/env bash
set -e
BINARY="arcfetch"
INSTALL_DIR="${HOME}/.local/bin"

command -v cargo >/dev/null 2>&1 || {
    echo "error: cargo not found — install rustup: https://rustup.rs"
    exit 1
}

echo "Building arcfetch (release)..."
cargo build --release

mkdir -p "${INSTALL_DIR}"
install -Dm755 "target/release/${BINARY}" "${INSTALL_DIR}/${BINARY}"
echo "Installed to ${INSTALL_DIR}/${BINARY}"
echo ""
echo "Tip: add ~/.local/bin to PATH if not already there"
echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
echo ""
echo "Quick config:"
echo "  mkdir -p ~/.config/arcfetch"
echo "  echo 'mauve' > ~/.config/arcfetch/accent"
