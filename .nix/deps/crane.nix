{
  pkgs,
  inputs,
  ...
}:

{
  lib = (inputs.crane.mkLib pkgs) // {
    vendorCargoDepsFlatten =
      src:
      pkgs.stdenvNoCC.mkDerivation (finalAttrs: {
        name = "graphite-cargo-vendored";
        inherit src;

        installPhase = ''
          cp -rL --no-preserve=mode "$src" "$out"
          chmod -R u+w "$out"
          find "$out" -type f -print0 | xargs -r -0 sed -i "s|$src|$out|g"
        '';

        disallowedReferences = [ finalAttrs.src ];

        dontUnpack = true;
        dontConfigure = true;
        dontBuild = true;
      });
  };
}
