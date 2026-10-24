{ pkgs, ... }:

let
  version = "156.0.8+g426920c+chromium-156.0.8078.48";
  hashes = {
    aarch64-linux = "sha256-txaNkuJmxyg5zYv5/G1MQ57b19xcEs1+XEw/2IqVSHY=";
    x86_64-linux = "sha256-3FisWT1Gwh7ixdULjbVzD6zvpCAYOODJ8961k3K/tPo=";
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

  gl_rpath = pkgs.lib.makeLibraryPath [ pkgs.stdenv.cc.cc ];

  rpath = pkgs.lib.makeLibraryPath [
    pkgs.glib
    pkgs.nss
    pkgs.nspr
    pkgs.atk
    pkgs.at-spi2-atk
    pkgs.libdrm
    pkgs.expat
    pkgs.libxkbcommon
    pkgs.libgbm
    pkgs.gtk3
    pkgs.pango
    pkgs.cairo
    pkgs.alsa-lib
    pkgs.dbus
    pkgs.at-spi2-core
    pkgs.cups
    pkgs.libGL
    pkgs.udev
    pkgs.systemdLibs
    pkgs.libxcb
    pkgs.libx11
    pkgs.libxcomposite
    pkgs.libxdamage
    pkgs.libxext
    pkgs.libxfixes
    pkgs.libxrandr
    pkgs.libxshmfence
  ];
in
pkgs.stdenv.mkDerivation {
  pname = "cef-binary";
  version = builtins.head (builtins.split "\\+" version);

  src = pkgs.fetchurl {
    inherit url;
    hash = selectSystem hashes;
  };

  dontStrip = true;

  dontPatchELF = true;

  installPhase = ''
    runHook preInstall

    sed 's/-O0/-O2/' -i cmake/cef_variables.cmake
    patchelf --set-rpath "${rpath}" --set-interpreter "${pkgs.stdenv.cc.bintools.dynamicLinker}" Release/chrome-sandbox
    patchelf --add-needed libudev.so --set-rpath "${rpath}" Release/libcef.so
    if [ -e Release/libEGL.so ]; then
      patchelf --set-rpath "${gl_rpath}" Release/libEGL.so
    fi
    if [ -e Release/libGLESv2.so ]; then
      patchelf --add-needed libGL.so.1 --set-rpath "${gl_rpath}" Release/libGLESv2.so
    fi
    patchelf --set-rpath "${gl_rpath}" Release/libvk_swiftshader.so
    patchelf --set-rpath "${gl_rpath}" Release/libvulkan.so.1
    cp --recursive . $out

    runHook postInstall
  '';

  postInstall = ''
    rm -r $out/* $out/.* || true
    strip ./Release/*.so*
    mkdir $out/lib
    mv ./Release/* $out/lib/
    find "./Resources/locales" -maxdepth 1 -type f ! -name 'en-US.pak' -delete
    mv ./Resources/* $out/lib/
    mv ./include $out/lib/

    cat ./CREDITS.html | ${pkgs.xz}/bin/xz -9 -e -c > $out/lib/CREDITS.html.xz

    echo '${
      builtins.toJSON {
        type = "minimal";
        name = builtins.baseNameOf url;
        sha1 = "";
      }
    }' > $out/lib/archive.json
  '';
}
