{ dockerTools, stdenvNoCC, undocker, patch }:

let
  imageName = "registry.gitlab.com/whisperfish/sailo-rs/rust-aarch64-5.2.0.15";
  imageDigest = "sha256:af0fe090e0cc812789cf34d96e4e004c5fb36200b35465cef8557555be8df262";
  target = "SailfishOS-5.2.0.15-aarch64";

  image = dockerTools.pullImage {
    inherit imageName imageDigest;
    arch = "amd64";
    os = "linux";
    finalImageName = "sailfish-sdk";
    finalImageTag = "5.2.0.15";
    hash = "sha256-ELtyDSha+tJBI+KKuaXudC1zTi/83PHMdTMCsw8KNgs=";
  };
in
stdenvNoCC.mkDerivation {
  name = "sailfish-sdk-sysroot-${target}";
  nativeBuildInputs = [ undocker patch ];
  buildCommand = ''
    set -euo pipefail
    mkdir -p "$out"

    err="$TMPDIR/tar.err"
    set +e
    undocker ${image} - | tar -x \
      --no-same-owner \
      --no-same-permissions \
      --no-delay-directory-restore \
      --strip-components=4 \
      --exclude="srv/mer/targets/${target}/dev" \
      --exclude="srv/mer/targets/${target}/dev/*" \
      -C "$out" \
      "srv/mer/targets/${target}" 2>"$err"
    status=$?
    set -e

    if [ "$status" -ne 0 ]; then
      if [ "$status" -eq 2 ] \
        && [ -f "$out/usr/include/qt5/QtCore/qtypetraits.h" ] \
        && [ -f "$out/lib/ld-linux-aarch64.so.1" ] \
        && [ -f "$out/usr/lib64/libc.so" ]
      then
        :
      else
        cat "$err" >&2
        exit "$status"
      fi
    fi

    # Replace absolute library references with just the library names
    while IFS= read -r -d "" script; do
      if grep -q -a -m 1 'GNU ld script' "$script"; then
        sed -i \
          -e 's|/usr/lib64/||g' \
          -e 's|/usr/lib/||g' \
          -e 's|/lib64/||g' \
          -e 's|/lib/||g' \
          "$script"
      fi
    done < <(find "$out/usr/lib64" "$out/lib64" "$out/usr/lib" "$out/lib" -name '*.so' -type f -print0)

    # Stop clang complaining?
    patch -d "$out" -p1 -i ${./qtypetraits-clang.patch}

    # cxx-qt connects signals to move-only lambdas, which Qt < 5.10 cannot store
    patch -d "$out" -p1 -i ${./qobject-move-only-functors.patch}

    # Find the libc version, we'll pass this to cargo zigbuild later
    libc="$(find "$out" \( -path '*/lib/libc.so.6' -o -path '*/lib64/libc.so.6' \) -print -quit)"
    version="$(grep -a -o -h 'GLIBC_[0-9]\+\.[0-9]\+' "$libc" | sed 's/GLIBC_//' | sort -Vu | tail -1)"
    printf '%s\n' "$version" > "$out/glibc-version"

    find "$out" -type d -exec chmod a+rx {} +
    find "$out" -type f -exec chmod a+r {} +
  '';
}
