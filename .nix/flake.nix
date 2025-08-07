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

        rustExtensions = [ "rust-src" "rust-analyzer" "clippy" "cargo" ];

        rust = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
          extensions = rustExtensions;
        };

        rustNightlyPkg = pkgs.rust-bin.nightly."2025-06-23".default.override {
          extensions = rustExtensions ++ [ "rustc-dev" "llvm-tools" ];
        };

        rustPlatformNightly = pkgs.makeRustPlatform {
          cargo = rustNightlyPkg;
          rustc = rustNightlyPkg;
        };

        rustc_codegen_spirv = rustPlatformNightly.buildRustPackage (finalAttrs: {
          pname = "rustc_codegen_spirv";
          version = "0-unstable-2025-08-04";
          src = pkgs.fetchFromGitHub {
            owner = "Rust-GPU";
            repo = "rust-gpu";
            rev = "df1628a032d22c864397417c2871b74d602af986";
            hash = "sha256-AFt3Nc+NqK8DxNUhDBcOUmk3XDVcoToVeFIMYNszdbY=";
          };
          cargoHash = "sha256-en3BYJWQabH064xeAwYQrvcr6EuWg/QjvsG+Jd6HHCk";

          cargoBuildFlags = [ "-p" "rustc_codegen_spirv" "--features=use-installed-tools" "--no-default-features" ];

          doCheck = false;
        });

        cargoGpuPkg = rustPlatformNightly.buildRustPackage (finalAttrs: {
          pname = "cargo-gpu";
          version = "0-unstable-2025-07-24";
          src = pkgs.fetchFromGitHub {
            owner = "Rust-GPU";
            repo = "cargo-gpu";
            rev = "a2ad3574dd32142ff661994e0d79448a45d18f47";
            hash = "sha256-YGu9Cuw+pcN9/rCuCxImouzsQ3ScHF+cW6zgxMm0XGI=";
          };
          cargoHash = "sha256-tyad9kO90uwAnMQYa09takIBXifrumSx2C4rpSK95aM=";

          doCheck = false;
        });

        cargoNightlyPkg = pkgs.writeShellScriptBin "cargo-nightly" ''
          #!${pkgs.bash}/bin/bash

          exec ${rustNightlyPkg}/bin/cargo $@
        '';


        libcef = pkgs.libcef.overrideAttrs (finalAttrs: previousAttrs: {
          version = "139.0.17";
          gitRevision = "6c347eb";
          chromiumVersion = "139.0.7258.31";
          srcHash = "sha256-kRMO8DP4El1qytDsAZBdHvR9AAHXce90nPdyfJailBg=";

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
          rust
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

          pkgs.spirv-tools
          cargoNightlyPkg
          cargoGpuPkg
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

          RUSTC_CODEGEN_SPIRV="${rustc_codegen_spirv}/lib/librustc_codegen_spirv.so";

          shellHook = ''
            alias cargo='mold --run cargo'
          '';
        };
      }
    );
}
