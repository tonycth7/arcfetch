#!/usr/bin/env bash
# Build arcfetch for every supported target.
#
# Host, Android, macOS, FreeBSD and NetBSD. Cross-targets are linked with zig
# (no platform SDK needed); Android uses the NDK.
#
# Requirements: rustup targets for the listed platforms, zig, and an Android NDK.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"

cross=target/cross
mkdir -p "$cross/lib"

# FreeBSD: link-time stand-in for the system libgcc_s.so.1 unwinder.
zig cc -target x86_64-freebsd -shared -Wl,-soname,libgcc_s.so.1 \
    scripts/stubs/unwind_stub.c -o "$cross/lib/libgcc_s.so"

host=$(rustc -vV | sed -n 's/^host: //p')
targets=(
    "$host"
    aarch64-linux-android
    armv7-linux-androideabi
    x86_64-linux-android
    i686-linux-android
    x86_64-apple-darwin
    aarch64-apple-darwin
    x86_64-unknown-freebsd
    x86_64-unknown-netbsd
)

for target in "${targets[@]}"; do
    printf '\n==> %s\n' "$target"
    cargo build --release --locked --target "$target"
done

printf '\nArtifacts:\n'
for target in "${targets[@]}"; do
    bin="target/$target/release/arcfetch"
    if [ -f "$bin" ]; then
        printf '  %-28s %s\n' "$target" "$(file -b "$bin" | cut -c1-72)"
    else
        printf '  %-28s (missing)\n' "$target"
    fi
done
