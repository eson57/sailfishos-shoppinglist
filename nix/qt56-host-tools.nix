# Build Qt5.6's moc and rcc tools for the host system
{ lib, stdenv, fetchurl, perl, python3, zlib, xcbuild }:

stdenv.mkDerivation {
  pname = "qt56-host-tools";
  version = "5.6.3";

  src = fetchurl {
    url = "https://download.qt.io/archive/qt/5.6/5.6.3/submodules/qtbase-opensource-src-5.6.3.tar.xz";
    hash = "sha256-/vSFKab8JhejDXXZUssyfGvjQf0QQVSZOSIYSzs7TaE=";
  };

  nativeBuildInputs = [ perl python3 ] ++ lib.optional stdenv.hostPlatform.isDarwin xcbuild;
  buildInputs = [ zlib ];

  hardeningDisable = [ "all" ];
  env.NIX_CFLAGS_COMPILE = "-Wno-error -Wno-deprecated-declarations -include cstdint -std=gnu++14";

  postPatch = ''
    # We need to patch the code to work with modern clang for some reason
    sed -i 's@^// Clang is missing the intrinsic for _bit_scan_reverse@&\n#undef _bit_scan_reverse\n#undef _bit_scan_forward@' src/corelib/tools/qsimd_p.h
    sed 's#usr/include/qt5/QtCore/qtypetraits.h#src/corelib/global/qtypetraits.h#' ${./qtypetraits-clang.patch} | patch -p1 --fuzz=3

    grep -rlI "/bin/pwd" . | xargs sed -i "s|/bin/pwd|pwd|g"

    # The sandbox has no /usr/bin/xcrun etc.; use xcbuild's on PATH instead.
    # (configure also reads only the first digit of clang's version, so clang 21
    # looks like "2" and selects macx-g++; the platform is forced below.)
    ${lib.optionalString stdenv.hostPlatform.isDarwin ''
      sed -i -E 's#/usr/bin/(xcrun|xcode-select|xcodebuild)#\1#g' configure mkspecs/features/mac/sdk.prf
      # Qt 5.6 targets macOS 10.7, which the current SDK refuses to compile Foundation calls for.
      sed -i 's/^QMAKE_MACOSX_DEPLOYMENT_TARGET = .*/QMAKE_MACOSX_DEPLOYMENT_TARGET = 10.13/' mkspecs/macx-clang/qmake.conf
    ''}
  '';

  configurePhase = ''
    runHook preConfigure
    ./configure -prefix "$TMPDIR/unused" -opensource -confirm-license -release ${lib.optionalString stdenv.cc.isClang "-platform linux-clang"} \
      ${lib.optionalString stdenv.hostPlatform.isDarwin "-platform macx-clang -no-framework"} -no-opengl -no-gui -no-widgets -no-dbus -no-icu -no-glib \
      -no-iconv -no-cups -no-fontconfig -qt-pcre \
      -system-zlib -nomake examples -nomake tests -nomake tools
    runHook postConfigure
  '';

  buildPhase = ''
    runHook preBuild
    for t in bootstrap moc rcc; do
      (cd src/tools/$t && ../../../bin/qmake && make -j$NIX_BUILD_CORES LINK="$CXX")
    done
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    install -Dm755 bin/moc "$out/bin/moc"
    install -Dm755 bin/rcc "$out/bin/rcc"
    runHook postInstall
  '';
}
