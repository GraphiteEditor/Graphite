{ info, deps, ... }:

(deps.crane.lib.overrideToolchain (_: deps.rustGPU.toolchain)).buildPackage {
  pname = "raster-nodes-shaders";
  inherit (info) version src;

  cargoVendorDir = deps.crane.lib.vendorMultipleCargoDeps {
    inherit (deps.crane.lib.findCargoFiles (deps.crane.lib.cleanCargoSource info.src)) cargoConfigs;
    cargoLockList = [
      "${info.src}/Cargo.lock"
      "${deps.rustGPU.toolchain.availableComponents.rust-src}/lib/rustlib/src/rust/library/Cargo.lock"
    ];
  };

  strictDeps = true;

  env = deps.rustGPU.env;

  buildPhase = ''
    cargo build -r -p raster-nodes-shaders
  '';

  installPhase = ''
    cp target/spirv-builder/spirv-unknown-naga-wgsl/release/deps/raster_nodes_shaders_entrypoint.wgsl $out
  '';

  doCheck = false;
}
