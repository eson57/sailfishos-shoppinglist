# LLM written thing that makes things work somehow
#
{ writeShellScript, runCommand, python3, qt56HostTools, sailfishSysroot }:

let
  # tree-sitter parses the C++ headers in moc-compat.py
  python = python3.withPackages (ps: [ ps.tree-sitter ps.tree-sitter-language-pack ]);

  sysroot = sailfishSysroot;

  qmake = writeShellScript "qmake" ''
    query() {
      case "$1" in
        QT_SYSROOT) echo "" ;;
        QT_INSTALL_PREFIX|QT_HOST_PREFIX) echo "${sysroot}/usr" ;;
        QT_INSTALL_ARCHDATA|QT_INSTALL_DATA|QT_HOST_DATA) echo "${sysroot}/usr/share/qt5" ;;
        QT_INSTALL_DOCS) echo "${sysroot}/usr/share/doc/qt5" ;;
        QT_INSTALL_HEADERS) echo "${sysroot}/usr/include/qt5" ;;
        QT_INSTALL_LIBS|QT_HOST_LIBS) echo "${sysroot}/usr/lib64" ;;
        QT_INSTALL_LIBEXECS|QT_HOST_LIBEXECS|QT_INSTALL_BINS|QT_HOST_BINS) echo "@out@/bin" ;;
        QT_INSTALL_TESTS) echo "${sysroot}/usr/lib64/qt5/tests" ;;
        QT_INSTALL_PLUGINS) echo "${sysroot}/usr/lib64/qt5/plugins" ;;
        QT_INSTALL_IMPORTS) echo "${sysroot}/usr/lib64/qt5/imports" ;;
        QT_INSTALL_QML) echo "${sysroot}/usr/lib64/qt5/qml" ;;
        QT_INSTALL_TRANSLATIONS) echo "${sysroot}/usr/share/qt5/translations" ;;
        QT_INSTALL_CONFIGURATION) echo "${sysroot}/etc/xdg" ;;
        QT_INSTALL_EXAMPLES|QT_INSTALL_DEMOS) echo "${sysroot}/usr/lib64/qt5/examples" ;;
        QMAKE_SPEC|QMAKE_XSPEC) echo "linux-g++" ;;
        QMAKE_VERSION) echo "3.0" ;;
        QT_VERSION) echo "5.6.3" ;;
        *) echo "**Unknown**" ;;
      esac
    }

    case "$1" in
      -query)
        if [ -n "$2" ]; then
          query "$2"
        else
          for k in QT_SYSROOT QT_INSTALL_PREFIX QT_INSTALL_ARCHDATA QT_INSTALL_DATA \
              QT_INSTALL_DOCS QT_INSTALL_HEADERS QT_INSTALL_LIBS QT_INSTALL_LIBEXECS \
              QT_INSTALL_BINS QT_INSTALL_TESTS QT_INSTALL_PLUGINS QT_INSTALL_IMPORTS \
              QT_INSTALL_QML QT_INSTALL_TRANSLATIONS QT_INSTALL_CONFIGURATION \
              QT_INSTALL_EXAMPLES QT_INSTALL_DEMOS QT_HOST_PREFIX QT_HOST_DATA \
              QT_HOST_BINS QT_HOST_LIBS QMAKE_SPEC QMAKE_XSPEC QMAKE_VERSION QT_VERSION; do
            echo "$k:$(query "$k")"
          done
        fi
        ;;
      -v|-version|--version)
        echo "QMake version 3.0"
        echo "Using Qt version 5.6.3 in ${sysroot}/usr/lib64"
        ;;
      *)
        echo "qmake: only -query and -version are supported" >&2
        exit 1
        ;;
    esac
  '';

  # moc-compat execs its first argument, which must be the real moc rather
  # than this wrapper.
  moc = writeShellScript "moc" ''
    exec ${python}/bin/python3 -I ${./moc-compat.py} ${qt56HostTools}/bin/moc ${sysroot} "$@"
  '';
in
runCommand "sailfish-qt-host-tools" { } ''
  mkdir -p "$out/bin"
  substitute ${qmake} "$out/bin/qmake" --subst-var out
  cp ${moc} "$out/bin/moc"
  ln -s ${qt56HostTools}/bin/rcc "$out/bin/rcc"
  chmod +x "$out/bin/qmake" "$out/bin/moc"
''
