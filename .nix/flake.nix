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
  description = "Development environment and build configuration";

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
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustc-wasm = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
          extensions = [ "rust-src" "rust-analyzer" "clippy" "cargo" ];
        };

        libcef = pkgs.libcef.overrideAttrs (finalAttrs: previousAttrs: {
          version = "138.0.26";
          gitRevision = "84f2d27";
          chromiumVersion = "138.0.7204.158";
          srcHash = "sha256-d9jQJX7rgdoHfROD3zmOdMSesRdKE3slB5ZV+U2wlbQ=";

          __intentionallyOverridingVersion = true;

          postInstall = ''
            strip $out/lib/*
          '';
        });

        libcefPath = pkgs.runCommand "libcef-path" {} ''
          mkdir -p $out

          ln -s ${libcef}/include $out/include
          find ${libcef}/lib -type f -name "*" -exec ln -s {} $out/ \;
          find ${libcef}/libexec -type f -name "*" -exec ln -s {} $out/ \;
          cp -r ${libcef}/share/cef/* $out/

          echo '${builtins.toJSON {
            type = "minimal";
            name = builtins.baseNameOf libcef.src.url;
            sha1 = "";
          }}' > $out/archive.json
        '';

        # Shared build inputs - system libraries that need to be in LD_LIBRARY_PATH
        buildInputs = with pkgs; [
          # System libraries
          wayland
          openssl
          vulkan-loader
          libraw
          libGL

          # X11 libraries, not needed on wayland! Remove when x11 is finally dead
          libxkbcommon
          xorg.libXcursor
          xorg.libxcb
          xorg.libX11
        ];

        # Development tools that don't need to be in LD_LIBRARY_PATH
        buildTools =  [
          rustc-wasm
          pkgs.nodejs
          pkgs.nodePackages.npm
          pkgs.binaryen
          pkgs.wasm-bindgen-cli
          pkgs.wasm-pack
          pkgs.pkg-config
          pkgs.git
          pkgs.cargo-about

          # Linker
          pkgs.mold
        ];
        # Development tools that don't need to be in LD_LIBRARY_PATH
        devTools = with pkgs; [
          cargo-watch
          cargo-nextest
          cargo-expand

          # Profiling tools
          gnuplot
          samply
          cargo-flamegraph
        ];
      in
      {
        # Development shell configuration
        devShells.default = pkgs.mkShell {
          packages = buildInputs ++ buildTools ++ devTools;

          LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath buildInputs}:${libcefPath}";
          CEF_PATH = libcefPath;
          XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS";

          shellHook = ''
            alias cargo='mold --run cargo'
          '';
        };
      }
    );
}
