{ pkgs, ... }:

let
  versionFile = builtins.fromJSON (builtins.readFile ./version.json);

  version = versionFile.version;
  hash = selectSystem versionFile.hashes;

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
    hash = hash;
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
