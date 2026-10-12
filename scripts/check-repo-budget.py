#!/usr/bin/env python3
"""Repository budget guard (ADR 0086 evidence policy).

Fails when the tracked tree:
  * contains a file larger than 1 MB (except under conformance/golden/ and the
    explicit SIZE_ALLOWLIST);
  * tracks more than 30,000 files;
  * tracks a path that looks like a snapshot or run dump: */source/crates/*,
    a path component starting with before- or uncommitted-, *.gz, *.stdout or
    *.stderr (pinned-oracle output fixtures under crates/*/tests/fixtures/ and
    the documented example observations are exempt from the last two).

By default it checks the index (what the next commit will contain), so it is
suitable for the pre-commit hook. `--rev REV` checks a commit instead. Run with
`python3 -I scripts/check-repo-budget.py`. Standard library only.
"""

from __future__ import annotations

import argparse
import fnmatch
import subprocess
import sys

MAX_FILE_BYTES = 1_000_000
MAX_FILES = 30_000

# Paths (exact) allowed to exceed MAX_FILE_BYTES. Keep this list short and
# justify every entry in the commit that adds it.
SIZE_ALLOWLIST: frozenset[str] = frozenset()
SIZE_EXEMPT_PREFIXES = ("conformance/golden/",)

SNAPSHOT_GLOBS = ("*/source/crates/*", "*.gz", "*.stdout", "*.stderr")
SNAPSHOT_COMPONENT_PREFIXES = ("before-", "uncommitted-")
# Pinned-oracle outputs that tests read as expected data, and the
# observations behind docs/user examples.
OUTPUT_FIXTURE_GLOBS = (
    "crates/*/tests/fixtures/*",
    "docs/user/examples/*-observations/*",
)


def git(*args: str) -> bytes:
    """Runs git and returns stdout, exiting with a clear message on failure."""
    result = subprocess.run(["git", *args], capture_output=True, check=False)
    if result.returncode != 0:
        sys.stderr.write(result.stderr.decode(errors="replace"))
        sys.exit(f"check-repo-budget: git {' '.join(args)} failed")
    return result.stdout


def tracked_from_rev(rev: str) -> list[tuple[str, int]]:
    """Returns (path, size) for every blob in REV."""
    out = []
    for line in git("ls-tree", "-r", "-l", "-z", rev).split(b"\0"):
        if not line:
            continue
        meta, path = line.split(b"\t", 1)
        kind, size = meta.split()[1], meta.split()[3]
        if kind != b"blob":
            continue
        out.append((path.decode(errors="replace"), int(size)))
    return out


def tracked_from_index() -> list[tuple[str, int]]:
    """Returns (path, size) for every staged blob."""
    entries = []
    for line in git("ls-files", "--stage", "-z").split(b"\0"):
        if not line:
            continue
        meta, path = line.split(b"\t", 1)
        mode, obj = meta.split()[0], meta.split()[1]
        if mode == b"160000":  # submodule
            continue
        entries.append((path.decode(errors="replace"), obj.decode()))
    if not entries:
        return []
    batch = subprocess.run(
        ["git", "cat-file", "--batch-check=%(objectsize)"],
        input="".join(obj + "\n" for _, obj in entries).encode(),
        capture_output=True,
        check=False,
    )
    if batch.returncode != 0:
        sys.exit("check-repo-budget: git cat-file failed")
    sizes = batch.stdout.decode().split()
    if len(sizes) != len(entries):
        sys.exit("check-repo-budget: unexpected git cat-file output")
    return [(path, int(size)) for (path, _), size in zip(entries, sizes)]


def snapshot_violation(path: str) -> str | None:
    """Names the snapshot pattern PATH matches, if any."""
    for component in path.split("/"):
        for prefix in SNAPSHOT_COMPONENT_PREFIXES:
            if component.startswith(prefix):
                return f"path component '{component}' starts with '{prefix}'"
    for pattern in SNAPSHOT_GLOBS:
        if fnmatch.fnmatchcase(path, pattern):
            if pattern in ("*.stdout", "*.stderr") and any(
                fnmatch.fnmatchcase(path, g) for g in OUTPUT_FIXTURE_GLOBS
            ):
                continue
            return f"matches '{pattern}'"
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--rev", help="check this commit instead of the index")
    parser.add_argument("--quiet", action="store_true", help="print only failures")
    args = parser.parse_args()

    files = tracked_from_rev(args.rev) if args.rev else tracked_from_index()
    problems = []
    if len(files) > MAX_FILES:
        problems.append(f"{len(files)} tracked files exceeds the budget of {MAX_FILES}")
    for path, size in files:
        if (
            size > MAX_FILE_BYTES
            and path not in SIZE_ALLOWLIST
            and not path.startswith(SIZE_EXEMPT_PREFIXES)
        ):
            problems.append(f"{path}: {size} bytes exceeds {MAX_FILE_BYTES}")
        reason = snapshot_violation(path)
        if reason:
            problems.append(f"{path}: snapshot or run-output path ({reason})")

    total = sum(size for _, size in files)
    if problems:
        shown = problems[:50]
        sys.stderr.write("check-repo-budget: the evidence policy (ADR 0086) is violated:\n")
        sys.stderr.write("".join(f"  {p}\n" for p in shown))
        if len(problems) > len(shown):
            sys.stderr.write(f"  ... and {len(problems) - len(shown)} more\n")
        return 1
    if not args.quiet:
        print(f"check-repo-budget: ok ({len(files)} files, {total} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
