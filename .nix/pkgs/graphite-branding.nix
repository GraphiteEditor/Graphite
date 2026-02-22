{ info, pkgs, ... }:

let
  brandingTar = pkgs.fetchurl (
    let
      lockContent = builtins.readFile "${info.src}/.branding";
      lines = builtins.filter (s: s != [ ]) (builtins.split "\n" lockContent);
      url = builtins.elemAt lines 0;
      hash = builtins.elemAt lines 1;
    in
    {
      url = url;
      sha256 = hash;
    }
  );
in
pkgs.runCommand "${info.pname}-branding" { } ''
  mkdir -p $out
  tar -xvf ${brandingTar} -C $out --strip-components 1
''
