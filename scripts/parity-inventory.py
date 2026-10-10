#!/usr/bin/env python3
"""Reproduce the pinned parity surface inventory; never infer conformance."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

TLC_COMMIT = "94d0c5070852b37b801bb8998c436383ab7ee37b"
COMMUNITY_COMMIT = "78b98ae9939d2fe7a6615e9a4b21dd6c23ef8ff3"
ROOT = Path(__file__).resolve().parents[1]


def fingerprint(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source(path, root):
    return {"path": path.relative_to(root).as_posix(), "sha256": fingerprint(path)}


def line_number(text, offset):
    return text.count("\n", 0, offset) + 1


def without_comments(text):
    """Preserve Java strings and line offsets while hiding comments."""
    pattern = r'"(?:[^"\\]|\\.)*"|\'(?:[^\'\\]|\\.)*\'|//[^\n]*|/\*[\s\S]*?\*/'

    def hide(match):
        value = match.group()
        if value.startswith("//") or value.startswith("/*"):
            return "".join("\n" if char == "\n" else " " for char in value)
        return value

    return re.sub(pattern, hide, text)


def verify_checkout(path, commit):
    actual = subprocess.check_output(
        ["git", "-C", str(path), "rev-parse", "HEAD"], text=True
    ).strip()
    if actual != commit:
        raise ValueError(f"checkout revision {actual} differs from required {commit}")
    dirty = subprocess.check_output(
        ["git", "-C", str(path), "status", "--porcelain", "--untracked-files=all"],
        text=True,
    )
    if dirty:
        raise ValueError("pinned source checkout is not clean")


def flags(path, root, pattern):
    text = without_comments(path.read_text())
    found = {}
    for match in re.finditer(pattern, text):
        name = match.group(1)
        if name.startswith("-"):
            found.setdefault(name, []).append(line_number(text, match.start()))
    return {
        "source": source(path, root),
        "rows": [{"name": name, "lines": lines} for name, lines in sorted(found.items())],
    }


def configuration(path, root):
    text = without_comments(path.read_text())
    declarations = dict(re.findall(
        r'(?:private|public)\s+static\s+final\s+String\s+(\w+)\s*=\s*([^;]+);', text
    ))
    names = re.search(r'ALL_KEYWORDS\s*=\s*\{([^}]+)\}', text).group(1)
    names = re.findall(r'\w+', names)

    def resolve(name):
        value = declarations[name].strip()
        if value.startswith('"'):
            return json.loads(value)
        if value.startswith("TLAConstants.KeyWords."):
            return value.split(".")[-1]
        match = re.fullmatch(r"(\w+)\s*\+\s*'S'", value)
        if match:
            return resolve(match.group(1)) + "S"
        raise ValueError(f"unhandled config keyword initializer {value}")

    return {
        "source": source(path, root),
        "rows": [{"name": resolve(name), "symbol": name} for name in names],
    }


def builtins(path, root):
    text = without_comments(path.read_text())
    rows = []
    for match in re.finditer(
        r'new BuiltInOperator\(("(?:[^"\\]|\\.)*"),\s*(-?\d+),\s*(\d+),', text
    ):
        rows.append({"name": json.loads(match.group(1)), "arity": int(match.group(2)),
                     "level": int(match.group(3)), "line": line_number(text, match.start())})
    return {"source": source(path, root), "rows": rows}


def overrides():
    path = ROOT / "crates/dive-ir/src/std_op.rs"
    text = path.read_text()
    pattern = (
        r'(\w+)\s*=\s*\(("(?:[^"\\]|\\.)*"),\s*("(?:[^"\\]|\\.)*"),'
        r'\s*(-?\d+),\s*(\w+),\s*(\w+),\s*(\w+),\s*(true|false),'
        r'\s*("(?:[^"\\]|\\.)*")\);'
    )
    rows = []
    for match in re.finditer(pattern, text):
        variant, module, name, arity, kind, phase, support, impure, java = match.groups()
        rows.append({"variant": variant, "module": json.loads(module),
                     "name": json.loads(name), "arity": int(arity), "kind": kind,
                     "installation_phase": phase, "registry_support": support,
                     "impure": impure == "true", "java_methods": json.loads(java),
                     "line": line_number(text, match.start()),
                     "certification": "audit-required"})
    return {"source": source(path, ROOT), "rows": rows}


def modules(path, root):
    return {"rows": [source(p, root) for p in sorted(path.glob("*.tla"))]}


def inventory(tlc, community):
    verify_checkout(tlc, TLC_COMMIT)
    verify_checkout(community, COMMUNITY_COMMIT)
    src = tlc / "tlatools/org.lamport.tlatools/src"
    data = {
        "schema_version": 1,
        "tlc_commit": TLC_COMMIT,
        "community_commit": COMMUNITY_COMMIT,
        "scope": "literal source/registry surfaces, not certified semantic features",
        "tlc_cli": flags(src / "tlc2/TLC.java", tlc,
                         r'args\[index\]\.equals(?:IgnoreCase)?\("([^"\n]+)"\)'),
        "pluscal_cli": flags(src / "pcal/trans.java", tlc,
                             r'option\.equals\("([^"\n]+)"\)'),
        "sany_cli": flags(src / "tla2sany/drivers/SANY.java", tlc,
                          r'args\[i\](?:\.toLowerCase\(\))?\.equals\("([^"\n]+)"\)'),
        "configuration": configuration(src / "tlc2/tool/impl/ModelConfig.java", tlc),
        "builtins": builtins(src / "tla2sany/semantic/BuiltInOperators.java", tlc),
        "override_registry": overrides(),
        "standard_modules": modules(src / "tla2sany/StandardModules", tlc),
        "community_modules": modules(community / "modules", community),
    }
    # Unexpected parser/source changes must be reviewed, not silently reduce the inventory.
    expected = {"tlc_cli": 46, "pluscal_cli": 21, "sany_cli": 14, "configuration": 22,
                "builtins": 72, "override_registry": 158,
                "standard_modules": 20, "community_modules": 34}
    actual = {name: len(data[name]["rows"]) for name in expected}
    if actual != expected:
        raise ValueError(f"review surface census before changing expected counts: {actual}")
    data["counts"] = actual
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tlc-source", required=True, type=Path)
    parser.add_argument("--community-source", required=True, type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    data = inventory(args.tlc_source, args.community_source)
    encoded = json.dumps(data, indent=2, ensure_ascii=False) + "\n"
    output = ROOT / "conformance/parity-surfaces.json"
    if args.check:
        if output.read_text() != encoded:
            raise SystemExit("parity surface inventory is stale")
    else:
        output.write_text(encoded)
    print(json.dumps(data["counts"], sort_keys=True))


if __name__ == "__main__":
    main()
