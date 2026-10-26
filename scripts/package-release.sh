#!/usr/bin/env bash
# Package built release binaries into dist/.
# Run after scripts/build-all.sh.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"

version=$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' Cargo.toml | head -1)
dist="$repo/dist"
rm -rf "$dist"
mkdir -p "$dist"

package() {
    target=$1
    name=$2
    bin="target/$target/release/arcfetch"
    if [ ! -f "$bin" ]; then
        echo "missing $bin (run scripts/build-all.sh first)" >&2
        exit 1
    fi

    stage=$(mktemp -d)
    install -m 755 "$bin" "$stage/arcfetch"
    install -m 644 "$repo/README.md" "$stage/README.md"
    install -m 644 "$repo/LICENSE" "$stage/LICENSE"
    cp -R "$repo/presets" "$stage/"
    tar -C "$stage" -czf "$dist/arcfetch-$version-$name.tar.gz" arcfetch README.md LICENSE presets
    rm -rf "$stage"
}

package x86_64-unknown-linux-gnu      linux-x86_64
package aarch64-linux-android        android-arm64
package armv7-linux-androideabi      android-armv7
package x86_64-linux-android         android-x86_64
package i686-linux-android           android-x86
package x86_64-apple-darwin          macos-x86_64
package aarch64-apple-darwin         macos-arm64
package x86_64-unknown-freebsd       freebsd-x86_64
package x86_64-unknown-netbsd        netbsd-x86_64

(
    cd "$dist"
    sha256sum ./*.tar.gz | sed 's#  \./#  #' > SHA256SUMS
)

echo "Artifacts:"
ls -lh "$dist"
