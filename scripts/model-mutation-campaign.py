#!/usr/bin/env python3
"""Run a seeded model-mutation campaign through the pinned conformance harness.

Behavioural mutants of pinned Examples models are model-checked by TLC (the
oracle) and by dive, and the harness compares their observable results. No
mutant needs a hand-written expectation: TLC's result is the expectation, and
an equivalent mutant is still a valid comparison. Only disposable copies are
mutated; goldens and reports live under --out, never in the checkout.
"""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import random
import re
import shutil
import subprocess
import sys


CORPUS = "model-mutation"
HEADER = re.compile(r"^-{4,}\s*MODULE\s+([A-Za-z_][A-Za-z0-9_]*)\b", re.M)
USES = re.compile(r"\b(?:EXTENDS|INSTANCE)\b([^\n]*)")
NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
# The `![k].f` path of an EXCEPT update, whose `=` is syntax, not equality.
EXCEPT_PATH = re.compile(r"!(?:\[[^\]]*\]|\.[A-Za-z_][A-Za-z0-9_]*)+\s*$")
CLASSES = ["boundary", "equality", "constant", "arithmetic", "logic", "boolean",
           "drop-conjunct", "drop-disjunct"]
MAX_RUNTIME = "00:00:05"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load_script(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def source_pin(repo):
    blocks = (repo / "conformance/corpora.toml").read_text().split("[[corpus]]")
    blocks = [b for b in blocks if re.search(r'^id = "examples"$', b, re.M)]
    if len(blocks) != 1:
        raise ValueError("expected one Examples pin")
    match = re.search(r'^commit = "([0-9a-f]{40})"', blocks[0], re.M)
    if match is None:
        raise ValueError("missing Examples commit")
    return match.group(1)


def verify_source(root, pin):
    def git(*args):
        return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()
    if git("rev-parse", "HEAD") != pin:
        raise ValueError("Examples source is not at the pinned commit")
    if git("status", "--porcelain", "--untracked-files=all"):
        raise ValueError("Examples source is not pristine")


# ---------------------------------------------------------------------------
# Mutation sites. Every edit keeps each line's length where it can, because
# conjunction and disjunction lists are aligned by column; the drop edits
# replace whole list items and leave every other line's columns alone.


def standalone(src, mask, start, end, before, after):
    """`src[start:end]` is code and not part of a longer operator."""
    if not all(mask[start:end]):
        return False
    prev = src[start - 1] if start > 0 else " "
    nxt = src[end] if end < len(src) else " "
    return prev not in before and nxt not in after


def operator_sites(src, mask, lo, hi):
    sites = []
    ops = "<>=#/\\|-+~*:.!@&^$%"

    def add(cls, i, j, new):
        # A length change is safe only where nothing follows on the line.
        line_end = src.find("\n", j)
        rest = src[j:line_end if line_end >= 0 else len(src)]
        if (len(new) == j - i or not rest.strip()) and src[i:j] != new:
            sites.append((cls, i, j, new))

    for m in re.finditer(r"<=|=<|>=|<|>|=|#|\+|-", src[lo:hi]):
        i, j = lo + m.start(), lo + m.end()
        tok = m.group(0)
        if not standalone(src, mask, i, j, ops, ops):
            continue
        nxt = src[j] if j < len(src) else ""
        if tok == "<" and nxt == " ":
            add("boundary", i, j + 1, "<=")
        elif tok == ">" and nxt == " ":
            add("boundary", i, j + 1, ">=")
        elif tok in ("<=", "=<"):
            add("boundary", i, j, "< ")
        elif tok == ">=":
            add("boundary", i, j, "> ")
        elif tok == "=" and not EXCEPT_PATH.search(src[src.rfind("\n", 0, i) + 1:i]):
            add("equality", i, j, "#")
        elif tok == "#":
            add("equality", i, j, "=")
        elif tok == "+":
            add("arithmetic", i, j, "-")
        elif tok == "-" and src[i - 1] == " " and nxt == " ":
            add("arithmetic", i, j, "+")
    for m in re.finditer(r"(?<![A-Za-z0-9_.])\d+(?![A-Za-z0-9_])", src[lo:hi]):
        i, j = lo + m.start(), lo + m.end()
        if not all(mask[i:j]):
            continue
        n = int(m.group(0))
        for v in (n + 1, n - 1):
            if v >= 0 and len(str(v)) == j - i:
                add("constant", i, j, str(v))
                break
    for m in re.finditer(r"/\\|\\/", src[lo:hi]):
        i, j = lo + m.start(), lo + m.end()
        line_start = src.rfind("\n", 0, i) + 1
        if all(mask[i:j]) and src[line_start:i].strip():
            add("logic", i, j, "\\/" if m.group(0) == "/\\" else "/\\")
    for m in re.finditer(r"\b(TRUE|FALSE)\b", src[lo:hi]):
        i, j = lo + m.start(), lo + m.end()
        if not all(mask[i:j]):
            continue
        if m.group(0) == "TRUE":
            if src[j:j + 1] == " ":
                add("boolean", i, j + 1, "FALSE")
            else:
                add("boolean", i, j, "FALSE")
        else:
            add("boolean", i, j, "TRUE ")
    return sites


def drop_sites(src, mask, lo, hi):
    """Replace one bulleted item of a list with two or more items by its unit."""
    lines, offset = [], 0
    for line in src.splitlines(keepends=True):
        lines.append((offset, line))
        offset += len(line)
    sites = []
    for k, (start, line) in enumerate(lines):
        if start < lo or start >= hi:
            continue
        body = line.lstrip(" ")
        col = len(line) - len(body)
        for bullet, cls, unit in (("/\\", "drop-conjunct", "TRUE"), ("\\/", "drop-disjunct", "FALSE")):
            if not body.startswith(bullet) or not all(mask[start + col:start + col + 2]):
                continue
            end = k + 1
            while end < len(lines):
                text = lines[end][1]
                stripped = text.lstrip(" ")
                if stripped.strip() and len(text) - len(stripped) <= col:
                    break
                end += 1
            # The item ends where a line starts at or left of its bullet.
            item_end = lines[end - 1][0] + len(lines[end - 1][1])
            # Items of the same list have their bullet in the same column,
            # the first often after the definition's `==` on its own line.
            siblings = [l for _, l in lines[max(0, k - 40):min(len(lines), k + 40)]
                        if l[col:col + 2] == bullet and l[:col].strip() in ("", ) or
                        (l[col:col + 2] == bullet and l[:col].rstrip().endswith("=="))]
            if len(siblings) < 2:
                continue
            newline = "\n" if lines[end - 1][1].endswith("\n") else ""
            sites.append((cls, start, item_end, " " * col + bullet + " " + unit + newline))
    return sites


def sites_of(text, sema):
    mask = sema.code_mask(text)
    lo, hi = sema.body_range(text)
    return operator_sites(text, mask, lo, hi) + drop_sites(text, mask, lo, hi)


def used_siblings(path):
    """The model's module and the same-directory modules it uses, transitively."""
    seen, todo = [], [path]
    while todo:
        p = todo.pop()
        if p in seen or not p.is_file():
            continue
        seen.append(p)
        for m in USES.finditer(p.read_text(encoding="utf-8", errors="replace")):
            for name in NAME.findall(m.group(1)):
                todo.append(p.parent / (name + ".tla"))
    return seen


def pick(sites, rng, count):
    """Round-robin over classes, in a seeded order within each class."""
    by_class = {c: [s for s in sites if s[1] == c] for c in CLASSES}
    for c in CLASSES:
        rng.shuffle(by_class[c])
    chosen = []
    while len(chosen) < count and any(by_class.values()):
        for c in CLASSES:
            if by_class[c] and len(chosen) < count:
                chosen.append(by_class[c].pop())
    return chosen


# ---------------------------------------------------------------------------
# Preparation


def candidate_models(source):
    rows = []
    for manifest in sorted((source / "specifications").glob("*/manifest.json")):
        spec = json.loads(manifest.read_text())
        for module in spec["modules"]:
            for model in module["models"]:
                runtime = model.get("runtime", "unknown")
                ok = (model.get("mode") == "exhaustive search" and runtime != "unknown"
                      and runtime <= MAX_RUNTIME)
                rows.append({"module": module, "model": model, "spec_dir": manifest.parent,
                             "selection": "candidate" if ok else "not-small-exhaustive"})
    return rows


def prepare(repo, source, out, seed, max_models, per_model):
    repo, source, out = repo.resolve(), source.resolve(), out.resolve()
    if out.is_relative_to(repo) or out.is_relative_to(source):
        raise ValueError("campaign output must be outside the checkout and source cache")
    if out.exists():
        raise ValueError("--out must not exist; refusing to overwrite a campaign")
    if not (1 <= max_models <= 256 and 1 <= per_model <= 32):
        raise ValueError("models/per-model outside the bounded campaign limits")
    pin = source_pin(repo)
    verify_source(source, pin)
    sema = load_script(repo / "crates/dive-sema/acceptance/mutate.py", "semantic_mutations")
    out.mkdir(parents=True)
    conf = out / "conformance"
    corpus = conf / "corpus" / CORPUS / "specifications"
    corpus.mkdir(parents=True)
    shutil.copyfile(repo / "conformance/oracles.toml", conf / "oracles.toml")
    census, entries = [], []
    rows = candidate_models(source)
    ranked = sorted((r for r in rows if r["selection"] == "candidate"),
                    key=lambda r: digest((str(seed) + ":" + r["model"]["path"]).encode()))
    for i, row in enumerate(ranked):
        if i >= max_models:
            row["selection"] = "outside-model-budget"
            continue
        root = source / row["module"]["path"]
        files = [p for p in used_siblings(root)]
        sites = []
        for p in files:
            text = p.read_text(encoding="utf-8")
            if not HEADER.search(text):
                continue
            sites.extend((p, *site) for site in sites_of(text, sema))
        rng = random.Random(int(digest((str(seed) + ":" + row["model"]["path"]).encode()), 16))
        chosen = pick(sites, rng, per_model)
        row["selection"] = "selected"
        row["sites"] = len(sites)
        row["mutants"] = len(chosen)
        for j, (path, cls, start, end, new) in enumerate(chosen):
            name = "%03d-%02d-%s" % (i, j, cls)
            dest = corpus / name
            shutil.copytree(row["spec_dir"], dest, symlinks=False)
            target = dest / path.relative_to(row["spec_dir"])
            text = path.read_text(encoding="utf-8")
            mutated = text[:start] + new + text[end:]
            target.write_text(mutated, encoding="utf-8")
            rebase = lambda p: "specifications/" + name + "/" + Path(p).relative_to(
                row["spec_dir"].relative_to(source)).as_posix()
            module = dict(row["module"])
            module["path"] = rebase(module["path"])
            model = dict(row["model"])
            model["path"] = rebase(model["path"])
            module["models"] = [model]
            (dest / "manifest.json").write_text(json.dumps(
                {"sources": [], "authors": ["model-mutation campaign"], "tags": ["mutant", cls],
                 "modules": [module]}, indent=2) + "\n")
            line = text.count("\n", 0, start) + 1
            entries.append({"mutant": name, "model": model["path"], "source_model": row["model"]["path"],
                            "file": target.relative_to(corpus).as_posix(), "class": cls, "line": line,
                            "from": text[start:end], "to": new,
                            "sha256": digest(mutated.encode("utf-8"))})
    for row in rows:
        census.append({"model": row["model"]["path"], "selection": row["selection"],
                       "sites": row.get("sites"), "mutants": row.get("mutants")})
    if not entries:
        raise ValueError("empty mutation selection")
    (conf / "corpora.toml").write_text('[[corpus]]\nid = ' + json.dumps(CORPUS) +
        '\npath = "conformance/corpus/' + CORPUS + '"\nlicense = "MIT"\nlayout = "examples-manifest"\n')
    selection = {"schema_version": 1, "seed": seed, "source_commit": pin,
                 "model_limit": max_models, "per_model": per_model, "max_runtime": MAX_RUNTIME,
                 "mutator_sha256": digest(Path(__file__).read_bytes()),
                 "model_census": census, "mutants": entries}
    (out / "selection.json").write_text(json.dumps(selection, indent=2) + "\n")
    verify_source(source, pin)
    return selection


# ---------------------------------------------------------------------------
# Running


def tally(report, selection):
    by_model = {row["model"]: row for row in report.get("models", [])}
    expected = {e["model"] for e in selection["mutants"]}
    if set(by_model) != expected:
        raise ValueError("report omitted or added campaign models")
    totals, by_class, findings = {}, {}, []
    for e in selection["mutants"]:
        row = by_model[e["model"]]
        outcome = row.get("outcome", "missing")
        totals[outcome] = totals.get(outcome, 0) + 1
        by_class.setdefault(e["class"], {})
        by_class[e["class"]][outcome] = by_class[e["class"]].get(outcome, 0) + 1
        if outcome not in ("agree", "expected-divergence", "known-upstream-change"):
            findings.append({"mutant": e["mutant"], "class": e["class"], "outcome": outcome,
                             "golden_verdict": row.get("golden_verdict"), "detail": row.get("detail"),
                             "diffs": [d for run in row.get("runs", []) for d in run.get("diffs", [])],
                             "edit": {"file": e["file"], "line": e["line"], "from": e["from"], "to": e["to"]}})
    return totals, by_class, findings


def run(repo, out, selection, harness, dive, jobs, timeout):
    harness, dive = harness.resolve(), dive.resolve()
    common = [str(harness), "--repo-root", str(out)]
    reports = out / "reports"
    reports.mkdir()
    steps = []

    def command(name, args):
        with (out / (name + ".log")).open("w") as log:
            status = subprocess.run(common + args, cwd=repo, stdout=log, stderr=subprocess.STDOUT).returncode
        steps.append({"name": name, "exit_code": status})

    command("oracle-verify", ["oracle", "verify"])
    if steps[-1]["exit_code"]:
        raise ValueError("pinned oracle verification failed")
    limits = ["--corpus", CORPUS, "--tier", "all", "--no-sany", "--jobs", str(jobs), "--timeout", str(timeout)]
    command("generate", ["golden", "generate", *limits,
                         "--reason", "model-mutation campaign seed " + str(selection["seed"])])
    command("run", ["run", "--impl", "dive", "--impl-profile", "tlc", "--dive-bin", str(dive),
                    "--workers", "1", *limits, "--no-summary", "--report-dir", str(reports)])
    files = sorted(reports.glob("*.json"))
    if len(files) != 1:
        raise ValueError("expected one report")
    totals, by_class, findings = tally(json.loads(files[0].read_text()), selection)
    result = {"schema_version": 1, "kind": "model-mutation-campaign", "seed": selection["seed"],
              "source_commit": selection["source_commit"],
              "selection_sha256": digest((out / "selection.json").read_bytes()),
              "implementation_sha256": digest(dive.read_bytes()), "harness_sha256": digest(harness.read_bytes()),
              "mutants": len(selection["mutants"]), "steps": steps, "totals": totals,
              "by_class": by_class, "findings": findings}
    (out / "campaign.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: v for k, v in result.items() if k != "findings"}, indent=2))
    print("%d findings; see %s" % (len(findings), out / "campaign.json"))
    return 0 if not findings and steps[0]["exit_code"] == 0 else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=20261027)
    parser.add_argument("--models", type=int, default=32)
    parser.add_argument("--per-model", type=int, default=8)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--timeout", type=int, default=60)
    parser.add_argument("--harness", type=Path)
    parser.add_argument("--dive-bin", type=Path)
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    repo, source, out = args.repo_root.resolve(), args.source_root.resolve(), args.out.absolute()
    if args.jobs < 1 or args.jobs > 8 or args.timeout < 1 or args.timeout > 600:
        parser.error("jobs/timeout outside campaign limits")
    selection = prepare(repo, source, out, args.seed, args.models, args.per_model)
    if args.prepare_only:
        print("prepared %d mutants" % len(selection["mutants"]))
        return 0
    return run(repo, out, selection, args.harness or repo / "target/release/dive-conformance",
               args.dive_bin or repo / "target/release/dive", args.jobs, args.timeout)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print("model mutation campaign: " + str(error), file=sys.stderr)
        sys.exit(2)
