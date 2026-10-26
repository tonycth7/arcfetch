#!/usr/bin/env bash
# Build and install arcfetch.
#
# From a checkout:  ./install.sh
# One-liner:        curl -fsSL https://raw.githubusercontent.com/tonycth7/arcfetch/main/install.sh | bash
set -euo pipefail

binary=arcfetch
repo_url="${ARCFETCH_REPO:-https://github.com/tonycth7/arcfetch.git}"
install_dir="${ARCFETCH_INSTALL_DIR:-$HOME/.local/bin}"

# Pick up rustup when cargo is installed but not on PATH yet.
if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found" >&2
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" >&2
    exit 1
fi

# Use the checkout this script lives in, or the current checkout.
src=""
script=${BASH_SOURCE[0]:-}
if [ -n "$script" ] && [ -f "$(dirname "$script")/Cargo.toml" ]; then
    src=$(cd "$(dirname "$script")" && pwd)
elif [ -f Cargo.toml ] && grep -q '^name[[:space:]]*=[[:space:]]*"arcfetch"' Cargo.toml; then
    src=$PWD
fi

# Piped execution has no checkout, so clone one.
if [ -z "$src" ]; then
    command -v git >/dev/null 2>&1 || {
        echo "error: git not found" >&2
        exit 1
    }
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    echo "cloning arcfetch..."
    git clone --depth 1 "$repo_url" "$tmp/arcfetch"
    src="$tmp/arcfetch"
fi

cd "$src"
echo "building arcfetch (release)..."
cargo build --release --locked

mkdir -p "$install_dir"
install -m 755 "target/release/$binary" "$install_dir/$binary"

data_dir="${XDG_DATA_HOME:-$HOME/.local/share}/arcfetch"
mkdir -p "$data_dir/presets"
cp -R presets/examples "$data_dir/presets/"

echo
echo "installed -> $install_dir/$binary"
echo "presets   -> $data_dir/presets/examples"
case ":$PATH:" in
    *":$install_dir:"*) ;;
    *)
        echo
        echo "add to PATH:"
        echo "  export PATH=\"$install_dir:\$PATH\""
        ;;
esac
echo
echo "generate a config:"
echo "  $binary --config"
