# Wrap cargo bloat so that it uses cargo zigbuild in the background
final: prev:
let
  target = "aarch64-unknown-linux-gnu";
  zigTarget = "${target}.2.41";

  shim = final.writeShellScriptBin "cargo" ''
    if [ "$1" = build ]; then
      shift
      exec ${final.cargo-zigbuild}/bin/cargo-zigbuild zigbuild "''${@/#--target=${target}/--target=${zigTarget}}"
    fi
    exec "$REAL_CARGO" "$@"
  '';
in
{
  cargo-bloat = final.writeShellScriptBin "cargo-bloat" ''
    export REAL_CARGO="$(command -v cargo)"
    export CARGO="$REAL_CARGO"
    export PATH="${shim}/bin:$PATH"
    exec ${prev.cargo-bloat}/bin/cargo-bloat "$1" --target ${target} "''${@:2}"
  '';
}
