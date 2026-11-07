{ pkgs, config, ... }:

{
  packages = with pkgs; [
    cargo-zigbuild
    cargo-bloat
    jq
    just
    pkg-config
    rpm
    zig
    bacon
    librsvg
    qt6.qtdeclarative
  ];

  env.FONTCONFIG_FILE = pkgs.makeFontsConf {
    fontDirectories = [ pkgs.lmodern ];
  };

  overlays = [
    (import ./nix/rpm.overlay.nix)
    (import ./nix/cargo-bloat.overlay.nix)

    (final: _prev: {
      sailfishSysroot = final.callPackage ./nix/sysroot.nix {};
      qt56HostTools = final.callPackage ./nix/qt56-host-tools.nix {};
      sailfishQtHostTools = final.callPackage ./nix/qt-host-tools.nix {};
    })
  ];

  languages.rust = {
    enable = true;
    wild.enable = pkgs.stdenv.hostPlatform.isLinux;
    lld.enable = pkgs.stdenv.hostPlatform.isDarwin;
    channel = "nightly";
    targets = [ "aarch64-unknown-linux-gnu" ];
  };

  enterShell = ''
    export RUSTFLAGS="--remap-path-prefix src/=/src --remap-path-prefix ${config.env.DEVENV_ROOT}=/ --remap-path-prefix $HOME=~"

    eval "$(cargo metadata --manifest-path "$DEVENV_ROOT/Cargo.toml" --no-deps --format-version 1 \
      | jq -r '.metadata.sailjail | "export SAILJAIL_ORGANIZATION=\(.organization | @sh) SAILJAIL_APPLICATION=\(.application | @sh)"')"

    sysroot="${pkgs.sailfishSysroot}"
    glibc_version="$(cat "$sysroot/glibc-version")"
    cxx_root="$(find "$sysroot/usr/include/c++" -mindepth 1 -maxdepth 1 -type d | sort | head -n 1)"
    cxx_triple="$(find "$cxx_root" -mindepth 1 -maxdepth 1 -type d ! -name backward | sort | head -n 1)"
    cc_dir="$(find "$sysroot/usr/lib64/gcc" -name crtbegin.o -printf '%h\n' | head -n 1)"
    cflags="-idirafter $sysroot/usr/include"
    cxxflags="-nostdinc++ -std=gnu++17 -Wno-deprecated-copy -isystem $cxx_root -isystem $cxx_triple $cflags"

    export SFOS_SYSROOT="$sysroot"
    export SFOS_CARGO_TARGET="aarch64-unknown-linux-gnu.$glibc_version"
    export QT_INCLUDE_PATH="$sysroot/usr/include/qt5"
    export QT_LIBRARY_PATH="$sysroot/usr/lib64"
    export SAILFISHAPP_INCLUDE="$sysroot/usr/include/sailfishapp"
    export QMAKE="${pkgs.sailfishQtHostTools}/bin/qmake"
    export CFLAGS_aarch64_unknown_linux_gnu="$cflags"
    export CXXFLAGS_aarch64_unknown_linux_gnu="$cxxflags"
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-L native=$sysroot/usr/lib64 -L native=$sysroot/lib64 -L native=$sysroot/lib -L native=$cc_dir"

    qml_imports="$DEVENV_STATE/qml-imports"
    qml_processed="$qml_imports/.processed-$(basename "$sysroot")"
    if [ ! -e "$qml_processed" ]; then
      rm -rf "$qml_imports"
      mkdir -p "$qml_imports"
      cp -rL "$sysroot/usr/lib64/qt5/qml/Sailfish" "$sysroot/usr/lib64/qt5/qml/Nemo" "$qml_imports/"
      chmod -R u+w "$qml_imports"
      find "$qml_imports" -name '*.qmltypes' -delete
      find "$qml_imports" -name qmldir -exec sed -i '/^typeinfo/d' {} +
      touch "$qml_processed"
    fi
    export QML_IMPORT_PATH="${pkgs.qt6.qtdeclarative}/lib/qt-6/qml:$qml_imports"
  '';
}
