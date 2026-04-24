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
    cargoVendorDir = deps.crane.lib.vendorCargoDepsFlatten (
      deps.crane.lib.vendorMultipleCargoDeps {
        inherit (deps.crane.lib.findCargoFiles (deps.crane.lib.cleanCargoSource info.src)) cargoConfigs;
        cargoLockList = [
          "${info.src}/Cargo.lock"
          "${deps.rustGPU.toolchain.availableComponents.rust-src}/lib/rustlib/src/rust/library/Cargo.lock"
        ];
      }
    );
    buildInputs = libs;
    strictDeps = true;
    doCheck = false;
  };

  cargoArtifacts = deps.crane.lib.buildDepsOnly (
    common
    // {
      nativeBuildInputs = [
        pkgs.pkg-config
        pkgs.lld
      ];
      env.CEF_PATH = self.packages.${system}.graphite-cef;
      buildPhase =
        let
          profile = if dev then "dev" else "release";
        in
        ''
          cargo check --profile ${profile} --locked -p graphite-desktop-platform-linux
          cargo build --profile ${profile} --locked -p graphite-desktop-platform-linux

          cargo check --profile ${profile} --target wasm32-unknown-unknown --locked -p graphite-wasm-wrapper --no-default-features --features native
          cargo build --profile ${profile} --target wasm32-unknown-unknown --locked -p graphite-wasm-wrapper --no-default-features --features native

          cargo check --locked -p third-party-licenses --features desktop
          cargo build --locked -p third-party-licenses --features desktop

          cargo check --profile ${profile} --locked -p graphite-desktop-bundle
          cargo build --profile ${profile} --locked -p graphite-desktop-bundle
        '';
    }
  );
in

deps.crane.lib.buildPackage (
  common
  // {
    inherit cargoArtifacts;

    buildInputs = libs;
    nativeBuildInputs = [
      pkgs.pkg-config
      pkgs.lld
      pkgs.nodejs
      pkgs.binaryen
      pkgs.wasm-bindgen-cli_0_2_100
      pkgs.wasm-pack
      pkgs.cargo-about
      pkgs.removeReferencesTo
      pkgs.importNpmLock.npmConfigHook
    ];

    npmDeps = pkgs.importNpmLock {
      npmRoot = "${info.src}/frontend";
    };
    npmRoot = "frontend";
    npmConfigScript = "setup";
    makeCacheWritable = true;

    env = {
      RASTER_NODES_SHADER_PATH = self.packages.${system}.graphite-raster-nodes-shaders;
      GRAPHITE_GIT_COMMIT_HASH = self.rev or "unknown";
      GRAPHITE_GIT_COMMIT_DATE = self.lastModified or "unknown";
      CEF_PATH = self.packages.${system}.graphite-cef;
    };

    postPatch = ''
      mkdir branding
      cp -r ${branding}/* branding
      cp ${info.src}/.branding branding/.branding
    '';

    preBuild = ''
      # Prevent `package-installer.js` from trying to update npm dependencies
      touch -r frontend/package-lock.json -d '+1 year' frontend/node_modules/.install-timestamp

      export HOME="$TMPDIR"
    ''
    + (
      if self ? rev then
        ''
          export GRAPHITE_GIT_COMMIT_DATE="$(date -u -d "@$GRAPHITE_GIT_COMMIT_DATE" +"%Y-%m-%dT%H:%M:%SZ")"
        ''
      else
        ""
    );

    buildPhaseCargoCommand = "cargo run build desktop${if dev then " debug" else ""}";

    doNotPostBuildInstallCargoBinaries = true;

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
      remove-references-to -t "${common.cargoVendorDir}" $out/bin/graphite

      patchelf \
        --set-rpath "${pkgs.lib.makeLibraryPath libs}:${self.packages.${system}.graphite-cef}" \
        --add-needed libGL.so \
        $out/bin/graphite
    '';

    passthru.deps = cargoArtifacts;
  }
)
