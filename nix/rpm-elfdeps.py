#!@PYTHON@

import subprocess
import sys

TOOL = "@ELFDEPS@"
PASSTHROUGH = (
    "--soname-only",
    "--no-fake-soname",
    "--no-filter-soname",
    "--require-interp",
)


def main() -> None:
    mode = None
    multifile = False
    extra: list[str] = []
    paths: list[str] = []
    for arg in sys.argv[1:]:
        if arg in ("--provides", "-P"):
            mode = "--provides"
        elif arg in ("--requires", "-R"):
            mode = "--requires"
        elif arg in ("--multifile", "-m"):
            multifile = True
        elif arg in PASSTHROUGH:
            extra.append(arg)
        elif arg.startswith("-"):
            sys.stderr.write(f"elfdeps: unknown option {arg}\n")
            sys.exit(2)
        else:
            paths.append(arg)
    if mode is None:
        sys.stderr.write("elfdeps: --provides or --requires is required\n")
        sys.exit(2)
    if not paths:
        paths = [line.rstrip("\r\n") for line in sys.stdin if line.strip()]

    for path in paths:
        proc = subprocess.run(
            [TOOL, mode, *extra, path],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        lines = [line for line in proc.stdout.splitlines() if line]
        if proc.returncode != 0 or not lines:
            continue
        if multifile:
            print(f";{path}")
        for line in lines:
            print(line)


if __name__ == "__main__":
    main()
