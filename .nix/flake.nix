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
    crane.url = "github:ipetkov/crane";

    # This is used to provide a identical development shell at `shell.nix` for users that do not use flakes
    flake-compat.url = "https://flakehub.com/f/edolstra/flake-compat/1.tar.gz";
  };

  outputs =
    inputs:
    inputs.flake-utils.lib.eachDefaultSystem (
      system:
      let
        info = {
          pname = "graphite";
          version = "unstable";
          src = ./..;
        };

        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [ (import inputs.rust-overlay) ];
        };

        deps = {
          crane = import ./deps/crane.nix { inherit pkgs inputs; };
          cef = import ./deps/cef.nix { inherit pkgs inputs; };
          rustGPU = import ./deps/rust-gpu.nix { inherit pkgs inputs; };
        };

        libs = rec {
          desktop = [
            pkgs.wayland
            pkgs.openssl
            pkgs.vulkan-loader
            pkgs.libraw
            pkgs.libGL
          ];
          desktop-x11 = [
            pkgs.libxkbcommon
            pkgs.xorg.libXcursor
            pkgs.xorg.libxcb
            pkgs.xorg.libX11
          ];
          desktop-all = desktop ++ desktop-x11;
          all = desktop-all;
        };

        tools = rec {
          desktop = [
            pkgs.pkg-config
          ];
          frontend = [
            pkgs.lld
            pkgs.nodejs
            pkgs.nodePackages.npm
            pkgs.binaryen
            pkgs.wasm-bindgen-cli_0_2_100
            pkgs.wasm-pack
            pkgs.cargo-about
          ];
          dev = [
            pkgs.rustc
            pkgs.cargo
            pkgs.rust-analyzer
            pkgs.clippy
            pkgs.rustfmt

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

            # Plotting tools
            pkgs.graphviz
          ];
          all = desktop ++ frontend ++ dev;
        };
      in
      {
        packages = rec {
          graphiteWithArgs =
            args:
            (import ./pkgs/graphite.nix {
              pkgs = pkgs // {
                inherit raster-nodes-shaders;
              };
              inherit
                info
                inputs
                deps
                libs
                tools
                ;
            })
              args;
          graphite = graphiteWithArgs { };
          graphite-dev = graphiteWithArgs { dev = true; };
          graphite-without-resources = graphiteWithArgs { embeddedResources = false; };
          graphite-without-resources-dev = graphiteWithArgs {
            embeddedResources = false;
            dev = true;
          };
          #TODO: graphene-cli = import ./pkgs/graphene-cli.nix { inherit info pkgs inputs deps libs tools; };
          raster-nodes-shaders = import ./pkgs/raster-nodes-shaders.nix {
            inherit
              info
              pkgs
              inputs
              deps
              libs
              tools
              ;
          };

          default = graphite;
        };

        devShells.default = import ./dev.nix {
          inherit
            pkgs
            deps
            libs
            tools
            ;
        };

        formatter = pkgs.nixfmt-tree;
      }
    );
}
