#!/usr/bin/env python3
"""Check one-step inductiveness of the original SlateDB writer-entry and restart actions."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory

MODULE = "37f80bb106ca1e5c9981d0a91f51de4fdf04bfcbb9b6fcadfb8e7b16b463821c"
CONFIG = "03d666d0af4def6b6e9b7ded231f7dc8d5f114ddb03ec46f45254dce70233169"
PROBES = [
    # phase, manifest epoch, replayed values, fix, action and expected first failure
    ("VALIDATE_AFTER_REPLAY", 1, "<<>>", True, "ValidateEpochAfterReplay", "ConsistentMachineData"),
    ("VALIDATE_AFTER_REPLAY", 1, "<<other>>", True, "ValidateEpochAfterReplay", "ConsistentMachineData"),
    ("VALIDATE_AFTER_REPLAY", 1, "<<val>>", True, "ValidateEpochAfterReplay", None),
    ("VALIDATE_AFTER_REPLAY", 2, "<<other>>", True, "ValidateEpochAfterReplay", None),
    ("REPLAY_WAL", 1, "<<>>", False, "ReplayWAL", "ConsistentMachineData"),
    ("REPLAY_WAL", 1, "<<>>", True, "ReplayWAL", None),
    ("REPLAY_WAL", 1, "<<>>", True, "ReplayWAL", "EntrySafe"),
    ("FENCED", 2, "<<val>>", True, "RestartFencedWriter", None),
]

ENTRY = r"""
EntrySafe == \A w \in Writers :
    /\ wState[w] = VALIDATE_AFTER_REPLAY
    /\ ManifestRefreshDone(w)
    /\ wEpoch[w] = wManifest[w].writerEpoch
    => /\ PrefixOf(wMachineData[w], auxWrittenEntries)
       /\ wManifest[w].id = LastManifestId => Len(wMachineData[w]) = Len(auxWrittenEntries)
RestartSafe == auxRestarts = 1 =>
    /\ Len(auxWrittenEntries) = 1
    /\ \A w \in Writers : wState[w] = INITIAL_MANIFEST_LOAD =>
           /\ wEpoch[w] = 0 /\ wMachineData[w] = <<>> /\ wManifest[w] = NIL
           /\ wRefreshState[w] = REFRESH_LOAD /\ wNextWalId[w] = 1
           /\ wReplayId[w] = 0 /\ wSnapshotId[w] = 0
"""


def once(text, old, new):
    if text.count(old) != 1:
        raise ValueError("source control does not have one occurrence of " + old)
    return text.replace(old, new)


def pinned(path, digest):
    content = path.read_bytes()
    if hashlib.sha256(content).hexdigest() != digest:
        raise ValueError("unexpected source: " + str(path))
    return content.decode()


def probe_init(source, phase, epoch, data):
    body = source.split("\nInit ==\n", 1)[1].split("\nNext ==\n", 1)[0]
    body = once(body, "writerEpoch      |-> 0", f"writerEpoch      |-> {epoch}")
    for name, old, changed in [
        ("wState", "INITIAL_MANIFEST_LOAD", phase), ("wEpoch", "0", "1"),
        ("wManifest", "NIL", "manifest[1]"), ("wRefreshState", "REFRESH_LOAD", "REFRESH_DONE"),
        ("wMachineData", "<<>>", data), ("wNextWalId", "1", "2"), ("wReplayId", "0", "2"),
    ]:
        pattern = rf"{name} = [w \in Writers |-> {old}]"
        body = once(body, pattern, rf"{name} = [w \in Writers |-> IF w = actor THEN {changed} ELSE {old}]")
    body = once(body, "wal = <<>>", "wal = <<[kind |-> DATA, epoch |-> 1, value |-> val]>>")
    body = once(body, "auxUsedValues = {}", "auxUsedValues = {val}")
    body = once(body, "auxWrittenEntries = <<>>", "auxWrittenEntries = <<[walId |-> 1, value |-> val]>>")
    return r"ProbeInit == \E actor \in Writers, val, other \in Values : /\ val /= other" + "\n" + body


def run(dive, folder, source, baseline, probe):
    phase, epoch, data, fix, action, expected = probe
    module = folder / "HistoryProbe.tla"
    module.write_text("---- MODULE HistoryProbe ----\nEXTENDS SlateDBWAL\n" + ENTRY
                      + probe_init(source, phase, epoch, data)
                      + rf"Step == \E w \in Writers : {action}(w)" + "\n====\n")
    cfg = once(baseline, "Writers = {w1, w2, w3}", "Writers = {w1, w2, w3, w4}")
    if not fix:
        cfg = once(cfg, "    EnableFix = TRUE", "    EnableFix = FALSE")
    cfg = once(cfg, "\nSPECIFICATION Spec\n", "\nINIT ProbeInit\nNEXT Step\n")
    if expected == "EntrySafe" or action == "RestartFencedWriter":
        cfg += "\nINVARIANT " + (expected or "RestartSafe") + "\n"
    conf = module.with_suffix(".cfg")
    conf.write_text(cfg)
    opts = [module, "--config", conf, "--output", "json", "--seed", "0", "--fp", "0"]
    check = subprocess.run([dive, "check", *opts, "--workers", "1", "--store", "exact",
                            "--time-limit", "10s", "--max-memory", "1g", "--exit-codes", "native"],
                           text=True, capture_output=True, timeout=15)
    result = json.loads(check.stdout)
    row = dict(phase=phase, epoch=epoch, data=data, fix=fix, action=action,
               verdict=result["verdict"], depth=result["statistics"]["depth"])
    if not expected:
        assert check.returncode == 0 and row["verdict"] == "success" and row["depth"] == 2, result
        assert result["statistics"]["left_on_queue"] == 0
        return row
    trace = result["counterexample"]["trace"]
    assert check.returncode == 10 and len(trace["states"]) == 2 and trace["ending"]["name"] == expected, result

    def replay():
        out = subprocess.run([dive, "validate-trace", *opts, "--trace", "/dev/stdin"],
                             input=json.dumps(trace), capture_output=True, text=True, timeout=15)
        return out.returncode, json.loads(out.stdout)

    conf.write_text(once(cfg, "NEXT Step", "NEXT Next"))
    rc, word = replay()
    assert rc == 0 and word["result"] == "valid", word
    conf.write_text(once(conf.read_text(), "INIT ProbeInit", "INIT Init"))
    rc, original = replay()
    assert rc != 0 and original["result"] == "invalid", original
    row.update(violated=expected, full_next_replay=word["result"], original_init_replay=original["result"])
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="directory containing the published SlateDBWAL.tla and CFG")
    parser.add_argument("--dive", default="dive")
    args = parser.parse_args()
    source = pinned(args.source / "SlateDBWAL.tla", MODULE)
    cfg = pinned(args.source / "SlateDBWAL.cfg", CONFIG)
    with TemporaryDirectory() as tmp:
        folder = Path(tmp)
        (folder / "SlateDBWAL.tla").write_text(source)
        for case in PROBES:
            print(json.dumps(run(args.dive, folder, source, cfg, case)), flush=True)


if __name__ == "__main__":
    main()
