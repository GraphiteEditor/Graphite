{ pkgs, ... }:

let
  version = "149.0.5+g6770623+chromium-149.0.7827.197";

  # Local custom CEF build (tarball) instead of the Spotify CDN download.
  # Absolute path outside the flake source tree, so this needs `nix develop --impure`.
  src = /home/timon/tmp/cef-build/dist/cef_149_7827_x86-64_linux_fixed_buffer_usage.tar.xz;
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

    if [ -f ./CREDITS.html ]; then
      cat ./CREDITS.html | ${pkgs.xz}/bin/xz -9 -e -c > $out/CREDITS.html.xz
    fi

    echo '${
      builtins.toJSON {
        type = "minimal";
        name = "cef_binary_${version}_linux64_minimal.tar.xz";
        sha1 = "";
      }
    }' > $out/archive.json
  '';
})
