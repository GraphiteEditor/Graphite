{ pkgs }:

let
  toolchainPkg = pkgs.rust-bin.nightly."2025-06-23".default.override {
    extensions =
      [ "rust-src" "rust-analyzer" "clippy" "cargo" "rustc-dev" "llvm-tools" ];
  };
  toolchainRustPlatform = pkgs.makeRustPlatform {
    cargo = toolchainPkg;
    rustc = toolchainPkg;
  };
  rustc_codegen_spirv = toolchainRustPlatform.buildRustPackage (finalAttrs: {
    pname = "rustc_codegen_spirv";
    version = "0-unstable-2025-08-04";
    src = pkgs.fetchFromGitHub {
      owner = "Rust-GPU";
      repo = "rust-gpu";
      rev = "3f05f5482824e3b1fbb44c9ef90a8795a0204c7c";
      hash = "sha256-ygNxjkzuvcO2jLYhayNuIthhH6/seCbTq3M0IkbsDrY=";
    };
    cargoHash = "sha256-SzTvKUG/da//pHb7hN230wRsQ6BYAkP8HoXqJO30/dU=";
    cargoBuildFlags = [
      "-p"
      "rustc_codegen_spirv"
      "--features=use-compiled-tools"
      "--no-default-features"
    ];
    doCheck = false;
  });
  cargoWrapper = pkgs.writeShellScriptBin "cargo" ''
    #!${pkgs.lib.getExe pkgs.bash}

    filtered_args=()
    for arg in "$@"; do
      case "$arg" in
        +nightly|+nightly-*) ;;
        *) filtered_args+=("$arg") ;;
      esac
    done

    exec ${toolchainPkg}/bin/cargo ${"\${filtered_args[@]}"}
  '';
in {
  RUST_GPU_PATH_OVERRIDE = "${cargoWrapper}/bin:${toolchainPkg}/bin";
  RUSTC_CODEGEN_SPIRV_PATH =
    "${rustc_codegen_spirv}/lib/librustc_codegen_spirv.so";
}
