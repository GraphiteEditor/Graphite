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
  cargoVendorDir =  deps.crane.lib.vendorCargoDeps { inherit (info) src; };
  resourcesCommon = {
    pname = "${info.pname}-resources";
    inherit (info) version src;
    inherit cargoVendorDir;
    strictDeps = true;
    nativeBuildInputs = tools.frontend;
    env.CARGO_PROFILE = if dev then "dev" else "release";
    cargoExtraArgs = "--target wasm32-unknown-unknown -p graphite-wasm --no-default-features --features native";
    doCheck = false;
  };
  resources = deps.crane.lib.buildPackage (
    resourcesCommon
    // {
      cargoArtifacts = deps.crane.lib.buildDepsOnly resourcesCommon;

      npmDeps = pkgs.importNpmLock {
        npmRoot = "${info.src}/frontend";
      };

      npmRoot = "frontend";
      npmConfigScript = "setup";
      makeCacheWritable = true;

      nativeBuildInputs = tools.frontend ++ [ pkgs.importNpmLock.npmConfigHook pkgs.removeReferencesTo ];

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

      postFixup = ''
        find "$out" -type f -exec remove-references-to -t "${cargoVendorDir}" '{}' +
      '';
    }
  );
  common = {
    inherit (info) pname version src;
    inherit cargoVendorDir;
    strictDeps = true;
    buildInputs = libs.desktop-all;
    nativeBuildInputs = tools.desktop ++ [ pkgs.makeWrapper pkgs.removeReferencesTo ];
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
      ) // {
        GRAPHITE_GIT_COMMIT_HASH = inputs.self.rev or "unknown";
        GRAPHITE_GIT_COMMIT_DATE = inputs.self.lastModified or "unknown";
      };

    postUnpack = ''
      mkdir ./branding
      cp -r ${branding}/* ./branding
    '';

    preBuild = if inputs.self ? rev then ''
      export GRAPHITE_GIT_COMMIT_DATE="$(date -u -d "@$GRAPHITE_GIT_COMMIT_DATE" +"%Y-%m-%dT%H:%M:%SZ")"
    '' else "";

    installPhase = ''
      mkdir -p $out/bin
      cp target/${if dev then "debug" else "release"}/graphite $out/bin/graphite

      mkdir -p $out/share/applications
      cp $src/desktop/assets/*.desktop $out/share/applications/

      mkdir -p $out/share/icons/hicolor/scalable/apps
      cp ${branding}/app-icons/graphite.svg $out/share/icons/hicolor/scalable/apps/art.graphite.Graphite.svg
      mkdir -p $out/share/icons/hicolor/512x512/apps
      cp ${branding}/app-icons/graphite-512.png $out/share/icons/hicolor/512x512/apps/art.graphite.Graphite.png
      mkdir -p $out/share/icons/hicolor/256x256/apps
      cp ${branding}/app-icons/graphite-256.png $out/share/icons/hicolor/256x256/apps/art.graphite.Graphite.png
      mkdir -p $out/share/icons/hicolor/128x128/apps
      cp ${branding}/app-icons/graphite-128.png $out/share/icons/hicolor/128x128/apps/art.graphite.Graphite.png
    '';

    postFixup = ''
      remove-references-to -t "${cargoVendorDir}" $out/bin/graphite

      patchelf \
        --set-rpath "${pkgs.lib.makeLibraryPath libs.desktop-all}:${deps.cef.env.CEF_PATH}" \
        --add-needed libGL.so \
        $out/bin/graphite
    '';
  }
)
