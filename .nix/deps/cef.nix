{ pkgs, inputs, ... }:

let
  cef = pkgs.cef-binary.overrideAttrs (_: _: {
    postInstall = ''
      strip $out/Release/*.so*
    '';
  });

  cefPath = pkgs.runCommand "cef-path" {} ''
    mkdir -p $out

    ln -s ${cef}/include $out/include
    find ${cef}/Release -name "*" -type f -exec ln -s {} $out/ \;
    find ${cef}/Resources -name "*" -maxdepth 1 -exec ln -s {} $out/ \;

    echo '${builtins.toJSON {
      type = "minimal";
      name = builtins.baseNameOf cef.src.url;
      sha1 = "";
    }}' > $out/archive.json
  '';
in
{
  env.CEF_PATH = cefPath;
}
