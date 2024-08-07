# This is a helper file for people using NixOS as their operating system.
# If you don't know what this file does, you can safely ignore it.

# If you are using Nix as your package manager, you can run 'nix-shell'
# in the root directory of the project and Nix will open a bash shell
# with all the packages needed to build and run Graphite installed.
# A shell.nix file is used in the Nix ecosystem to define a development
# environment with specific dependencies. When you enter a Nix shell using
# this file, it ensures that all the specified tools and libraries are
# available regardless of the host system's configuration. This provides
# a reproducible development environment across different machines and developers.

# You can enter the Nix shell and run Graphite like normal with:
# > npm start
# Or you can run it like this without needing to first enter the Nix shell:
# > nix-shell --command "npm start"

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
    extensions = [ "rust-src" "rust-analyzer" "clippy"];
  };
in
  # Make a shell with the dependencies we need
  pkgs.mkShell {
    packages = [
      rustc-wasm
      pkgs.nodejs
      pkgs.cargo
      pkgs.cargo-watch
      pkgs.cargo-nextest
      pkgs.cargo-expand
      pkgs.wasm-pack
      pkgs.binaryen
      pkgs.wasm-bindgen-cli
      pkgs.vulkan-loader
      pkgs.libxkbcommon
      pkgs.llvm
      pkgs.gcc-unwrapped.lib
      pkgs.llvmPackages.libcxxStdenv
      pkgs.pkg-config

      # For Tauri
      pkgs.openssl
      pkgs.glib
      pkgs.gtk3
      pkgs.libsoup
      pkgs.webkitgtk

      # For Raw-rs tests
      pkgs.libraw

      # Use Mold as a linker
      pkgs.mold
    ];

    # Hacky way to run Cargo through Mold
    LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [pkgs.openssl pkgs.vulkan-loader pkgs.libxkbcommon pkgs.llvmPackages.libcxxStdenv pkgs.gcc-unwrapped.lib pkgs.llvm pkgs.libraw];
    shellHook = ''
    alias cargo='mold --run cargo'
    '';
  }
