#!/usr/bin/env bash
# Cargo linker wrapper for cross-compiling via zig (macOS/BSD).
# Zig ships the libc headers and CRT objects these targets need, so no
# platform SDK is required.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)

triple=""
for arg in "$@"; do
    case "$arg" in
        */target/*/*) triple=${arg#*/target/}; triple=${triple%%/*}; break ;;
    esac
done
if [ -z "$triple" ]; then
    echo "zig-linker: could not determine target triple from arguments" >&2
    exit 1
fi

case "$triple" in
    x86_64-apple-darwin)        zig_target=x86_64-macos.10.13 ;;
    aarch64-apple-darwin)       zig_target=aarch64-macos.11.0 ;;
    x86_64-unknown-freebsd)     zig_target=x86_64-freebsd ;;
    x86_64-unknown-netbsd)      zig_target=x86_64-netbsd ;;
    *)
        echo "zig-linker: no zig mapping for $triple" >&2
        exit 1
        ;;
esac

args=()
extra=()
case "$zig_target" in
    *-macos*)
        # Zig bundles libSystem but not libiconv; Rust never references it.
        for arg in "$@"; do
            [ "$arg" = "-liconv" ] || args+=("$arg")
        done
        ;;
    *-freebsd)
        # Rust links these base-system libs by default; none are referenced,
        # and zig only bundles libc. libgcc_s is stubbed by build-all.sh.
        for arg in "$@"; do
            case "$arg" in
                -lkvm|-lmemstat|-lprocstat|-ldevstat|-lexecinfo|-lutil) ;;
                *) args+=("$arg") ;;
            esac
        done
        extra=(-L "$repo/target/cross/lib")
        ;;
    *)
        args=("$@")
        ;;
esac

exec zig cc -target "$zig_target" "${extra[@]}" "${args[@]}"
