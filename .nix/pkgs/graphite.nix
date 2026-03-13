{
  info,
  pkgs,
  self,
  deps,
  system,
  lib,
  ...
}:

{
  dev ? false,
}:

let
  branding = self.packages.${system}.graphite-branding;
  cargoVendorDir = deps.crane.lib.vendorCargoDeps { inherit (info) src; };
  resourcesCommon = {
    pname = "${info.pname}-resources";
    inherit (info) version src;
    inherit cargoVendorDir;
    strictDeps = true;
    nativeBuildInputs = [
      pkgs.pkg-config
      pkgs.lld
      pkgs.nodejs
      pkgs.nodePackages.npm
      pkgs.binaryen
      pkgs.wasm-bindgen-cli_0_2_100
      pkgs.wasm-pack
      pkgs.cargo-about
    ];
    buildInputs = [ pkgs.openssl ];
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

      nativeBuildInputs = [
        pkgs.importNpmLock.npmConfigHook
        pkgs.removeReferencesTo
      ]
      ++ resourcesCommon.nativeBuildInputs;

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
  libs = [
    pkgs.wayland
    pkgs.vulkan-loader
    pkgs.libGL
    pkgs.openssl
    pkgs.libraw

    # X11 Support
    pkgs.libxkbcommon
    pkgs.libXcursor
    pkgs.libxcb
    pkgs.libX11
  ];
  common = {
    inherit (info) pname version src;
    inherit cargoVendorDir;
    strictDeps = true;
    buildInputs = libs;
    nativeBuildInputs = [
      pkgs.pkg-config
      pkgs.cargo-about
      pkgs.removeReferencesTo
    ];
    env = deps.cef.env // {
      CARGO_PROFILE = if dev then "dev" else "release";
    };
    cargoExtraArgs = "-p graphite-desktop";
    doCheck = false;
  };
in

deps.crane.lib.buildPackage (
  common
  // {
    cargoArtifacts = deps.crane.lib.buildDepsOnly common;

    env = common.env // {
      RASTER_NODES_SHADER_PATH = self.packages.${system}.graphite-raster-nodes-shaders;
      EMBEDDED_RESOURCES = resources;
      GRAPHITE_GIT_COMMIT_HASH = self.rev or "unknown";
      GRAPHITE_GIT_COMMIT_DATE = self.lastModified or "unknown";
    };

    npmDeps = pkgs.importNpmLock {
      npmRoot = "${info.src}/frontend";
    };
    npmRoot = "frontend";
    nativeBuildInputs = [
      pkgs.importNpmLock.npmConfigHook
      pkgs.nodePackages.npm
    ]
    ++ common.nativeBuildInputs;

    preBuild = ''
      ${lib.getExe self.packages.${system}.tools.third-party-licenses}
    ''
    + (
      if self ? rev then
        ''
          export GRAPHITE_GIT_COMMIT_DATE="$(date -u -d "@$GRAPHITE_GIT_COMMIT_DATE" +"%Y-%m-%dT%H:%M:%SZ")"
        ''
      else
        ""
    );

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
        --set-rpath "${pkgs.lib.makeLibraryPath libs}:${deps.cef.env.CEF_PATH}" \
        --add-needed libGL.so \
        $out/bin/graphite
    '';
  }
)
