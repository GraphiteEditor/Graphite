{
  pkgs,
  deps,
  libs,
  tools,
  ...
}:

pkgs.mkShell (
  {
    packages = tools.all;

    LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath libs.desktop-all}:${deps.cef.env.CEF_PATH}";
    XDG_DATA_DIRS = "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS";

    shellHook = ''
      alias cargo='mold --run cargo'
    '';
  }
  // deps.cef.env
  // deps.rustGPU.env
)
