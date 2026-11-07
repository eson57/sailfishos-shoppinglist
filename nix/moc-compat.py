#!/usr/bin/env python3

"""
LLM written blob that does something to rewrite cpp to be understood by moc tool.

I have no idea what moc is or how it works.
"""


import os
import subprocess
import sys
import tempfile

from tree_sitter_language_pack import get_parser


def rewrite(src: bytes) -> bytes:
    """Split `namespace a::b::c { ... }` into nested namespace definitions.

    Only the namespace name and the closing brace are edited, so every other
    byte (and therefore every line number moc reports) is left untouched.
    """
    tree = get_parser("cpp").parse(src)
    edits = []  # (offset, bytes to delete, bytes to insert)

    def walk(node):
        if node.type == "namespace_definition":
            name = node.child_by_field_name("name")
            body = node.child_by_field_name("body")
            if name is not None and name.type == "nested_namespace_specifier" and body is not None:
                parts = [p.strip() for p in src[name.start_byte:name.end_byte].decode().split("::")]
                edits.append((name.start_byte, name.end_byte - name.start_byte,
                              " { namespace ".join(parts).encode()))
                edits.append((body.end_byte, 0, b"}" * (len(parts) - 1)))
        for child in node.children:
            walk(child)

    walk(tree.root_node)
    for offset, length, insert in sorted(edits, key=lambda e: e[0], reverse=True):
        src = src[:offset] + insert + src[offset + length:]
    return src


def main():
    real_moc, qt_prefix, *args = sys.argv[1:]

    passed = []
    input_file = None
    it = iter(args)
    for a in it:
        if a == "--output-json":
            continue
        elif a.startswith("-I"):
            d = a[2:] or next(it, "")
            if d.startswith(qt_prefix):
                passed.append("-I" + d)
        elif a == "-o":
            passed += [a, next(it, "")]
        elif a.startswith("-"):
            passed.append(a)
        else:
            input_file = a

    if input_file is None:
        os.execv(real_moc, [real_moc] + passed)

    try:
        with open(input_file, "rb") as f:
            src = f.read()
    except OSError as e:
        sys.exit(f"moc-compat: cannot read {input_file}: {e}")

    with tempfile.TemporaryDirectory() as d:
        tmp = os.path.join(d, os.path.basename(input_file))
        with open(tmp, "wb") as f:
            f.write(rewrite(src))
        r = subprocess.run([real_moc] + passed + ["-f", os.path.abspath(input_file), tmp])
    sys.exit(r.returncode if r.returncode >= 0 else 127)


if __name__ == "__main__":
    main()
