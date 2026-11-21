"""Recreate three small counting diagnostics from the released Flint module."""

import argparse
import hashlib
import re
from pathlib import Path

SOURCE = "edaa0c49822fa3272127ffee2e1358e42c7b5137a0f86e97a1a9b1fd424d6211"
ACTIONS = (
    "GPut GCas GDelete GRename GRenameFinish Checkout Consume Scan Skip PullOnly "
    "Claim Verify Install Collect Finish Restart Sync RescopeBegin RescopeFirst "
    "RescopeSecond RPullRead RPullSync Edit Delete Upload Sweep Age RLoad Reap"
).split()


def modified_modules(source):
    """Build the equivalent guard, the action instrumentation and a two-state witness."""
    old = r'LeaseGuard == GatewayIgnoresLease \/ holder = "none"'
    new = 'LeaseGuard == IF GatewayIgnoresLease THEN TRUE ELSE holder = "none"'
    if source.count(old) != 1:
        raise ValueError("expected exactly one released LeaseGuard")
    disjoint = source.replace("MODULE LeanP1 ", "MODULE DisjointFlint ", 1).replace(old, new)
    marked = source.replace("MODULE LeanP1 ", "MODULE MarkedFlint ", 1).replace(
        "EXTENDS Naturals, FiniteSets", "EXTENDS Naturals, FiniteSets, TLC", 1
    )
    start = marked.index("GatewayStep ==")
    end = marked.index("Spec == Init", start)
    selection = marked[start:end]
    wrappers = []
    for name in ACTIONS:
        definition = re.search(rf"^{name}(\([^)]*\))?\s*==", marked[:start], re.MULTILINE)
        if definition is None:
            raise ValueError(f"missing definition for {name}")
        signature = name + (definition[1] or "")
        wrappers.append(
            f"Count{signature} == /\\ {signature}\n    /\\ PrintT(\"FLINTACTION_{name}\")\n"
        )
        selection, matches = re.subn(rf"\b{name}\b", "Count" + name, selection)
        if matches != 1:
            raise ValueError(f"expected a single action reference to {name}")
    minimal = """---- MODULE DuplicateGuard ----
CONSTANT Open
VARIABLE x
Init == x = 0
Allowed == Open \\/ x = 0
Next == /\\ x = 0 /\\ Allowed /\\ x' = 1
===="""
    return {
        "MarkedFlint.tla": marked[:start] + "\n".join(wrappers) + "\n" + selection + marked[end:],
        "DisjointFlint.tla": disjoint,
        "DuplicateGuard.tla": minimal,
        "tiny.cfg": "CONSTANT Open = TRUE\nINIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("released_module", type=Path)
    parser.add_argument("new_directory", type=Path)
    args = parser.parse_args()
    raw = args.released_module.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SOURCE:
        parser.error("expected the pinned LeanP1.tla from the Flint evidence record")
    modules = modified_modules(raw.decode("utf-8"))
    args.new_directory.mkdir()
    for name, content in modules.items():
        (args.new_directory / name).write_bytes(content.encode("utf-8"))


if __name__ == "__main__":
    main()
