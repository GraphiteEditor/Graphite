{
  pkgs,
  deps,
  self,
  system,
  lib,
  ...
}:

{
  desktop ? false,
  x11 ? false,
  playwright ? false,
}:

let
  features = [
    {
      packages = [
        pkgs.pkg-config

        pkgs.lld
        pkgs.nodejs
        pkgs.binaryen
        pkgs.wasm-bindgen-cli_0_2_121
        pkgs.cargo-about

        pkgs.rustc
        pkgs.cargo
        pkgs.rust-analyzer
        pkgs.clippy
        pkgs.rustfmt

        pkgs.git

        pkgs.cargo-nextest
        pkgs.cargo-expand

        # Linker
        pkgs.mold

        # Profiling tools
        pkgs.gnuplot
        pkgs.samply
        pkgs.cargo-flamegraph

        # Plotting tools
        pkgs.graphviz
      ];
    }
    deps.rustGPU
    (lib.optionalAttrs desktop (
      {
        libs = [
          pkgs.wayland
          pkgs.vulkan-loader
          pkgs.libGL
          pkgs.libxkbcommon
          self.packages.${system}.graphite-cef
        ];
        env = {
          CEF_PATH = "${self.packages.${system}.graphite-cef}/lib";
          XDG_DATA_DIRS = "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS";
        };
      }
    ))
    (lib.optionalAttrs x11 {
      libs = [
        pkgs.libXcursor
        pkgs.libxcb
        pkgs.libX11
      ];
    })
    (lib.optionalAttrs playwright (
      assert pkgs.playwright-driver.version == (lib.importJSON ../tools/web-editor-driver/playwright/package-lock.json).packages."node_modules/playwright-core".version;
      {
        env = {
          PLAYWRIGHT_BROWSERS_PATH = pkgs.playwright-driver.browsers-chromium;
          PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
        };
      }
    ))
  ];

  libs = lib.concatMap (feature: feature.libs or [ ]) features;
in
pkgs.mkShell {
  packages = libs ++ lib.concatMap (feature: feature.packages or [ ]) features;
  env = lib.mergeAttrsList (map (feature: feature.env or { }) features) // {
    LD_LIBRARY_PATH = lib.makeLibraryPath libs;
  };
}
