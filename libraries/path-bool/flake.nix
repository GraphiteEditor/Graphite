{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = {self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem ( system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        toolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "clippy" "rust-analyzer"];
        };
        buildInputs = with pkgs; [
            llvm
        ];
        in {
        devShells.default = pkgs.mkShell {
          stdenv = pkgs.clangStdenv;
          packages = with pkgs; [
            bacon
            valgrind
            kcachegrind
            cargo-flamegraph
          ];
          nativeBuildInputs = with pkgs; [
            lld
            toolchain
            llvm
            cargo
          ];
          inherit buildInputs;
          
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
        };
      }
    );
}
