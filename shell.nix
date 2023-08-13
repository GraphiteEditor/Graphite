# This is a helper file for people using NixOs as their Operating System
# > If you don't know what this file does you can safely ignore it :D

# If you are using nix as your package manager, you can run 'nix-shell'
# in the root directory of the project and nix will open a bash shell
# with all the packages needed to build and run Graphite installed.
# A shell.nix file is used in the Nix ecosystem to define a development
# environment with specific dependencies. When you enter a Nix shell using
# this file, it ensures that all the specified tools and libraries are
# available regardless of the host system's configuration. This provides
# a reproducible development environment across different machines and developers.

# If you don't need the shell, you can build Graphite using this command:
# nix-shell --command "npm start"

let
  # Get oxalica's Rust overlay for better Rust integration
  rust-overlay-source = builtins.fetchGit {
    url = "https://github.com/oxalica/rust-overlay";
  };

  # Import it so we can use it in Nix
  rust-overlay = import rust-overlay-source;

  # Import system packages overlaid with the Rust overlay
  pkgs = import <nixpkgs> {
    overlays = [ rust-overlay ];
  };

  # Define the rustc we need
  rustc-wasm = pkgs.rust-bin.stable.latest.default.override {
    targets = [ "wasm32-unknown-unknown" ];
    # wasm-pack needs this
    extensions = [ "rust-src" ];
  };
in
  # Make a shell with the dependencies we need
  pkgs.mkShell {
    packages = [
      rustc-wasm
      pkgs.nodejs
      pkgs.cargo
      pkgs.cargo-watch
      pkgs.wasm-pack

      pkgs.openssl
      pkgs.glib
      pkgs.gtk3
      pkgs.libsoup
      pkgs.webkitgtk

      pkgs.pkg-config

      # Use Mold as a Linke
      pkgs.mold
    ];

    # Hacky way to run cago through Mold
    shellHook = ''
    alias cargo='mold --run cargo'
    '';
  }

