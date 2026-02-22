{ info, deps, pkgs, ...}:

let
  cargoVendorDir = deps.crane.lib.vendorCargoDeps { inherit (info) src; };
  common = {
    pname = "third-party-licenses";
    inherit (info) version src;
    inherit cargoVendorDir;
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.openssl ];
    strictDeps = true;
    env = deps.cef.env // {
      CARGO_PROFILE = "dev";
    };
    cargoExtraArgs = "-p third-party-licenses";
    doCheck = false;
  };
in
deps.crane.lib.buildPackage common // {
  inherit cargoVendorDir;
  cargoArtifacts = deps.crane.lib.buildDepsOnly common;
  meta.mainProgram = "third-party-licenses";
}
