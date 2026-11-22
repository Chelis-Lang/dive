#!/usr/bin/env python3
"""Check a caller-declared finite symmetry on original TLA+ inputs with the Dive CLI."""
import argparse
import hashlib
import itertools
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory
import time


class Refused(ValueError):
    """The requested comparison has no supported, conclusive result."""


def require(test, reason):
    if not test:
        raise Refused(reason)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def run_json(binary, args, folder, seconds, census=False, home=None):
    # The original and the wrapper see the same source snapshot and the same
    # built-in library policy. No caller optimizer or library env var leaks in.
    env = {"HOME": str(home or folder), "PATH": os.environ.get("PATH", ""), "LC_ALL": "C"}
    if census:
        env["DIVE_CORE_OPT"] = "+orbit-census"
    started = time.monotonic()
    result = subprocess.run([binary, *args, "--output", "json"], cwd=folder,
                            env=env, capture_output=True, timeout=seconds + 5)
    try:
        data = json.loads(result.stdout)
    except ValueError as error:
        raise Refused(result.stderr.decode(errors="replace")[:1500] or str(error)) from error
    require(isinstance(data, dict) and result.returncode >= 0, f"{args[0]} terminated without a complete report")
    if args[0] == "check":
        expected = {"success": 0, "invariant-violation": 12, "action-property-violation": 13,
                    "deadlock": 11}.get(data.get("verdict"))
    elif args[0] == "validate-trace":
        expected = 0 if data.get("result") == "valid" else None
    else:
        expected = 0 if data.get("accepted") else None
    if expected is not None:
        require(result.returncode == expected, f"{args[0]} report disagrees with exit {result.returncode}")
    return data, round(time.monotonic() - started, 6)


def value(node, rename):
    for key in ("model_value", "string", "int", "integer", "bool", "boolean"):
        if key in node:
            v = node[key]
            return key, (rename.get(v, v) if key == "model_value" else v)
    require("set" in node, f"unsupported configuration value {list(node)}")
    return "set", frozenset(value(child, rename) for child in node["set"])


def check_constants(config, carrier, observers):
    require(not observers["refusals"], "; ".join(observers["refusals"][:5]))
    for option in ("symmetry", "view", "alias", "periodic", "postconditions", "rl_reward", "possible"):
        require(not config.get(option), f"unsupported configured {option}")
    constants = {}
    for entry in config["constants"]:
        require(entry["kind"] == "assignment" and not entry["module"] and not entry["args"],
                "operator substitutions and qualified or operator-valued assignments require a separate audit")
        name = entry["name"]["text"]
        require(name not in constants, f"repeated constant binding {name}")
        constants[name] = entry["value"]
    domain = constants.get(carrier, {}).get("set", [])
    atoms = sorted({item.get("model_value", "") for item in domain})
    require(2 <= len(atoms) <= 4 and len(atoms) == len(domain)
            and all(re.fullmatch(r"[A-Za-z][A-Za-z0-9]*", a) for a in atoms),
            f"{carrier} must be a literal set of two to four distinct untyped model values")
    for left, right in itertools.pairwise(atoms):
        swap = {left: right, right: left}
        for name, val in constants.items():
            require(value(val, swap) == value(val, {}), f"{name} distinguishes {left} and {right}")
        for name in observers["choice_domains"]:
            choices = constants.get(name, {}).get("set")
            require(choices is not None and all(value(x, swap) == value(x, {}) for x in choices),
                    f"CHOOSE requires a pointwise-fixed explicit domain: {name}")
    return atoms


def source_snapshot(args, binary, folder):
    source = args.module.resolve()
    report, _ = run_json(binary, ["analyze", str(source), "--orbit-observers"], source.parent, args.seconds, home=folder)
    require(report.get("accepted"), f"original source did not pass analysis: {report.get('diagnostics', [])[:1]}")
    require("orbit_observers" in report, "this Dive binary lacks --orbit-observers")
    pins = {}
    for record in report["files"]:
        if record["origin"] == "standard":
            continue
        require(record["origin"] == "file", f"unsupported source origin {record['origin']}")
        path = (source.parent / record["path"]).resolve()
        require(path.parent == source.parent and path.name not in pins, f"unsupported module path {path.name}")
        shutil.copyfile(path, folder / path.name)
        pins[path.name] = digest(folder / path.name)
    require(source.name in pins and source.stem == report["module"], "module name and file must agree")
    # Reanalyze the snapshot: an edit while the initial closure was discovered
    # must never attach old observer facts or digests to the checked inputs.
    report, _ = run_json(binary, ["analyze", source.name, "--orbit-observers"], folder, args.seconds)
    require(report.get("accepted") and report.get("module") == source.stem, "could not bind the input snapshot")
    require({Path(f["path"]).name for f in report["files"] if f["origin"] != "standard"} == set(pins),
            "the source module closure changed while copying")
    cfg = folder / "input.cfg"
    cfg.write_bytes(args.config.read_bytes())
    parsed, _ = run_json(binary, ["parse", source.name, "--config", cfg.name], folder, args.seconds)
    require(parsed.get("accepted") and parsed["config"]["accepted"], "invalid original source or configuration")
    config = parsed["config"]["model"]
    atoms = check_constants(config, args.carrier, report["orbit_observers"])
    wrapper = "DiveDeclaredOrbit"
    operator = "DiveDeclaredOrbitGroup"
    # Pick scratch names outside the whole snapshot. Identifiers also found in
    # strings or comments only force another suffix; the frontend binds the result.
    taken = {word for name in pins for word in
             re.findall(r"[A-Za-z_][A-Za-z0-9_]*", (folder / name).read_text())}

    def fresh(name):
        while name in taken:
            name += "_"
        taken.add(name)
        return name

    group, fvar, left, right = (fresh("DiveOrbit" + n) for n in ("Swaps", "Key", "Left", "Right"))
    reserved = {wrapper, operator, group}
    roles = [config.get(k) for k in ("specification", "init", "next")]
    for key in ("constraints", "action_constraints", "invariants", "properties"):
        roles.extend(config.get(key, ()))
    require(not any(r and r["text"].split("!")[0] in reserved for r in roles),
            "configuration role collides with an orbit helper")
    require(wrapper not in report["modules"] and
            all(d["name"] not in reserved for d in report["definitions"]), "orbit wrapper name collision")
    # Every pair transposition is bijective; these generate the full symmetric
    # group. Core checks bijectivity and materializes the closure. The alias is
    # an ordinary zero-arity SYMMETRY definition accepted by the config binder.
    carrier = args.carrier
    adapter = (f"---- MODULE {wrapper} ----\nEXTENDS {source.stem}\n"
               f"LOCAL {group} == {{[{fvar} \\in {carrier} |-> IF {fvar} = {left} THEN {right} "
               f"ELSE IF {fvar} = {right} THEN {left} ELSE {fvar}] : {left}, {right} \\in {carrier}}}\n"
               f"{operator} == {group}\n====\n")
    extra_cfg = f"\nSYMMETRY {operator}\n"
    (folder / (wrapper + ".tla")).write_text(adapter)
    (folder / "orbit.cfg").write_bytes(cfg.read_bytes() + extra_cfg.encode())
    return source.name, wrapper + ".tla", dict(
        wrapper_module=wrapper + ".tla", wrapper_source=adapter, config_suffix=extra_cfg,
        files=pins, config_sha256=digest(cfg), carrier=args.carrier, atoms=atoms,
        observers=report["orbit_observers"], invariants=[i["text"] for i in config["invariants"]],
        properties=[p["text"] for p in config["properties"]])


def check(args, binary, folder, module, cfg, orbit):
    cmd = ["check", module, "--config", cfg, "--store", "exact", "--compat=tlc",
           "--workers", str(args.workers), "--seed", "0", "--fp", "0",
           "--time-limit", f"{args.seconds}s", "--max-memory", args.max_memory]
    report, seconds = run_json(binary, cmd, folder, args.seconds, orbit)
    verdict = report.get("verdict")
    require(verdict in {"success", "invariant-violation", "action-property-violation", "deadlock"},
            f"inconclusive {module}: {verdict} {report.get('reason', report.get('message', ''))}")
    stats = report["statistics"]
    metrics = report["core"]["metrics"]
    result = dict(verdict=verdict, statistics=stats, seconds=seconds, exact_states=metrics["exact_states"])
    if verdict == "success":
        require(stats["left_on_queue"] == 0 and stats["distinct"] == metrics["exact_states"],
                "search did not exhaust exact membership")
        if orbit:
            census = metrics.get("orbit_census", {})
            require("labeled_orbit_union" in census and "weighted_generated" in census,
                    f"missing complete orbit census: {census}")
            result["census"] = census
    else:
        result["property"] = report.get("property")
        result["trace"] = report["counterexample"]["trace"]
    return result


def verify_word(args, binary, folder, original, result):
    trace = folder / "witness.json"
    trace.write_text(json.dumps(result["trace"]))
    validation, _ = run_json(binary, ["validate-trace", original, "--config", "input.cfg",
                             "--trace", trace.name, "--compat=tlc", "--seed", "0", "--fp", "0"],
                             folder, args.seconds)
    require(validation.get("result") == "valid", "counterexample failed independent original-source replay")
    result["original_trace_replay"] = "valid"


def audit(args):
    installed = Path(args.dive).resolve()
    require(re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", args.carrier), "invalid carrier identifier")
    require(1 <= args.workers <= 256 and args.seconds > 0, "expected positive, bounded settings")
    start = time.monotonic()
    with TemporaryDirectory(prefix="dive-orbit-") as tmp:
        folder = Path(tmp)
        # An installation can be replaced between subprocesses. All observers,
        # searches and the independent validator must use the same bytes.
        executable = folder / ".orbit-checker"
        shutil.copyfile(installed, executable)
        executable.chmod(0o500)
        binary = str(executable)
        original, wrapper, inputs = source_snapshot(args, binary, folder)
        identity = dict(inputs=inputs, binary_sha256=digest(Path(binary)), driver_sha256=digest(Path(__file__)),
                        workers=args.workers, max_memory=args.max_memory, seconds_per_check=args.seconds)
        if args.prepare_only:
            return dict(status="prepared", **identity)
        reduced = check(args, binary, folder, wrapper, "orbit.cfg", True)
        result = dict(**identity,
                      prepare_and_reduced_seconds=round(time.monotonic() - start, 6), reduced=reduced)
        if reduced["verdict"] != "success":
            try:
                verify_word(args, binary, folder, original, reduced)
            except Refused as error:
                if args.assume_equivariance:
                    raise
                reduced["original_trace_replay"] = str(error)
        if args.certify_equivariance:
            if reduced["verdict"] != "success":
                if reduced.get("original_trace_replay") != "valid":
                    raise Refused("uncertified reduced counterexample")
                result["status"] = "validated-counterexample"
                return result
            from orbit_certificate import establish
            result["certificate"] = establish(
                args, folder, original, identity, reduced,
                lambda: check(args, binary, folder, original, "input.cfg", False))
            result["status"] = "certified-orbit"
            return result
        if args.assume_equivariance:
            result["status"] = "conditional-orbit" if reduced["verdict"] == "success" else "validated-counterexample"
            return result
        full = check(args, binary, folder, original, "input.cfg", False)
        result["unreduced"] = full
        if full["verdict"] != "success":
            verify_word(args, binary, folder, original, full)
            same_failure = (reduced.get("original_trace_replay") == "valid" and
                            (reduced["verdict"], reduced.get("property")) == (full["verdict"], full.get("property")))
            result["status"] = "validated-counterexample" if same_failure else "rejected-symmetry"
        elif reduced["verdict"] != "success":
            result["status"] = "rejected-symmetry"
        else:
            census = reduced["census"]
            expected = (int(census["labeled_orbit_union"]), int(census["weighted_generated"]), reduced["statistics"]["depth"])
            observed = tuple(full["statistics"][key] for key in ("distinct", "generated", "depth"))
            result["status"] = "verified-observables" if expected == observed else "rejected-symmetry"
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path)
    parser.add_argument("--config", required=True, type=Path)
    parser.add_argument("--carrier", required=True)
    parser.add_argument("--dive", default=shutil.which("dive") or "target/release/dive")
    parser.add_argument("--workers", type=int, default=1)
    parser.add_argument("--seconds", type=int, default=180)
    parser.add_argument("--max-memory", default="auto")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--prepare-only", action="store_true", help="emit source pins and the adapter for another process")
    mode.add_argument("--assume-equivariance", action="store_true",
                        help="skip direct comparison; successful weighted counts are conditional")
    mode.add_argument("--certify-equivariance", choices=("finite", "graph"),
                      help="prove all generator swaps on finite roles or the original reachable graph")
    parser.add_argument("--type-invariant", help="complete finite layout for the finite proof")
    parser.add_argument("--exporter", type=Path, default=Path("target/release/examples/symbolic_input"))
    parser.add_argument("--graph-auditor", type=Path, default=Path("target/release/examples/orbit_graph"))
    parser.add_argument("--certificate-cache", type=Path, help="shared authenticated certificate directory")
    parser.add_argument("--proof-memory-mib", type=int, default=4096)
    args = parser.parse_args()
    if args.certify_equivariance == "finite" and not args.type_invariant:
        parser.error("the finite proof requires --type-invariant")
    if args.proof_memory_mib < 1 or (args.certificate_cache and not args.certify_equivariance):
        parser.error("expected a positive proof budget and a proof mode for the cache")
    try:
        result = audit(args)
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        result = {"status": "refused", "reason": str(error)}
    print(json.dumps(result, sort_keys=True))
    return {"verified-observables": 0, "certified-orbit": 0, "conditional-orbit": 3, "prepared": 3, "validated-counterexample": 1}.get(result["status"], 2)


if __name__ == "__main__":
    sys.exit(main())
