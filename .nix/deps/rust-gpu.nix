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
          rev = "e53a94458e1e699f0d5f56acfcd71fc483c37da0";
          sha256 = "sha256-V1c0tMjCr1e414fprID7cFFMHc2UlUUodKiBYqbvFB8=";
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
