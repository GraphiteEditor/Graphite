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
    crane.url = "github:ipetkov/crane";

    # This is used to provide a identical development shell at `shell.nix` for users that do not use flakes
    flake-compat.url = "https://flakehub.com/f/edolstra/flake-compat/1.tar.gz";
  };

  outputs =
    inputs:
    (
      let
        systems = [
          "x86_64-linux"
          "aarch64-linux"
        ];
        forAllSystems = f: inputs.nixpkgs.lib.genAttrs systems (system: f system);
        args =
          system:
          (
            let
              lib = inputs.nixpkgs.lib // {
                call = p: import p args;
              };

              pkgs = import inputs.nixpkgs {
                inherit system;
                overlays = [ (import inputs.rust-overlay) ];
              };

              info = {
                pname = "graphite";
                version = "unstable";
                src = inputs.nixpkgs.lib.cleanSourceWith {
                  src = ./..;
                  filter = path: type: !(type == "directory" && builtins.baseNameOf path == ".nix");
                };
                cargoVendored = deps.crane.lib.vendorCargoDeps { inherit (info) src; };
              };

              deps = {
                crane = lib.call ./deps/crane.nix;
                cef = lib.call ./deps/cef.nix;
                rustGPU = lib.call ./deps/rust-gpu.nix;
              };

              args = {
                inherit system;
                inherit (inputs) self;
                inherit inputs;
                inherit pkgs;
                inherit lib;
                inherit info;
                inherit deps;
              }
              // inputs;
            in
            args
          );
        withArgs = f: forAllSystems (system: f (args system));
      in
      {
        packages = withArgs (
          { lib, ... }:
          rec {
            default = graphite;
            graphite = (lib.call ./pkgs/graphite.nix) { };
            graphite-dev = (lib.call ./pkgs/graphite.nix) { dev = true; };
            graphite-raster-nodes-shaders = lib.call ./pkgs/graphite-raster-nodes-shaders.nix;
            graphite-branding = lib.call ./pkgs/graphite-branding.nix;
            graphite-bundle = lib.call ./pkgs/graphite-bundle.nix;
            graphite-flatpak-manifest = lib.call ./pkgs/graphite-flatpak-manifest.nix;

            # TODO: graphene-cli = lib.call ./pkgs/graphene-cli.nix;

            tools = {
              third-party-licenses = lib.call ./pkgs/tools/third-party-licenses.nix;
            };
          }
        );

        devShells = withArgs (
          { lib, ... }:
          {
            default = lib.call ./dev.nix;
          }
        );

        formatter = withArgs ({ pkgs, ... }: pkgs.nixfmt-tree);
      }
    );
}
