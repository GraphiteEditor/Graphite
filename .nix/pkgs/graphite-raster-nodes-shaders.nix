{
  info,
  deps,
  self,
  system,
  ...
}:

(deps.crane.lib.overrideToolchain (_: deps.rustGPU.toolchain)).buildPackage {
  pname = "graphite-raster-nodes-shaders";
  inherit (info) version src;

  inherit (self.packages.${system}.graphite) cargoVendorDir cargoArtifacts;

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
