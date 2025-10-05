{ pkgs }:

let
  libcef = pkgs.libcef.overrideAttrs (finalAttrs: previousAttrs: {
    version = "139.0.17";
    gitRevision = "6c347eb";
    chromiumVersion = "139.0.7258.31";
    srcHash = "sha256-kRMO8DP4El1qytDsAZBdHvR9AAHXce90nPdyfJailBg=";

    __intentionallyOverridingVersion = true;

    postInstall = ''
      strip $out/lib/*
    '';
  });
  cefPath = pkgs.runCommand "cef-path" {} ''
    mkdir -p $out

    ln -s ${libcef}/include $out/include
    find ${libcef}/lib -type f -name "*" -exec ln -s {} $out/ \;
    find ${libcef}/libexec -type f -name "*" -exec ln -s {} $out/ \;
    cp -r ${libcef}/share/cef/* $out/

    echo '${builtins.toJSON {
      type = "minimal";
      name = builtins.baseNameOf libcef.src.url;
      sha1 = "";
    }}' > $out/archive.json
  '';
in
{
  CEF_PATH = cefPath;
}
