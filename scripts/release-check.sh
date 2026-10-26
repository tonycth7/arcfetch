#!/usr/bin/env bash
# Local release pre-flight. Does not tag, push, or publish anything.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"

version=$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' Cargo.toml | head -1)
[ -n "$version" ] || { echo "cannot read version from Cargo.toml" >&2; exit 1; }

check() {
    file=$1
    pattern=$2
    grep -Eq "$pattern" "$file" || {
        echo "$file is not set to $version" >&2
        exit 1
    }
}

check Cargo.toml "^version[[:space:]]*=[[:space:]]*\"$version\""
check Cargo.lock "^version = \"$version\""
check PKGBUILD "^pkgver=$version$"
check .SRCINFO "^[[:space:]]*pkgver = $version$"

[ -f LICENSE ] || { echo "LICENSE is missing" >&2; exit 1; }

if grep -q "sha256sums=('SKIP')" PKGBUILD; then
    echo "note: AUR checksum is SKIP; run updpkgsums after the v$version tag exists."
fi

cargo test --release --locked
./scripts/build-all.sh
./scripts/package-release.sh

echo
echo "arcfetch $version release artifacts are ready in dist/."
echo "Nothing has been tagged, pushed, or published."
