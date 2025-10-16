{ pkgs }:

let
  libcef = pkgs.libcef.overrideAttrs (_: _: {
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
