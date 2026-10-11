#!/usr/bin/env python3
"""Check portable original Git-object identity without a repository or network."""
from pathlib import Path
import gzip
import runpy
import shutil
import tempfile

ROOT = Path(__file__).resolve().parent.parent
PROBE = ROOT / "conformance/probes/fallible-seen-engine"
PRODUCER = "da0f857161c57b52e8cb1d6c18ef0cebb0021ea4"


def main():
    reader = runpy.run_path(str(ROOT / "scripts/check-fallible-seen-engine.py"))
    check = reader["original_git_blobs"]
    original = check.__globals__["PROBE"]
    try:
        with tempfile.TemporaryDirectory(prefix="dive-engine-git-proof-") as temporary:
            copied = Path(temporary) / "probe"
            shutil.copytree(PROBE / "producer-git-objects", copied / "producer-git-objects")
            check.__globals__["PROBE"] = copied
            assert not (Path(temporary) / ".git").exists()
            blobs = check(PRODUCER)
            assert len(blobs) == 470
            proof = reader["load"](copied / "producer-git-objects/index.json")
            tree = copied / "producer-git-objects/objects" / (proof["tree"] + ".gz")
            packed = tree.read_bytes()
            index = copied / "producer-git-objects/index.json"
            index_bytes = index.read_bytes()

            def rejected(label, operation):
                operation()
                try:
                    check(PRODUCER)
                except (AssertionError, KeyError, FileNotFoundError):
                    print("Rejected " + label)
                else:
                    raise AssertionError("Accepted " + label)
                finally:
                    tree.write_bytes(packed)
                    index.write_bytes(index_bytes)

            rejected("missing original tree", tree.unlink)
            rejected("corrupt original tree", lambda: tree.write_bytes(gzip.compress(b"wrong tree", mtime=0)))
            rejected("retagged producer index", lambda: index.write_bytes(index_bytes.replace(PRODUCER.encode(), b"0" * 40)))
            rejected("altered path-to-blob claim", lambda: index.write_bytes(index_bytes.replace(next(iter(blobs.values())).encode(), b"1" * 40, 1)))
            print("Verified all470 original path/blob identities without .git; four negative controls rejected.")
    finally:
        check.__globals__["PROBE"] = original


if __name__ == "__main__":
    main()
