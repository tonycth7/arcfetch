#!/usr/bin/env bash
# Cargo linker wrapper for Android targets.
#
# Finds the NDK (ANDROID_NDK_HOME / ANDROID_NDK_ROOT, or a standard SDK path),
# recovers the target triple from the `-o target/<triple>/...` argument, and
# forwards everything to that triple's NDK clang.
set -euo pipefail

triple=""
for arg in "$@"; do
    case "$arg" in
        */target/*/*) triple=${arg#*/target/}; triple=${triple%%/*}; break ;;
    esac
done
if [ -z "$triple" ]; then
    echo "android-linker: could not determine target triple from arguments" >&2
    exit 1
fi

ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [ -z "$ndk" ]; then
    for d in "$HOME/Android/Sdk/ndk"/* /opt/android-sdk/ndk/* /usr/lib/android-sdk/ndk/*; do
        [ -d "$d" ] && ndk="$d"
    done
fi
if [ -z "$ndk" ]; then
    echo "android-linker: NDK not found; set ANDROID_NDK_HOME" >&2
    exit 1
fi

case "$(uname -s)" in
    Linux)                 host=linux-x86_64 ;;
    Darwin)                host=darwin-x86_64 ;;
    MINGW*|MSYS*|CYGWIN*)  host=windows-x86_64 ;;
    *) echo "android-linker: unsupported host $(uname -s)" >&2; exit 1 ;;
esac

# getifaddrs/freeifaddrs are API 24, so the binary targets Android 7+.
case "$triple" in
    armv7-linux-androideabi) cc_triple=armv7a-linux-androideabi ;;
    *)                       cc_triple=$triple ;;
esac
cc="$ndk/toolchains/llvm/prebuilt/$host/bin/${cc_triple}24-clang"
if [ ! -x "$cc" ]; then
    echo "android-linker: missing $cc (set ANDROID_NDK_HOME)" >&2
    exit 1
fi

exec "$cc" "$@"
