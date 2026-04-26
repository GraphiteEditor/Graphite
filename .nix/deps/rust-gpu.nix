{ pkgs, ... }:

let
  extensions = [
    "rust-src"
    "rust-analyzer"
    "clippy"
    "cargo"
    "rustc-dev"
    "llvm-tools"
  ];
  toolchain = pkgs.rust-bin.nightly."2026-04-11".default.override {
    inherit extensions;
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
      (finalAttrs: rec {
        pname = "rustc_codegen_spirv";
        version = "0.10.0-alpha.1";
        src = pkgs.fetchCrate {
          inherit pname version;
          sha256 = "sha256-zJEpExkPgYzwo7fR4ge4GxJNj7H5yo4bJ4eTOw36+7c=";
        };
        cargoHash = "sha256-J1rtbfGqrL2NJ7Bu2pYfDwCdUmnECB/kzxrpYluA0kY=";
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
