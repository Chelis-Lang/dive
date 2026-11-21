"""Prepare a pinned Kafka KIP-996 voting progress check in a new directory."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil

HERE = Path(__file__).resolve().parent
RECORD = HERE.parents[3] / "conformance/reports/perf/2026-11-21-kafka-liveness.json"
CASES = ("original", "fixed", "two", "election", "NoOutgoing", "NoIncoming", "NoEmpty",
         "unhandled", "reinsert")


def edited_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f"expected one occurrence of {old!r}")
    return source.replace(old, new)


def prepare(source, case, dest):
    pins = json.loads(RECORD.read_text())["hashes"]
    for name, digest in pins.items():
        if hashlib.sha256((source / name).read_bytes()).hexdigest() != digest:
            raise ValueError(f"unexpected source or configuration: {name}")
    dest.mkdir(parents=True, exist_ok=False)
    for name in pins:
        if name.endswith(".tla"):
            shutil.copyfile(source / name, dest / name)
    shutil.copyfile(HERE / "KRaftVoteDrain.tla", dest / "KRaftVoteDrain.tla")
    constants = (source / "model_liveness.cfg").read_text().split("\nSPECIFICATION")[0]
    checks = "INVARIANT DrainEnabled\nPROPERTY DrainSteps"
    bound = case in ("two", "election", "reinsert")
    if bound or case == "fixed":
        servers = 2 if case in ("two", "reinsert") else 3
        settings = dict(InitClusterSize=servers, MinClusterSize=servers,
                        MaxClusterSize=servers, MaxSpawnedServers=servers,
                        Hosts="{" + ",".join(f"h{i}" for i in range(1, servers + 1)) + "}",
                        Value="{v1}", MaxAddReconfigs=1, MaxRemoveReconfigs=0,
                        MaxPermCrashes=0, MaxRestarts=0, MaxDisruptiveElectionTriggers=int(bound),
                        MaxConnectivityChanges=0)
        for key, value in settings.items():
            constants, count = re.subn(rf"(?m)^    {key} = [^\n]*", f"    {key} = {value}", constants)
            if count != 1:
                raise ValueError(f"expected one assignment to {key}")
    if case.startswith("No"):
        checks = "INVARIANT " + case
    elif case == "unhandled":
        checks = "INVARIANT DrainEnabled"
        module = dest / "kraft_kip_996.tla"
        before, signature, body = module.read_text().partition("HandleVoteResponse(s, peer) ==")
        guard = r"/\ IsVoter(s) \* new check"
        module.write_text(before + signature + edited_once(body, guard, "/\\ FALSE\n        " + guard))
    elif case == "reinsert":
        checks = "PROPERTY DrainSteps"
        module = dest / "network.tla"
        module.write_text(edited_once(module.read_text(),
                          "net_messages' = SendFunc(m, DiscardFunc(d, net_messages), 1)",
                          "net_messages' = SendFunc(m, net_messages, 1)"))
    cfg = (constants + "\nINIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n"
           + ("CONSTRAINT Bound\n" if bound else "") + checks + "\n")
    (dest / (case + ".cfg")).write_text(cfg)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="pinned upstream kraft/kip_996 directory")
    parser.add_argument("case", choices=CASES)
    parser.add_argument("dest", type=Path, help="new output directory")
    args = parser.parse_args()
    prepare(args.source, args.case, args.dest)
