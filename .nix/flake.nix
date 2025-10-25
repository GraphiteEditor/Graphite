# This is a helper file for people using NixOS as their operating system.
# If you don't know what this file does, you can safely ignore it.
# This file defines the reproducible development environment for the project.
#
# Development Environment:
# - Provides all necessary tools for Rust/Wasm development
# - Includes dependencies for desktop app development
# - Sets up profiling and debugging tools
# - Configures mold as the default linker for faster builds
#
# Usage:
# - Development shell: `nix develop .nix` from the project root
# - Run in dev shell with direnv: add `use flake` to .envrc
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";

    # This is used to provide a identical development shell at `shell.nix` for users that do not use flakes
    flake-compat.url = "https://flakehub.com/f/edolstra/flake-compat/1.tar.gz";
  };

  outputs = { nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustExtensions = [ "rust-src" "rust-analyzer" "clippy" "cargo" ];
        rust = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
          extensions = rustExtensions;
        };

        # Shared build inputs; libraries that need to be in LD_LIBRARY_PATH
        buildInputs = [
          pkgs.wayland
          pkgs.openssl
          pkgs.vulkan-loader
          pkgs.libraw
          pkgs.libGL

          # X11 libraries, not needed on wayland! Remove when x11 is finally dead
          pkgs.libxkbcommon
          pkgs.xorg.libXcursor
          pkgs.xorg.libxcb
          pkgs.xorg.libX11
        ];

        # Packages needed to build the package
        buildTools = [
          rust
          pkgs.nodejs
          pkgs.nodePackages.npm
          pkgs.binaryen
          pkgs.wasm-bindgen-cli_0_2_100
          pkgs.wasm-pack
          pkgs.pkg-config
          pkgs.cargo-about
        ];

        # Development tools; not needed to build the package
        devTools = [
          pkgs.git

          pkgs.cargo-watch
          pkgs.cargo-nextest
          pkgs.cargo-expand

          # Linker
          pkgs.mold

          # Profiling tools
          pkgs.gnuplot
          pkgs.samply
          pkgs.cargo-flamegraph
        ];

        cefEnv = import ./cef.nix { inherit pkgs; };
        rustGPUEnv = import ./rust-gpu.nix { inherit pkgs; };

        libPath = "${pkgs.lib.makeLibraryPath buildInputs}:${cefEnv.CEF_PATH}";
      in {
        devShells.default = pkgs.mkShell ({
          packages = buildInputs ++ buildTools ++ devTools;

          LD_LIBRARY_PATH = libPath;
          XDG_DATA_DIRS =
            "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS";

          shellHook = ''
            alias cargo='mold --run cargo'
          '';
        } // cefEnv // rustGPUEnv);

        packages.default = pkgs.rustPlatform.buildRustPackage (finalAttrs: {
          pname = "graphite-editor";
          version = "unstable";
          src = pkgs.lib.cleanSource ./..;

          cargoLock = {
            lockFile = ../Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          # TODO: Remove the need for this hash by using individual package resolutions and hashes from package-lock.json
          npmDeps = pkgs.fetchNpmDeps {
            inherit (finalAttrs) pname version;
            src = "${finalAttrs.src}/frontend";
            hash = "sha256-UWuJpKNYj2Xn34rpMDZ75pzMYUOLQjPeGuJ/QlPbX9A=";
          };

          npmRoot = "frontend";
          npmConfigScript = "setup";
          makeCacheWritable = true;

          buildInputs = buildInputs;
          nativeBuildInputs = buildTools ++ [
            pkgs.rustPlatform.cargoSetupHook
            pkgs.npmHooks.npmConfigHook
            pkgs.makeWrapper
          ];

          env = cefEnv // rustGPUEnv;

          buildPhase = ''
            export HOME="$TMPDIR"

            pushd frontend
            npm run build-native
            popd
            cargo build -r -p graphite-desktop
          '';

          installPhase = ''
            mkdir -p $out/bin
            cp target/release/graphite-desktop $out/bin/graphite-editor

            mkdir -p $out/share/applications
            cp $src/desktop/assets/*.desktop $out/share/applications/

            mkdir -p $out/share/icons/hicolor/scalable/apps
            cp $src/desktop/assets/graphite-icon-color.svg $out/share/icons/hicolor/scalable/apps/
          '';

          doCheck = false;

          postFixup = ''
            wrapProgram "$out/bin/graphite-editor" \
              --prefix LD_LIBRARY_PATH : "${libPath}" \
              --set CEF_PATH "${cefEnv.CEF_PATH}"
          '';
        });
      });
}
