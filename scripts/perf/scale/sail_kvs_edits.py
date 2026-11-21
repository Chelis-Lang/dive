"""Prepare the KVS source-edit controls for the installed Sail symbolic checker."""

import argparse
from hashlib import sha256
from pathlib import Path


INPUTS = {
    "KeyValueStore.tla": "0306e7659212ef214fdb40012f1f24a78b25669292a80127c815da5eec466b07",
    "MCKVS.tla": "4d66a40abeb29558b473770b736dc1bd16c841d9ac847b4644b36cd47d8c8962",
    "MCKVSSafetyLarge.cfg": "c1f414a1caf84df31089ecb569de0dd6b835fad640f7545b2b7b239b7aad7964",
}
EDITS = r"""TxIsClean == \A t \in TxId \ tx : missed[t] = {}
AtMostOneConflict == \A a \in tx : \A b \in tx :
    a /= b => ((missed[a] \cap written[a]) = {} \/ (missed[b] \cap written[b]) = {})
"""
COMMIT = r"""SomeCommit == tx = TxId => \E t \in tx : (missed[t] \cap written[t]) = {}
"""
FRESH = r"""FullHasFresh == tx = TxId => \E t \in tx : missed[t] = {}
"""
ACTIONS = {
    "AbortAll": r"""AbortAll ==
    /\ tx /= {}
    /\ tx' = {}
    /\ snapshotStore' = [t \in TxId |-> [k \in Key |-> NoVal]]
    /\ written' = [t \in TxId |-> {}]
    /\ missed' = [t \in TxId |-> {}]
    /\ UNCHANGED store""",
    "Corrupt": r"""Corrupt ==
    /\ tx' = TxId
    /\ written' = [t \in TxId |-> Key]
    /\ missed' = [t \in TxId |-> Key]
    /\ UNCHANGED <<store,snapshotStore>>""",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kvs", type=Path, help="specifications/KeyValueStore from pinned Examples")
    parser.add_argument("destination", type=Path, help="new directory for all inputs")
    parser.add_argument("--keys", type=int, default=6)
    parser.add_argument("--values", type=int, default=2)
    parser.add_argument("--transactions", type=int, default=3)
    args = parser.parse_args()
    if min(args.keys, args.values, args.transactions) < 1:
        parser.error("each domain must be nonempty")
    inputs = {}
    for name, digest in INPUTS.items():
        raw = (args.kvs / name).read_bytes()
        if sha256(raw).hexdigest() != digest:
            parser.error(f"input digest mismatch for {name}")
        inputs[name] = raw
    if args.destination.exists():
        parser.error("destination already exists")
    args.destination.mkdir()
    base_cfg = inputs["MCKVSSafetyLarge.cfg"].decode()
    for old, prefix, n in (("k1, k2, k3", "k", args.keys), ("v1, v2", "v", args.values),
                           ("t1, t2, t3", "t", args.transactions)):
        base_cfg = base_cfg.replace(old, ", ".join(f"{prefix}{i}" for i in range(1, n + 1)))
    base = inputs["MCKVS.tla"].decode()
    revised = base.replace("====", EDITS + "====")
    commit = revised.replace("====", COMMIT + "====")

    def write(name, module, config, *properties):
        path = args.destination / name
        path.mkdir()
        (path / "KeyValueStore.tla").write_bytes(inputs["KeyValueStore.tla"])
        (path / "MCKVS.tla").write_text(module)
        if properties:
            config += "\n" + "".join(f"INVARIANT {prop}\n" for prop in properties)
        (path / "Model.cfg").write_text(config)

    write("base", base, base_cfg)
    for prop in ("TxIsClean", "AtMostOneConflict"):
        write(prop, revised, base_cfg, prop)
    write("SomeCommit", commit, base_cfg, "SomeCommit")
    write("FullHasFresh", commit.replace("====", FRESH + "===="),
          base_cfg, "SomeCommit", "FullHasFresh")
    cfg = base_cfg.replace("SPECIFICATION\n    Spec", "INIT Init\nNEXT ChangedNext")
    for name, action in ACTIONS.items():
        addition = f"{action}\nChangedNext == Next \\/ {name}\n"
        write(name, commit.replace("====", addition + "===="), cfg, "SomeCommit")


if __name__ == "__main__":
    main()
