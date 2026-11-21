"""Independently validate the compiled checker's four-state ZeroBarriers witness."""

import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import subprocess


def main(args):
    source = args.module.read_bytes()
    cfg = args.config.read_bytes()
    if sha256(source).hexdigest() != "edaa0c49822fa3272127ffee2e1358e42c7b5137a0f86e97a1a9b1fd424d6211":
        raise ValueError("expected the released LeanP1.tla")
    if sha256(cfg).hexdigest() != "ea6cb0a200d45fa3b9ae9e4e51a4d23373234850b289b91b52997fc1c51577d7":
        raise ValueError("expected the compiled small-control CFG")
    out = args.new_directory
    out.mkdir(exist_ok=False)
    source = source.decode().replace("MODULE LeanP1 ", "MODULE Fault ")
    end = source.rindex("\n====") + 1
    (out / "Fault.tla").write_text(source[:end] + "ZeroBarriers == barriers = 0\n" + source[end:])
    cfg = cfg.decode().rstrip() + "\nINVARIANT ZeroBarriers\n"
    (out / "Fault.cfg").write_text(cfg)
    pred = ["---- MODULE PathCheck ----", "EXTENDS Fault, TLC", "CONSTANTS A, B, C, p1"]
    states = re.findall(r"^State (\d+):\n(.*?)(?=^State \d+:|^Error:)", args.witness.read_text(), re.M | re.S)
    if len(states) != 4:
        raise ValueError("expected four printed states")
    names = None
    for i, (number, body) in enumerate(states):
        fields = re.findall(r"^/\\ (\w+) = (.+)$", body, re.M)
        keys = {key for key, _ in fields}
        if int(number) != i + 1 or len(keys) != 31 or len(fields) != 31 or (i and keys != names):
            raise ValueError("incomplete or inconsistent printed state")
        names = keys
        pred += [f"Position{i} =="] + [f"    /\\ {k} = {v}" for k, v in fields]
    moves = " \\/ ".join(f"(Position{i} /\\ Position{i+1}')" for i in range(3))
    pred += ["PathInit == Init /\\ Position0", f"PathNext == Next /\\ ({moves})", "===="]
    (out / "PathCheck.tla").write_text("\n".join(pred) + "\n")
    (out / "PathCheck.cfg").write_text(cfg.replace("SPECIFICATION Spec", "INIT PathInit\nNEXT PathNext") +
                                       "\nCONSTANTS\n A = A\n B = B\n C = C\n p1 = p1\n")

    def run(argv):
        proc = subprocess.run([args.dive, *argv, "--output", "json"], cwd=out,
                              stdout=subprocess.PIPE, text=True, check=False)
        return json.loads(proc.stdout)

    report = run(["check", "PathCheck.tla", "--config", "PathCheck.cfg", "--workers", "1",
                  "--store", "exact", "--seed", "0", "--fp", "0"])
    if report["verdict"] != "invariant-violation" or report["property"] != "ZeroBarriers":
        raise ValueError("the source did not reproduce the compiled violation")
    word = report["counterexample"]["trace"]
    if word["length"] != 4 or len(word["states"]) != 4:
        raise ValueError("the reconstructed trace has the wrong length")
    for state in word["states"]:
        state["action"] = None
    for label, expected in [("valid", None), ("wrong-init", "init"), ("skipped-edge", "next")]:
        trial = json.loads(json.dumps(word))
        if label == "wrong-init":
            trial["states"][0]["values"]["barriers"] = 999
        elif label == "skipped-edge":
            del trial["states"][1]
            trial["length"] -= 1
        path = label + ".json"
        (out / path).write_text(json.dumps(trial))
        verdict = run(["validate-trace", "Fault.tla", "--config", "Fault.cfg", "--trace", path])
        if verdict["result"] != ("invalid" if expected else "valid") or verdict.get("failure", {}).get("check") != expected:
            raise ValueError(f"{label}: {verdict}")
        print(label, verdict["result"], verdict.get("failure", {}).get("check", ""))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("module", "config", "witness", "new_directory"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--dive", default="dive")
    args = parser.parse_args()
    main(args)
