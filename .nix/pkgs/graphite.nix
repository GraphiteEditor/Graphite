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
        hash = "sha256-UWuJpKNYj2Xn34rpMDZ75pzMYUOLQjPeGuJ/QlPbX9A=";
      };

      npmRoot = "frontend";
      npmConfigScript = "setup";
      makeCacheWritable = true;

      nativeBuildInputs = tools.frontend ++ [ pkgs.npmHooks.npmConfigHook ];

      buildPhase = ''
        export HOME="$TMPDIR"

        pushd frontend
        npm run build-native${if dev then "-dev" else ""}
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

    installPhase = ''
      mkdir -p $out/bin
      cp target/${if dev then "debug" else "release"}/graphite $out/bin/graphite

      mkdir -p $out/share/applications
      cp $src/desktop/assets/*.desktop $out/share/applications/

      mkdir -p $out/share/icons/hicolor/scalable/apps
      cp $src/desktop/assets/graphite-icon-color.svg $out/share/icons/hicolor/scalable/apps/
    '';

    postFixup = ''
      wrapProgram "$out/bin/graphite" \
        --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath libs.desktop-all}:${deps.cef.env.CEF_PATH}" \
        --set CEF_PATH "${deps.cef.env.CEF_PATH}"
    '';
  }
)
