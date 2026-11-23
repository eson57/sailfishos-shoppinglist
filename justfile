app := env("SAILJAIL_APPLICATION")
org := env("SAILJAIL_ORGANIZATION")

default:
    @just --list

[arg("profile", pattern="debug|release")]
build profile:
    cargo zigbuild --profile {{ if profile == "debug" { "dev" } else { "release" } }} --target "${SFOS_CARGO_TARGET}" --bin {{ app }}

lint-qml:
    qmllint -I qml/modules $(for p in $(echo "$QML_IMPORT_PATH" | tr : " "); do echo -I "$p"; done) $(find qml -path qml/modules -prune -o -name '*.qml' -print | sort)

add-language lang:
    #!/usr/bin/env bash
    set -euo pipefail
    pot="po/{{ app }}.pot"
    po="po/{{ lang }}.po"
    [ -f "$pot" ] || { echo "$pot missing; run 'just build <debug|release>' first" >&2; exit 1; }
    [ ! -e "$po" ] || { echo "$po already exists" >&2; exit 1; }
    msginit --no-translator --locale="{{ lang }}" --input="$pot" --output-file="$po"
    sed -i 's/^"Content-Type: text\/plain; charset=.*\\n"$/"Content-Type: text\/plain; charset=UTF-8\\n"/' "$po"
    grep -q '^"Content-Type: text/plain; charset=UTF-8\\n"$' "$po" || { echo "failed to set UTF-8 charset in $po" >&2; exit 1; }
    echo "Created $po"

# Re-resolve the sysroot packages listed in nix/sysroot.json into nix/sysroot.lock.json
update-sysroot:
    python3 -I nix/sysroot-lock.py

[arg("profile", pattern="debug|release")]
rpm profile: (build profile)
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -n 1)"
    mkdir -p "$root/rpm/build/rpmdb"
    rpm --initdb --dbpath "$root/rpm/build/rpmdb"
    rpmbuild -bb "$root/rpm/app.spec" \
        --target aarch64-linux \
        --dbpath "$root/rpm/build/rpmdb" \
        --define "_dbpath $root/rpm/build/rpmdb" \
        --define "project_root $root" \
        --define "app_name {{ app }}" \
        --define "org_name {{ org }}" \
        --define "ver $version" \
        --define "profile_dir {{ profile }}" \
        --define "_topdir $root/rpm/build" \
        --define "_rpmdir $root/RPMS" \
        --define "_srcrpmdir $root/rpm/build/SRPMS"
    find "$root/RPMS" -mindepth 2 -type f -name '*.rpm' -exec mv -t "$root/RPMS" {} +
    find "$root/RPMS" -mindepth 1 -type d -empty -delete
    ls -l "$root"/RPMS/*.rpm

[arg("profile", pattern="debug|release")]
deploy profile: (rpm profile)
    ./deploy.sh --host defaultuser@100.91.174.16 --rpm RPMS/{{ app }}-0.1.0-1.aarch64.rpm

console *args:
    cargo run -p debug-console --target "$(rustc -vV | sed -n 's/host: //p')" -- {{ args }}
