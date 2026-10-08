#!/usr/bin/env python3
"""Oracle runner: record whether the pinned SANY accepts each module, with
semantic analysis and level checking on (`-error-codes`).

Usage: run_sany.py JAVA CLASSPATH OUT.jsonl LIST [JOBS]

LIST holds one absolute `.tla` path per line. Files are grouped by
directory, and SANY runs with that directory as its working directory and
the bare file names as arguments, exactly as the conformance harness runs it
(`crates/dive-conformance/src/oracle.rs`). CLASSPATH is the harness
classpath: `tla2tools.jar`, the CommunityModules jars, then the module
library directories.

With `-error-codes`, SANY exits non-zero at the first module it rejects, so
the batch restarts after that file. Each file is classified from its own
output segment (SANY prints a `****** SANY2` header per file):

  accept   no error marker; warnings alone do not reject
  reject   a parse error, a semantic or level error, or a fatal error

The record keeps the exit code, the error kind and the error text, from
which `sany_codes.py` recovers SANY's error codes.
"""
import concurrent.futures
import json
import os
import subprocess
import sys

HEADER = "****** SANY2"
PARSE_MARKERS = ("***Parse Error***", "Lexical {error", "Lexical error",
                 "Could not parse module", "Fatal errors while parsing")
SEMANTIC_MARKERS = ("*** Errors:", "Semantic errors:")
ABORT_MARKERS = ("Fatal errors in semantic processing", "*** Abort messages:")


def classify(segment):
    """(verdict, kind) for one file's output segment."""
    if any(m in segment for m in PARSE_MARKERS):
        return "reject", "parse"
    if "Cannot find source file" in segment or "Circular dependency" in segment:
        return "reject", "parse"
    if "*** Errors:" in segment:
        return "reject", "semantic"
    if any(m in segment for m in ABORT_MARKERS):
        return "reject", "abort"
    return "accept", None


def error_text(segment):
    for marker in ("Semantic errors:", "*** Errors:", "***Parse Error***",
                   "Fatal errors", "Cannot find source file", "Circular dependency",
                   "Could not parse module", "Lexical"):
        i = segment.find(marker)
        if i >= 0:
            return segment[i:i + 4000]
    return segment[-2000:]


def run_dir(java, cp, directory, names):
    out = []
    i = 0
    while i < len(names):
        chunk = names[i:i + 30]
        cmd = [java, "-XX:+UseParallelGC", "-Xmx1g", "-Xss8m",
               "-Duser.language=en", "-Duser.country=US", "-Dfile.encoding=UTF-8",
               "-cp", cp, "tla2sany.SANY", "-error-codes"] + chunk
        try:
            p = subprocess.run(cmd, cwd=directory, capture_output=True, text=True,
                               timeout=900, errors="replace")
            text, code = p.stdout + p.stderr, p.returncode
        except subprocess.TimeoutExpired:
            text, code = "", None
        segs = text.split(HEADER)[1:]
        if not segs:
            out.append({"path": os.path.join(directory, chunk[0]), "verdict": "crash",
                        "exit_code": code, "kind": None, "error": text[-2000:]})
            i += 1
            continue
        done = 0
        for k, seg in enumerate(segs):
            path = os.path.join(directory, chunk[k])
            verdict, kind = classify(seg)
            last = k == len(segs) - 1
            if last and code not in (0, None) and verdict == "accept":
                # The run stopped on this file without a recognised marker.
                verdict, kind = "reject", "exit-%s" % code
            if last and code is None:
                verdict, kind = "timeout", None
            out.append({"path": path, "verdict": verdict, "exit_code": code if last else 0,
                        "kind": kind,
                        "error": error_text(seg) if verdict != "accept" else ""})
            done += 1
        i += done
    return out


def main():
    java, cp, outp, listf = sys.argv[1:5]
    jobs = int(sys.argv[5]) if len(sys.argv) > 5 else 2
    files = [l.strip() for l in open(listf) if l.strip()]
    by_dir = {}
    for f in files:
        by_dir.setdefault(os.path.dirname(f), []).append(os.path.basename(f))
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as ex:
        futs = [ex.submit(run_dir, java, cp, d, ns) for d, ns in sorted(by_dir.items())]
        for n, fut in enumerate(concurrent.futures.as_completed(futs)):
            results.extend(fut.result())
            sys.stderr.write("dirs %d/%d\n" % (n + 1, len(futs)))
    results.sort(key=lambda r: r["path"])
    with open(outp, "w") as fh:
        for r in results:
            fh.write(json.dumps(r) + "\n")


if __name__ == "__main__":
    main()
