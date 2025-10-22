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
      owner = "Firestar99";
      repo = "rust-gpu-new";
      rev = "c12f216121820580731440ee79ebc7403d6ea04f";
      hash = "sha256-rG1cZvOV0vYb1dETOzzbJ0asYdE039UZImobXZfKIno=";
    };
    cargoHash = "sha256-AEigcEc5wiBd3zLqWN/2HSbkfOVFneAqNvg9HsouZf4=";
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
