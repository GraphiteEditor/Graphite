{
  info,
  pkgs,
  inputs,
  deps,
  libs,
  tools,
  ...
}:

{
  embeddedResources ? true,
  dev ? false,
}:

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
  branding = pkgs.runCommand "${info.pname}-branding" { } ''
    mkdir -p $out
    tar -xvf ${brandingTar} -C $out --strip-components 1
  '';
  resourcesCommon = {
    pname = "${info.pname}-resources";
    inherit (info) version src;
    strictDeps = true;
    doCheck = false;
    nativeBuildInputs = tools.frontend;
    env.CARGO_PROFILE = if dev then "dev" else "release";
    cargoExtraArgs = "--target wasm32-unknown-unknown -p graphite-wasm --no-default-features --features native";
  };
  resources = deps.crane.lib.buildPackage (
    resourcesCommon
    // {
      cargoArtifacts = deps.crane.lib.buildDepsOnly resourcesCommon;

      # TODO: Remove the need for this hash by using individual package resolutions and hashes from package-lock.json
      npmDeps = pkgs.fetchNpmDeps {
        inherit (info) pname version;
        src = "${info.src}/frontend";
        hash = "sha256-D8VCNK+Ca3gxO+5wriBn8FszG8/x8n/zM6/MPo9E2j4=";
      };

      npmRoot = "frontend";
      npmConfigScript = "setup";
      makeCacheWritable = true;

      nativeBuildInputs = tools.frontend ++ [ pkgs.npmHooks.npmConfigHook ];

      prePatch = ''
        mkdir branding
        cp -r ${branding}/* branding
        cp ${info.src}/.branding branding/.branding
      '';

      buildPhase = ''
        export HOME="$TMPDIR"

        pushd frontend
        npm run native:build-${if dev then "dev" else "production"}
        popd
      '';

      installPhase = ''
        mkdir -p $out
        cp -r frontend/dist/* $out/
      '';
    }
  );
  common = {
    inherit (info) pname version src;
    strictDeps = true;
    buildInputs = libs.desktop-all;
    nativeBuildInputs = tools.desktop ++ [ pkgs.makeWrapper ];
    env = deps.cef.env // {
      CARGO_PROFILE = if dev then "dev" else "release";
    };
    cargoExtraArgs = "-p graphite-desktop${
      if embeddedResources then "" else " --no-default-features --features recommended"
    }";
    doCheck = false;
  };
in

deps.crane.lib.buildPackage (
  common
  // {
    cargoArtifacts = deps.crane.lib.buildDepsOnly common;

    env =
      common.env
      // {
        RASTER_NODES_SHADER_PATH = pkgs.raster-nodes-shaders;
      }
      // (
        if embeddedResources then
          {
            EMBEDDED_RESOURCES = resources;
          }
        else
          { }
      );

    postUnpack = ''
      mkdir ./branding
      cp -r ${branding}/* ./branding
    '';

    installPhase = ''
      mkdir -p $out/bin
      cp target/${if dev then "debug" else "release"}/graphite $out/bin/graphite

      mkdir -p $out/share/applications
      cp $src/desktop/assets/*.desktop $out/share/applications/

      mkdir -p $out/share/icons/hicolor/scalable/apps
      cp ${branding}/app-icons/graphite.svg $out/share/icons/hicolor/scalable/apps/art.graphite.Graphite.svg
    '';

    postFixup = ''
      wrapProgram "$out/bin/graphite" \
        --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath libs.desktop-all}:${deps.cef.env.CEF_PATH}" \
        --set CEF_PATH "${deps.cef.env.CEF_PATH}"
    '';
  }
)
