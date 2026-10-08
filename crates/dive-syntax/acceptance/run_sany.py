#!/usr/bin/env python3
"""Throwaway oracle runner: record whether SANY's *parser* accepts each module.

Usage: run_sany.py JAVA JAR OUT.jsonl LIBPATH FILE...

Runs `tla2sany.SANY -s` (semantic analysis off) over the files in batches.
SANY stops at the first fatal error, so after a failure the batch restarts
with the next file. Each file is classified from its own output segment:

  accept         the root module parsed (dependency problems ignored)
  reject         "Could not parse module M from file F" for this file
  name_mismatch  parsed, but file and module names differ
  crash          the JVM died without a verdict for this file
"""
import json
import os
import re
import subprocess
import sys

HEADER = "****** SANY2"


def classify(segment, path):
    base = os.path.basename(path)
    for m in re.finditer(r"Could not parse module (\S+) from file (.+)", segment):
        if m.group(2).strip() in (path, base):
            return "reject"
    if "does not match the name" in segment:
        return "name_mismatch"
    if "Fatal errors" in segment or "Exception" in segment:
        # A fatal error is explained if it concerns another module: a missing
        # dependency, or a dependency that failed to parse. Anything else
        # (for example a Java exception inside SANY's parser) is a crash.
        explained = ("Cannot find source file" in segment
                     or re.search(r"Could not parse module \S+ from file", segment))
        if not explained or "Exception" in segment:
            return "crash"
    return "accept"


def first_error(segment):
    lines = [l for l in segment.splitlines() if l.strip()]
    for i, l in enumerate(lines):
        if "***Parse Error***" in l or "Lexical {error" in l or "Lexical error" in l:
            return " | ".join(lines[i:i + 4])
    for l in lines:
        if "Fatal" in l or "error" in l.lower():
            return l
    return ""


def run_batch(java, jar, lib, files):
    cmd = [java, "-Xss64m", "-DTLA-Library=" + lib, "-cp", jar, "tla2sany.SANY", "-s"] + files
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
        out = p.stdout + p.stderr
    except subprocess.TimeoutExpired as e:
        out = (e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
    segs = out.split(HEADER)[1:]
    return segs


def main():
    java, jar, outp, lib = sys.argv[1:5]
    files = [os.path.abspath(f) for f in sys.argv[5:]]
    results = []
    i = 0
    batch = 40
    while i < len(files):
        chunk = files[i:i + batch]
        segs = run_batch(java, jar, lib, chunk)
        if not segs:
            results.append({"path": chunk[0], "verdict": "crash", "message": ""})
            i += 1
            continue
        done = 0
        for k, seg in enumerate(segs):
            f = chunk[k]
            fatal = "Fatal errors" in seg or "*** Errors" in seg
            v = classify(seg, f)
            if k == len(segs) - 1 and not fatal and len(segs) < len(chunk):
                # Last segment ended without a fatal marker but the run stopped:
                # treat as crash for this file.
                if "Parsing file" not in seg:
                    v = "crash"
            results.append({"path": f, "verdict": v,
                            "message": first_error(seg) if v != "accept" else ""})
            done += 1
        i += done
        sys.stderr.write("%d/%d\n" % (i, len(files)))
    with open(outp, "w") as fh:
        for r in results:
            fh.write(json.dumps(r) + "\n")


if __name__ == "__main__":
    main()
