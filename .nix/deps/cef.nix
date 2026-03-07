{ pkgs, ... }:

let
  cefPath = pkgs.cef-binary.overrideAttrs (finalAttrs: {
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
  });
in
{
  env.CEF_PATH = cefPath;
}
