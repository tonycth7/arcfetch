{
  description = "arcfetch — blazing-fast Unix sysinfo";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustPkgs = pkgs.rustPlatform;

        manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        version = manifest.package.version;

      in rec {
        packages.default = rustPkgs.buildRustPackage {
          pname = "arcfetch";
          inherit version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          # .cargo/config.toml asks for a static-pie Linux build, which nixpkgs'
          # dynamic glibc cannot link. NixOS always has glibc, so build dynamic.
          RUSTFLAGS = "-C target-feature=-crt-static";

          postInstall = ''
            mkdir -p $out/share/arcfetch
            cp -R presets $out/share/arcfetch/
          '';

          meta = {
            description = manifest.package.description;
            homepage = "https://github.com/tonycth7/arcfetch";
            license = pkgs.lib.licenses.mit;
            mainProgram = "arcfetch";
            platforms = pkgs.lib.platforms.unix;
          };
        };

        apps.default = flake-utils.lib.mkApp {
          drv = packages.default;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "rust-analyzer" ];
            })
          ];
        };
      }
    );
}
