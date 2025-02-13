# This is a helper file for people using NixOS as their operating system.
# If you don't know what this file does, you can safely ignore it.
# This file defines both the development environment for the project.
#
# Development Environment:
# - Provides all necessary tools for Rust/WASM development
# - Includes Tauri dependencies for desktop app development
# - Sets up profiling and debugging tools
# - Configures mold as the default linker for faster builds
#
#
# Usage:
# - Development shell: `nix develop`
# - Run in dev shell with direnv: add `use flake` to .envrc
{
  description = "Development environment and build configuration";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";
    nixpkgs-unstable.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, nixpkgs-unstable, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        pkgs-unstable = import nixpkgs-unstable {
          inherit system overlays;
        };
        
        rustc-wasm = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
          extensions = [ "rust-src" "rust-analyzer" "clippy" "cargo" ];
        };

        # Shared build inputs - system libraries that need to be in LD_LIBRARY_PATH
        buildInputs = with pkgs; [
          # System libraries
          openssl
          vulkan-loader
          libxkbcommon
          llvmPackages.libcxxStdenv
          gcc-unwrapped.lib
          llvm
          # libgbm
          mesa
          libraw

          # Graphics and GPU support
          mesa
          mesa.drivers
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
          libglvnd
          libdrm
          wayland

          # Tauri dependencies
          at-spi2-atk
          atkmm
          cairo
          gdk-pixbuf
          glib
          gtk3
          harfbuzz
          librsvg
          libsoup_3
          pango
          webkitgtk_4_1
          openssl
        ];

        # Development tools that don't need to be in LD_LIBRARY_PATH
        buildTools =  [
          rustc-wasm
          pkgs.cargo
          pkgs.nodejs
          pkgs.nodePackages.npm
          pkgs.binaryen
          pkgs.wasm-bindgen-cli
          pkgs-unstable.wasm-pack
          pkgs.pkg-config
          pkgs.git
          pkgs.gobject-introspection
          pkgs-unstable.cargo-tauri

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

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

          shellHook = ''
            alias cargo='mold --run cargo'
          '';
        };
      }
    );
}
