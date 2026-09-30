{ pkgs, ... }:

let
  toolchain = pkgs.rust-bin.nightly."2026-07-03".default.override {
    extensions = [
      "rust-src"
      "rust-analyzer"
      "clippy"
      "cargo"
      "rustc-dev"
      "llvm-tools"
    ];
  };
  cargo = pkgs.writeShellScriptBin "cargo" ''
    #!${pkgs.lib.getExe pkgs.bash}

    filtered_args=()
    for arg in "$@"; do
      case "$arg" in
        +nightly|+nightly-*) ;;
        *) filtered_args+=("$arg") ;;
      esac
    done

    exec ${toolchain}/bin/cargo ${"\${filtered_args[@]}"}
  '';
  rustc_codegen_spirv =
    (pkgs.makeRustPlatform {
      cargo = toolchain;
      rustc = toolchain;
    }).buildRustPackage
      (finalAttrs: {
        pname = "rustc_codegen_spirv";
        version = "0.10.0-alpha.1";
        src = pkgs.fetchgit {
          url = "https://github.com/Rust-GPU/rust-gpu";
          rev = "2fff75a21a1909945ac85f5818843dd9960004d7";
          sha256 = "sha256-clUt4jUh4D9EmoXZFgur1RrMYob4X5jti2e2hvBnAx8=";
        };
        cargoHash = "sha256-9dGb+RDYpJKaIhkMPkZAx1M1VGE3gMfCD2T84i1VtV8=";
        cargoBuildFlags = [
          "-p"
          "rustc_codegen_spirv"
          "--features=use-compiled-tools"
          "--no-default-features"
        ];
        doCheck = false;
      });
in
{
  toolchain = toolchain;
  env = {
    RUST_GPU_PATH_OVERRIDE = "${cargo}/bin:${toolchain}/bin";
    RUSTC_CODEGEN_SPIRV_PATH = "${rustc_codegen_spirv}/lib/librustc_codegen_spirv.so";
  };
}
