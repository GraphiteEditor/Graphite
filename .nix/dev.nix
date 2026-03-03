{ pkgs, deps, ... }:

let
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
in
pkgs.mkShell (
  {
    packages = libs ++ [
      pkgs.pkg-config

      pkgs.lld
      pkgs.nodejs
      pkgs.nodePackages.npm
      pkgs.binaryen
      pkgs.wasm-bindgen-cli_0_2_100
      pkgs.wasm-pack
      pkgs.cargo-about

      pkgs.rustc
      pkgs.cargo
      pkgs.rust-analyzer
      pkgs.clippy
      pkgs.rustfmt

      pkgs.git

      pkgs.cargo-watch
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

    LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath libs}:${deps.cef.env.CEF_PATH}";
    XDG_DATA_DIRS = "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS";

    shellHook = ''
      alias cargo='mold --run cargo'
    '';
  }
  // deps.cef.env
  // deps.rustGPU.env
)
