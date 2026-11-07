(final: prev: {
  rpm =
    let
      rpmElfdeps = final.writeTextFile {
        name = "rpm-elfdeps";
        destination = "/bin/elfdeps";
        executable = true;
        text =
          builtins.replaceStrings
            [ "@PYTHON@" "@ELFDEPS@" ]
            [ "${final.python3}/bin/python3" (final.lib.getExe final.elfdeps) ]
            (builtins.readFile ./rpm-elfdeps.py);
      };
    in
    prev.symlinkJoin {
      name = "rpm";
      paths = [ prev.rpm ];
      postBuild = ''
        define_provides="__elf_provides ${rpmElfdeps}/bin/elfdeps --provides --multifile"
        define_requires="__elf_requires ${rpmElfdeps}/bin/elfdeps --requires --multifile"
        rpm_orig=$(readlink -f "$out/bin/rpm")
        rpmbuild_orig=$(readlink -f "$out/bin/rpmbuild")
        write_wrapper() {
          rm -f "$out/bin/$1"
          printf '%s\n' \
            '#!/bin/sh' \
            "exec $2 \\" \
            "  --define \"$define_provides\" \\" \
            "  --define \"$define_requires\" \\" \
            '  "$@"' \
            > "$out/bin/$1"
          chmod +x "$out/bin/$1"
        }
        write_wrapper rpm "$rpm_orig"
        write_wrapper rpmbuild "$rpmbuild_orig"
      '';
    };
})
