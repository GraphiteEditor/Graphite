{ pkgs, ... }:

let
  version = "147.0.14+g76d2442+chromium-147.0.7727.138";
  hashes = {
    aarch64-linux = "sha256-Gy2Xs1NHwmIr+buzoqDso1QJVkKlA/UMXytHjNGqpNk=";
		x86_64-linux = "sha256-os7wAFJ+mVK65HCikvEjhMeQUj2ty7y+6Ad0OlOcbeA=";
  };

  selectSystem =
    attrs:
    attrs.${pkgs.stdenv.hostPlatform.system}
      or (throw "Unsupported system ${pkgs.stdenv.hostPlatform.system}");

  src = pkgs.fetchurl {
    url = "https://cef-builds.spotifycdn.com/cef_binary_${version}_${
      selectSystem {
        aarch64-linux = "linuxarm64";
        x86_64-linux = "linux64";
      }
    }_minimal.tar.bz2";
    hash = selectSystem hashes;
  };
in
pkgs.cef-binary.overrideAttrs (finalAttrs: {
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
        name = builtins.baseNameOf finalAttrs.src.url;
        sha1 = "";
      }
    }' > $out/archive.json
  '';
})
