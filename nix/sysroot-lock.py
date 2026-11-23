#!/usr/bin/env python3
"""Resolve the Sailfish OS sysroot package set and write nix/sysroot.lock.json.

Reads the root packages from nix/sysroot.json, downloads the repository
metadata for that release, solves for the root packages with libsolv's
command line tools (libsolv is the solver zypper uses) and records each
package's URL and sha256 so Nix can fetch exactly those RPMs.
"""

import gzip
import json
import subprocess
import sys
import tempfile
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

NS = {
    "repo": "http://linux.duke.edu/metadata/repo",
    "common": "http://linux.duke.edu/metadata/common",
}

HERE = Path(__file__).resolve().parent
CONFIG = HERE / "sysroot.json"
LOCK = HERE / "sysroot.lock.json"


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url) as response:
        return response.read()


def primary_xml(base: str) -> bytes:
    repomd = ET.fromstring(fetch(f"{base}/repodata/repomd.xml"))
    for data in repomd.findall("repo:data", NS):
        if data.get("type") == "primary":
            href = data.find("repo:location", NS).get("href")
            primary = fetch(f"{base}/{href}")
            return gzip.decompress(primary) if href.endswith(".gz") else primary
    raise SystemExit(f"{base}: no primary metadata")


def packages(base: str, primary: bytes) -> dict[str, dict]:
    """Map each package's name-[epoch:]version-release.arch, as testsolv
    prints it, to its download URL and sha256."""
    result = {}
    for pkg in ET.fromstring(primary).findall("common:package", NS):
        name = pkg.findtext("common:name", namespaces=NS)
        arch = pkg.findtext("common:arch", namespaces=NS)
        version = pkg.find("common:version", NS)
        epoch = version.get("epoch", "0")
        evr = f"{'' if epoch == '0' else epoch + ':'}{version.get('ver')}-{version.get('rel')}"
        checksum = pkg.find("common:checksum", NS)
        if checksum.get("type") != "sha256":
            raise SystemExit(f"{base}: unsupported checksum type {checksum.get('type')}")
        result[f"{name}-{evr}.{arch}"] = {
            "name": name,
            "version": evr,
            "url": f"{base}/{pkg.find('common:location', NS).get('href')}",
            "sha256": checksum.text,
            "size": int(pkg.find("common:size", NS).get("package")),
        }
    return result


def main() -> None:
    config = json.loads(CONFIG.read_text())
    release, arch = config["release"], config["arch"]

    available: dict[str, dict] = {}
    with tempfile.TemporaryDirectory() as tmp:
        testcase = []
        for index, repo in enumerate(config["repositories"]):
            base = repo.replace("@RELEASE@", release).replace("@ARCH@", arch)
            print(f"Reading {base}", file=sys.stderr)
            primary = primary_xml(base)
            for key, pkg in packages(base, primary).items():
                # Earlier repositories win if the same build is in several.
                available.setdefault(key, pkg)

            solv = Path(tmp) / f"repo{index}.solv"
            with solv.open("wb") as out:
                subprocess.run(["rpmmd2solv"], input=primary, stdout=out, check=True)
            # testsolv resolves repo files relative to the testcase's directory.
            testcase.append(f"repo repo{index} 0 solv {solv.name}")

        testcase += [
            f"system {arch} rpm",
            # A sysroot only needs hard dependencies.
            "solverflags ignorerecommended",
            *(f"job install name {name}" for name in config["packages"]),
        ]
        testcase_file = Path(tmp) / "sysroot.t"
        testcase_file.write_text("\n".join(testcase) + "\n")
        result = subprocess.run(
            ["testsolv", str(testcase_file)], capture_output=True, text=True, check=True
        )
        solved = result.stdout

    # testsolv exits successfully even when the jobs can't be solved, or when
    # it can't read a repository (which it only reports on stderr).
    if result.stderr or "problems:" in solved or "installed packages:" not in solved:
        raise SystemExit(result.stderr + solved)
    selected = [
        available[line.removeprefix("  - ")]
        for line in solved.splitlines()
        if line.startswith("  - ")
    ]
    selected.sort(key=lambda pkg: pkg["name"])

    lock = {
        "release": release,
        "arch": arch,
        "packages": [
            {key: pkg[key] for key in ("name", "version", "url", "sha256")}
            for pkg in selected
        ],
    }
    LOCK.write_text(json.dumps(lock, indent=2) + "\n")
    size = sum(pkg["size"] for pkg in selected)
    print(
        f"Wrote {LOCK.name} with {len(selected)} packages ({size / 2**20:.0f} MiB)",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
