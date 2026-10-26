# Release 1.1.0

Nothing in this file runs automatically. The commands below publish only when you run them.

## 1. Pre-flight

```bash
./scripts/release-check.sh
```

This checks that Cargo, PKGBUILD and `.SRCINFO` all say `1.1.0`, runs the tests, builds every target and writes release tarballs to `dist/`.

The Nix package is separate from that script:

```bash
nix build .# --no-link -L
```

It builds a normal dynamic binary (NixOS always has glibc); the static-pie build in `.cargo/config.toml` applies to `cargo build --release` and the release tarballs.

The build needs the Rust targets listed in `scripts/build-all.sh`, Zig, and an Android NDK:

```bash
rustup target add \
  aarch64-linux-android armv7-linux-androideabi \
  x86_64-linux-android i686-linux-android \
  x86_64-apple-darwin aarch64-apple-darwin \
  x86_64-unknown-freebsd x86_64-unknown-netbsd

sudo pacman -S zig
export ANDROID_NDK_HOME="$HOME/Android/Sdk/ndk/27.1.12297001"
```

The NDK version only needs to be one that provides API 24 clang wrappers.

## 2. Commit and tag

```bash
git add .
git commit -m "release: 1.1.0"
git tag -a v1.1.0 -m "arcfetch 1.1.0"
```

Do not tag until the code is final.

## 3. Publish

```bash
git push origin main
git push origin v1.1.0
```

Pushing the tag runs `.github/workflows/release.yml`. It runs the tests, builds the same artifacts, uploads them, and creates the GitHub release. You can also run the workflow manually to build artifacts without creating a release.

## 4. AUR

`PKGBUILD` currently uses `sha256sums=('SKIP')` because the `v1.1.0` tarball does not exist until the tag is pushed. After step 3:

```bash
updpkgsums
makepkg --printsrcinfo > .SRCINFO
makepkg -si
```

Then push `PKGBUILD` and `.SRCINFO` to the AUR repository:

```bash
git clone ssh://aur@aur.archlinux.org/arcfetch.git /tmp/aur-arcfetch
cp PKGBUILD .SRCINFO /tmp/aur-arcfetch/
cd /tmp/aur-arcfetch
git add PKGBUILD .SRCINFO
git commit -m "upgpkg: arcfetch 1.1.0-1"
git push
```

## Release contents

GitHub release tarballs (binary, README, LICENSE and `presets/`):

- Linux x86_64
- Android arm64, armv7, x86_64 and x86
- macOS x86_64 and arm64
- FreeBSD x86_64
- NetBSD x86_64

OpenBSD has no prebuilt Rust target here and is not covered by `scripts/build-all.sh`, so it is not in the release matrix.
