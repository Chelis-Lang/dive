#!/usr/bin/env python3
"""Check selected role Git identity without .git; four tamper controls."""
from pathlib import Path
import gzip
import json
import runpy
import shutil
import tempfile

ROOT = Path(__file__).resolve().parent.parent
PROBE = ROOT / "conformance/probes/disk-safety-roles/implemented-epoch"


def main():
    reader = runpy.run_path(str(ROOT / "scripts/check-disk-safety-roles.py"))
    check = reader["original_git_blobs"]
    assert len(check(PROBE)) == 1347
    with tempfile.TemporaryDirectory(prefix="dive-role-proof-") as temporary:
        base = Path(temporary)
        shutil.copytree(PROBE / "producer-git-objects", base / "producer-git-objects")
        assert not (base / ".git").exists()
        assert len(check(base)) == 1347
        index = base / "producer-git-objects/index.json"
        original_index = index.read_bytes()
        proof = json.loads(original_index)
        tree = base / "producer-git-objects/objects" / (proof["tree"] + ".gz")
        original_tree = tree.read_bytes()
        mutations = [
            ("missing tree", tree.unlink),
            ("corrupt tree", lambda: tree.write_bytes(gzip.compress(b"wrong tree", mtime=0))),
            ("retagged producer", lambda: index.write_bytes(original_index.replace(proof["producer"].encode(), b"0" * 40))),
            ("altered blob claim", lambda: index.write_bytes(original_index.replace(next(iter(proof["blob_oid"].values())).encode(), b"1" * 40))),
        ]
        for name, change in mutations:
            change()
            try:
                check(base)
            except (AssertionError, KeyError, FileNotFoundError):
                print("Rejected " + name)
            else:
                raise AssertionError("Accepted " + name)
            finally:
                tree.write_bytes(original_tree)
                index.write_bytes(original_index)
        print("Verified all 1,347 original selected blob identities without .git")


if __name__ == "__main__":
    main()
