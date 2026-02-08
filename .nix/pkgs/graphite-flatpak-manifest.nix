{
  pkgs,
  archive,
}:

(pkgs.formats.json { }).generate "art.graphite.Graphite.json" {
  app-id = "art.graphite.Graphite";
  runtime = "org.freedesktop.Platform";
  runtime-version = "25.08";
  sdk = "org.freedesktop.Sdk";
  command = "graphite";
  finish-args = [
    "--device=dri"
    "--share=ipc"
    "--socket=wayland"
    "--socket=fallback-x11"
    "--share=network"
  ];
  modules = [
    {
      name = "app";
      buildsystem = "simple";
      build-commands = [
        "mkdir -p /app"
        "cp -r ./* /app/"
        "chmod +x /app/bin/*"
      ];
      sources = [
        {
          type = "archive";
          path = archive;
          strip-components = 0;
        }
      ];
    }
  ];
}
