{
  pkgs,
  self,
  system,
  ...
}:
let
  bundle =
    {
      archive ? false,
      compression ? null,
      passthru ? { },
    }:
    (
      let
        graphite = self.packages.${system}.graphite;
        tar = if compression == null then archive else true;
        nameArchiveSuffix = if tar then ".tar" else "";
        nameCompressionSuffix = if compression == null then "" else "." + compression;
        name = "graphite-bundle${nameArchiveSuffix}${nameCompressionSuffix}";
        build = ''
          mkdir -p out
          mkdir -p out/bin
          cp ${graphite}/bin/graphite out/bin/graphite
          chmod -v +w out/bin/graphite
          patchelf \
            --set-rpath '$ORIGIN/../lib:$ORIGIN/../lib/cef' \
            --set-interpreter '/lib64/ld-linux-x86-64.so.2' \
            --remove-needed libGL.so \
            out/bin/graphite
          cp -r ${graphite}/share out/share
          mkdir -p out/lib/cef
          mkdir -p ./cef
          tar -xvf ${pkgs.cef-binary.src} -C ./cef --strip-components=1
          cp -r ./cef/Release/* out/lib/cef/
          cp -r ./cef/Resources/* out/lib/cef/
          find "out/lib/cef/locales" -type f ! -name 'en-US*' -delete
          ${pkgs.bintools}/bin/strip out/lib/cef/*.so*
        '';
        install =
          if tar then
            ''
              cd out
              tar -c \
              --sort=name \
              --mtime='@1' --clamp-mtime \
              --owner=0 --group=0 --numeric-owner \
              --mode='u=rwX,go=rX' \
              --format=posix \
              --pax-option=delete=atime,delete=ctime \
              --no-acls --no-xattrs --no-selinux \
              * ${
                if compression == "xz" then
                  "| xz "
                else if compression == "gz" then
                  "| gzip -n "
                else
                  ""
              }> $out
            ''
          else
            ''
              mkdir -p $out
              cp -r out/* $out/
            '';
      in

      pkgs.runCommand name
        {
          inherit passthru;
        }
        ''
          ${build}
          ${install}
        ''
    );
in
bundle {
  passthru = {
    tar = bundle {
      archive = true;
      passthru = {
        gz = bundle {
          compression = "gz";
        };
        xz = bundle {
          compression = "xz";
        };
      };
    };
  };
}
