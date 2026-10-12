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
        version = "0.10.0";
        src = pkgs.fetchCrate {
          inherit (finalAttrs) pname version;
          sha256 = "sha256-QluNI2TdJI0j2CBOeAbB9DfOv+8LqzWk7siZyT1WWLU=";
        };
        cargoHash = "sha256-vvviUccnip+j60Ml1vqLNJ49KyEF40VLUoV1TMALHMs=";
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
