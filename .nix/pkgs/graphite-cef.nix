{ pkgs, ... }:

let
  version = "151.3.24+g2384915+chromium-151.0.7922.174";
  hashes = {
    aarch64-linux = "sha256-R5ZbnDallYvdbW/bP+M2DzjRfWWRTvY2q63hSIHNxZs=";
    x86_64-linux = "sha256-21PEP9rOi37krw8AUSARbWlzqp2Ot3AsBhe635voaE4=";
  };

  selectSystem =
    attrs:
    attrs.${pkgs.stdenv.hostPlatform.system}
      or (throw "Unsupported system ${pkgs.stdenv.hostPlatform.system}");

  url = "https://cef-builds.spotifycdn.com/cef_binary_${version}_${
    selectSystem {
      aarch64-linux = "linuxarm64";
      x86_64-linux = "linux64";
    }
  }_minimal.tar.bz2";

  src = pkgs.fetchurl {
    inherit url;
    hash = selectSystem hashes;
  };
in
pkgs.cef-binary.overrideAttrs {
  version = builtins.head (builtins.split "\\+" version);
  inherit src;
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
        name = builtins.baseNameOf url;
        sha1 = "";
      }
    }' > $out/archive.json
  '';
}
