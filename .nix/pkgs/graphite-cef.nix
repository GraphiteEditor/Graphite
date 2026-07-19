{ pkgs, ... }:

let
  version = "149.7827.0";
  upstream = "149.0.5+g6770623+chromium-149.0.7827.197";

  selectSystem =
    attrs:
    attrs.${pkgs.stdenv.hostPlatform.system}
      or (throw "Unsupported system ${pkgs.stdenv.hostPlatform.system}");

  src = selectSystem {
    x86_64-linux = pkgs.fetchurl {
      url = "https://github.com/timon-schelling/graphite-cef/releases/download/v${version}/graphite_cef_x86-64_linux.tar.xz";
      hash = "sha256-lrxAALouHjQlw5lSSZka/BNpa5PVjjcncbofgKruOWk=";
    };
    aarch64-linux = pkgs.fetchurl {
      url = "https://cef-builds.spotifycdn.com/cef_binary_${upstream}_linuxarm64_minimal.tar.bz2";
      hash = "sha256-cBAvcvs1rAg5EKJkCt81RZYupCWpUNIC/nLt3PJow7Q=";
    };
  };
in
pkgs.cef-binary.overrideAttrs (_: {
  inherit src version;
  postInstall = ''
    rm -r $out/* $out/.* || true
    strip ./Release/*.so*
    mv ./Release/* $out/
    find "./Resources/locales" -maxdepth 1 -type f ! -name 'en-US.pak' -delete
    mv ./Resources/* $out/
    mv ./include $out/

    cat ./CREDITS.html | ${pkgs.xz}/bin/xz -9 -e -c > $out/CREDITS.html.xz

    echo '${
      builtins.toJSON {
        type = "minimal";
        name = "cef_binary_${upstream}";
        sha1 = "";
      }
    }' > $out/archive.json
  '';
})
