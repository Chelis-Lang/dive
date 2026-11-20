#!/usr/bin/env python3
"""Run deterministic frontend campaigns through the pinned conformance harness.

Only disposable copies are mutated. SANY goldens and both phase reports live
under --out, never in the checkout. An oracle crash is a finding, not acceptance.
"""

import argparse
from concurrent.futures import ThreadPoolExecutor
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


CORPUS = "frontend-campaign"
HEADER = re.compile(r"^-{4,}\s*MODULE\s+([A-Za-z_][A-Za-z0-9_]*)\b", re.M)
PARSER_SOURCE_SHA = "012167ef75cbce6bbfade559ad3bd9b7d577ea2401ee853b680f507bf83fbde5"
PARSER_CLASS_SHA = "9cdcf84346808f0079e7fdb7ca7cc2eb411383b4feb71bcaa4df8abe48baed50"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load_script(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def plain_files(root, suffix):
    """Never follow a link into the verified source cache or outside a case."""
    files = []
    for directory, dirs, names in os.walk(root, followlinks=False):
        for name in dirs + names:
            p = Path(directory) / name
            if p.is_symlink():
                raise ValueError("symlink in campaign source: " + str(p))
        files.extend(Path(directory) / n for n in names if n.endswith(suffix))
    return sorted(files)


def source_pin(repo):
    # Read the actual pinned Examples block; do not introduce another pin.
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
    if not (root / "specifications").is_dir():
        raise ValueError("missing Examples specifications")


def copy_siblings(source, destination):
    destination.mkdir(parents=True)
    for sibling in sorted(source.parent.glob("*.tla")):
        if sibling.is_symlink() or not sibling.is_file():
            raise ValueError("nonplain module dependency: " + str(sibling))
        shutil.copyfile(sibling, destination / sibling.name)


def mutants(text, rng, per_file, syntax, sema, kind):
    """Keep an explicit count if the bounded edit search is exhausted."""
    generated = []
    for _ in range(per_file * 8):
        if len(generated) == per_file:
            break
        if kind == "semantic":
            result = rng.choice(sema.OPS)(text, sema.code_mask(text), rng)
            if result is None:
                continue
            value, edit = result
        elif kind == "layout":
            value = syntax.mutate(text, rng, ["tab", "indent", "crlf", "join"])
            edit = "layout"
        else:
            value = syntax.mutate(text, rng)
            edit = "syntax"
        if value is not None and value != text:
            generated.append((value, edit))
    return generated


def prepare(repo, source, out, seed, max_sources, per_file):
    repo, source, out = repo.resolve(), source.resolve(), out.resolve()
    if out.resolve().is_relative_to(repo) or out.resolve().is_relative_to(source):
        raise ValueError("campaign output must be outside the checkout and source cache")
    if out.exists():
        raise ValueError("--out must not exist; refusing to overwrite a campaign")
    if not (1 <= max_sources <= 512 and 1 <= per_file <= 32):
        raise ValueError("sources/per-file outside the bounded campaign limits")
    pin = source_pin(repo)
    verify_source(source, pin)
    source_files = plain_files(source / "specifications", ".tla")
    syntax = load_script(repo / "crates/dive-syntax/acceptance/mutate.py", "syntax_mutations")
    sema = load_script(repo / "crates/dive-sema/acceptance/mutate.py", "semantic_mutations")
    out.mkdir(parents=True)
    conf = out / "conformance"
    corpus = conf / "corpus" / CORPUS
    corpus.mkdir(parents=True)
    shutil.copyfile(repo / "conformance/oracles.toml", conf / "oracles.toml")
    census, selected, entries, attempts = [], [], [], []
    for path in source_files:
        data = path.read_bytes()
        rel = path.relative_to(source).as_posix()
        row = {"source": rel, "sha256": digest(data)}
        try:
            text = data.decode("utf-8")
            header = HEADER.search(text)
            if len(data) > 30_000:
                row["selection"] = "over-30000-bytes"
            elif header is None or header.group(1) != path.stem:
                row["selection"] = "header-name-mismatch"
            else:
                row["selection"] = "candidate"
                selected.append((digest((str(seed) + ":" + rel).encode()), path, text, row))
        except UnicodeDecodeError:
            row["selection"] = "invalid-utf8"
        census.append(row)
    selected.sort(key=lambda item: item[0])
    for i, (_, path, text, row) in enumerate(selected):
        if i >= max_sources:
            row["selection"] = "outside-source-budget"
            continue
        row["selection"] = "selected"
        for kind in ["syntax", "layout", "semantic"]:
            rng = random.Random(int(digest((str(seed) + ":" + row["source"] + ":" + kind).encode()), 16))
            made = mutants(text, rng, per_file, syntax, sema, kind)
            attempts.append({"source": row["source"], "kind": kind,
                             "requested": per_file, "generated": len(made),
                             "exhausted": len(made) != per_file})
            for j, (mutated, edit) in enumerate(made):
                dest = corpus / "mutants" / kind / ("%04d-%03d" % (i, j))
                copy_siblings(path, dest)
                # Every target has its own directory, original name and byte
                # dependencies. No root renaming or shared mutation writes.
                target = dest / path.name
                target.write_bytes(mutated.encode("utf-8"))
                entries.append({"module": target.relative_to(corpus).as_posix(),
                                "source": row["source"], "source_sha256": row["sha256"],
                                "kind": kind, "edit": edit, "sha256": digest(target.read_bytes())})
    for crate in ["dive-syntax", "dive-sema"]:
        cases = repo / "crates" / crate / "tests/sany-cases"
        files = plain_files(cases, ".tla")
        if not cases.is_dir() or not files:
            raise ValueError("missing SANY cases: " + crate)
        for path in files:
            rel = path.relative_to(cases)
            dest = corpus / "cases" / crate / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, dest)
            entries.append({"module": dest.relative_to(corpus).as_posix(),
                            "source": path.relative_to(repo).as_posix(), "kind": "sany-case",
                            "source_sha256": digest(path.read_bytes()), "sha256": digest(dest.read_bytes())})
    if not entries or not any(r["selection"] == "selected" for r in census):
        raise ValueError("empty mutation source selection")
    entries.sort(key=lambda row: row["module"])
    paths = [row["module"] for row in entries]
    if len(set(paths)) != len(paths):
        raise ValueError("duplicate campaign module")
    (conf / "corpora.toml").write_text('[[corpus]]\nid = ' + json.dumps(CORPUS) +
        '\npath = "conformance/corpus/' + CORPUS + '"\nlicense = "MIT"\nlayout = "modules-only"\ninclude = ' +
        json.dumps(paths) + '\n')
    selection = {"schema_version": 1, "seed": seed, "source_commit": pin,
                 "source_limit": max_sources, "per_file": per_file,
                 "mutators": {crate: digest((repo / "crates" / crate / "acceptance/mutate.py").read_bytes())
                              for crate in ["dive-syntax", "dive-sema"]},
                 "source_census": census, "mutation_attempts": attempts, "modules": entries}
    (out / "selection.json").write_text(json.dumps(selection, indent=2) + "\n")
    verify_source(source, pin)
    return selection


def check_report(report, modules, phase):
    """Census equality and agreement are both required; an empty run fails."""
    expected = {row["module"] for row in modules}
    rows = report.get("modules", [])
    observed = [row.get("module") for row in rows]
    if not expected or len(observed) != len(expected) or set(observed) != expected:
        raise ValueError("report omitted, duplicated or added campaign modules")
    if report.get("models"):
        raise ValueError("frontend campaign unexpectedly checked models")
    if any(row.get("corpus") != CORPUS or row.get("phase") != phase for row in rows):
        raise ValueError("report corpus/phase does not match the campaign")
    return {outcome: sum(row.get("outcome") == outcome for row in rows)
            for outcome in sorted({row.get("outcome", "missing") for row in rows})}


def parse_document(document, exit_code):
    """A supported schema-1/2/3 parser diagnostic, never an unrecognized process failure."""
    if document.get("schema_version") not in (1, 2, 3, 4) or document.get("command") != "parse":
        raise ValueError("invalid dive parse schema/command")
    if not isinstance(document.get("accepted"), bool) or not isinstance(document.get("diagnostics"), list):
        raise ValueError("invalid dive parse acceptance/diagnostics")
    errors = []
    for diagnostic in document["diagnostics"]:
        if diagnostic.get("severity") == "error":
            if not isinstance(diagnostic.get("code"), str) or not diagnostic["code"].startswith("DIVE-S"):
                raise ValueError("unexpected non-parser error")
            errors.append(diagnostic["code"])
    verdict = "reject" if any(code != "DIVE-S0006" for code in errors) else (
        "name-mismatch" if "DIVE-S0006" in errors else "accept")
    expected_accepted = verdict == "accept"
    if document["accepted"] != expected_accepted or exit_code != (0 if expected_accepted else 1):
        raise ValueError("contradictory parser acceptance/exit")
    return verdict


def direct_parse(repo, out, selection, dive, jobs, timeout):
    """Use the grammar itself so a post-parse PlusCal crash cannot masquerade as syntax."""
    java_dir = repo / "scripts/java"
    if digest((java_dir / "DiveSanyParse.java").read_bytes()) != PARSER_SOURCE_SHA or digest(
            (java_dir / "DiveSanyParse.class").read_bytes()) != PARSER_CLASS_SHA:
        raise ValueError("direct SANY parser source/bytecode pin mismatch")
    goldens = sorted((out / "conformance/golden").rglob("*.tla.json"))
    if not goldens:
        raise ValueError("no pinned SANY invocation for the grammar probe")
    golden = json.loads(goldens[0].read_text())
    command = golden["provenance"]["command"]
    cache = Path(os.environ.get("DIVE_CACHE_DIR", str(Path.home() / ".cache/dive"))).resolve()
    java = command[0].replace("$DIVE_CACHE_DIR", str(cache))
    classpath = command[command.index("-cp") + 1].replace("$DIVE_CACHE_DIR", str(cache))
    oracle = golden["provenance"]["oracle"]
    corpus = out / "conformance/corpus" / CORPUS
    def check(entry):
        path = corpus / entry["module"]
        row = {"module": entry["module"], "source_sha256": entry["sha256"], "outcome": "oracle-error"}
        try:
            actual = subprocess.run([java, "-Xmx1g", "-Xss8m", "-Duser.language=en", "-Duser.country=US", "-Dfile.encoding=UTF-8", "-cp", str(java_dir) + os.pathsep + classpath,
                                     "DiveSanyParse", str(path)], timeout=timeout, capture_output=True, text=True)
            row.update({"oracle_exit": actual.returncode, "oracle_stdout": actual.stdout, "oracle_stderr": actual.stderr})
            marker = re.fullmatch(r"DIVE-SANY-PARSE (accept|reject|name-mismatch)\s*", actual.stdout)
            if actual.returncode != 0 or marker is None:
                return row
            expected = marker.group(1)
            row["oracle_verdict"] = expected
            row["outcome"] = "implementation-error"
            observed = subprocess.run([str(dive), "parse", str(path), "--output", "json"],
                                      timeout=timeout, capture_output=True, text=True)
            row.update({"implementation_exit": observed.returncode, "implementation_stdout": observed.stdout,
                        "implementation_stderr": observed.stderr})
            result = parse_document(json.loads(observed.stdout), observed.returncode)
            row["implementation_verdict"] = result
            row["outcome"] = "agree" if expected == result else "disagree"
        except subprocess.TimeoutExpired as error:
            row.update({"outcome": "timeout", "detail": str(error)})
        except (OSError, ValueError) as error:
            row["detail"] = str(error)
        return row
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        rows = list(pool.map(check, selection["modules"]))
    totals = {outcome: sum(row["outcome"] == outcome for row in rows)
              for outcome in sorted({row["outcome"] for row in rows})}
    report = {"schema_version": 1, "kind": "sany-direct-grammar", "claim": "Root grammar acceptance only; no module resolution, semantics or PlusCal validation.",
              "oracle": oracle, "parser_source_sha256": PARSER_SOURCE_SHA, "parser_class_sha256": PARSER_CLASS_SHA,
              "selection_sha256": digest((out / "selection.json").read_bytes()), "rows": rows, "totals": totals}
    (out / "reports/direct-parse.json").write_text(json.dumps(report, indent=2) + "\n")
    return totals


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
    limits = ["--corpus", CORPUS, "--tier", "all", "--sany-only", "--jobs", str(jobs), "--timeout", str(timeout)]
    command("generate", ["golden", "generate", *limits, "--reason", "deterministic deep frontend campaign seed " + str(selection["seed"])])
    totals = {}
    for phase in ["full"]:
        directory = reports / phase
        command(phase, ["run", "--impl", "dive", "--impl-profile", "tlc", "--dive-bin", str(dive),
                        *limits, "--sany-phase", phase, "--no-summary", "--report-dir", str(directory)])
        files = sorted(directory.glob("*.json"))
        if len(files) != 1:
            raise ValueError("expected one " + phase + " report")
        totals[phase] = check_report(json.loads(files[0].read_text()), selection["modules"], phase)
    totals["direct-parse"] = direct_parse(repo, out, selection, dive, jobs, timeout)
    result = {"schema_version": 1, "kind": "frontend-campaign", "seed": selection["seed"],
              "source_commit": selection["source_commit"], "selection_sha256": digest((out / "selection.json").read_bytes()),
              "implementation_sha256": digest(dive.read_bytes()), "harness_sha256": digest(harness.read_bytes()),
              "selected_modules": len(selection["modules"]), "steps": steps, "totals": totals}
    (out / "campaign.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    return 0 if all(step["exit_code"] == 0 for step in steps) and all(
        set(counts) == {"agree"} for counts in totals.values()) else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=20261009)
    parser.add_argument("--sources", type=int, default=64)
    parser.add_argument("--per-file", type=int, default=4)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--timeout", type=int, default=15)
    parser.add_argument("--harness", type=Path)
    parser.add_argument("--dive-bin", type=Path)
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    repo, source, out = args.repo_root.resolve(), args.source_root.resolve(), args.out.absolute()
    if args.jobs < 1 or args.jobs > 8 or args.timeout < 1 or args.timeout > 120:
        parser.error("jobs/timeout outside campaign limits")
    selection = prepare(repo, source, out, args.seed, args.sources, args.per_file)
    if args.prepare_only:
        print("prepared %d modules" % len(selection["modules"]))
        return 0
    return run(repo, out, selection, args.harness or repo / "target/release/dive-conformance",
               args.dive_bin or repo / "target/release/dive", args.jobs, args.timeout)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print("frontend campaign: " + str(error), file=sys.stderr)
        sys.exit(2)
