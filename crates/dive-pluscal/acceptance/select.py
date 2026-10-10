#!/usr/bin/env python3
"""Explicit, reproducible source selection; never writes a pinned checkout."""
import argparse
import hashlib
import json
from pathlib import Path
import re


def sha(data):
    return hashlib.sha256(data).hexdigest()


def end_algorithm(source, start):
    """Find the source terminator outside strings and nested comments.

    This only selects input bytes; the native and Java parsers decide grammar.
    Missing terminators are retained as explicit selection failures.
    """
    position = start
    tokens = []
    braces = 0
    c_style = None
    while position < len(source):
        if source.startswith("\\*", position):
            end = source.find("\n", position)
            position = len(source) if end < 0 else end + 1
            continue
        if source.startswith("(*", position):
            depth = 1
            position += 2
            while depth and position < len(source):
                if source.startswith("(*", position):
                    depth += 1
                    position += 2
                elif source.startswith("*)", position):
                    depth -= 1
                    position += 2
                else:
                    position += 1
            continue
        if source[position] == '"':
            position += 1
            while position < len(source):
                char = source[position]
                position += 1
                if char == "\\":
                    position += 1
                elif char == '"':
                    break
            tokens.append('"string"')
            continue
        match = re.match(r"[A-Za-z_][A-Za-z_0-9]*", source[position:])
        if match:
            token = match.group(0)
            position += len(token)
            tokens.append(token)
            if token == "algorithm" and tokens[-2:-1] == ["end"] and not c_style:
                if source[position:position + 1] == ";":
                    position += 1
                return position
        else:
            char = source[position]
            position += 1
            if char.isspace() or char == "-":
                continue
            tokens.append(char)
            if char == "{" and c_style is None:
                # A C algorithm's first token after its name is '{'.
                header = 3 if tokens[0] == "fair" else 2
                c_style = len(tokens) == header + 1
            if c_style:
                if char == "{":
                    braces += 1
                elif char == "}":
                    braces -= 1
                    if braces == 0:
                        return position
    return None


def select(cache, output):
    output.mkdir(parents=True, exist_ok=True)
    selection = []
    inputs = output / "inputs"
    inputs.mkdir(exist_ok=True)
    corpora = {
        "examples": "6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f",
        "tlaplus-tests": "94d0c5070852b37b801bb8998c436383ab7ee37b",
    }
    marker = re.compile(r"--(?:algorithm\b|fair\s+algorithm\b)")
    for corpus, pin in corpora.items():
        checkout = cache / "git" / corpus / pin
        for path in sorted(checkout.rglob("*.tla")):
            raw = path.read_bytes()
            try:
                source = raw.decode("utf-8")
            except UnicodeDecodeError:
                continue  # Corpus-wide non-UTF8 files contain no selected marker.
            for number, match in enumerate(marker.finditer(source)):
                end = end_algorithm(source, match.start())
                ident = f"{corpus}-{len(selection):04d}"
                row = {"id": ident, "corpus": corpus, "commit": pin,
                       "path": path.relative_to(checkout).as_posix(), "source_sha256": sha(raw),
                       "marker_ordinal": number, "start": len(source[:match.start()].encode())}
                if end is None:
                    row.update(status="selection-error", reason="no standalone algorithm terminator")
                else:
                    data = source[match.start():end].encode()
                    target = inputs / (ident + ".pcal")
                    target.write_bytes(data)
                    row.update(status="selected", end=len(source[:end].encode()),
                               input=target.relative_to(output).as_posix(), input_sha256=sha(data))
                selection.append(row)
    (output / "selection.json").write_text(json.dumps({"schema_version": 1, "rows": selection}, indent=2) + "\n")
    return selection


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    select(arguments.cache, arguments.output)
