"""Check support projection against explicit populations and certificate obligations against Core."""

import json
from types import SimpleNamespace

from occupancy_predicate import successors


def populations(total, classes):
    if classes == 1:
        yield (total,)
        return
    for count in range(total + 1):
        for tail in populations(total - count, classes - 1):
            yield (count, *tail)


def mask(counts):
    return sum(1 << i for i, count in enumerate(counts) if count)


def check_projection():
    for classes in range(1, 6):
        for size in range(9):
            concrete = {}
            for counts in populations(size, classes):
                for old, count in enumerate(counts):
                    if not count:
                        continue
                    for new in range(classes):
                        after = list(counts)
                        after[old] -= 1
                        after[new] += 1
                        concrete.setdefault((mask(counts), old, new), set()).add(mask(after))
            for (before, old, new), expected in concrete.items():
                model = SimpleNamespace(ids=range(size), tables={before: [(old, new, 1)]})
                assert successors(model, before) == expected


ARM = r'''---- MODULE Arm ----
CONSTANT Id
VARIABLE st
Bounds == st \in [Id -> {"a","b","c"}]
Start == st = [i \in Id |-> "a"]
First(i) == /\ st[i] = "a" /\ (\A j \in Id : st[j] = "a")
            /\ st' = [st EXCEPT ![i] = "b"]
Last(i) == /\ st[i] = "b" /\ st' = [st EXCEPT ![i] = "c"]
Step == \E i \in Id : First(i) \/ Last(i)
Safe == ~((\E i \in Id : st[i] = "b") /\ (\E j \in Id : st[j] = "c"))
====
'''


def check_certificates(folder, tcommit, cfg, original_cfg, run, compare):
    check_projection()
    cfg.write_text(original_cfg)
    proof = folder / "candidate.json"
    opts = ("--type-invariant", "TCTypeOK")
    cold = run(tcommit, cfg, *opts, "--write-predicate", proof)
    assert (cold["distinct"], cold["generated"], cold["depth"]) == (34, 94, 7)
    raw = proof.read_bytes()
    candidate = json.loads(raw)
    assert candidate["format"] == cold["predicate_format"] == "dive-occupancy-support-v1"

    def proved(row):
        assert row["proof"] == "inductive-safety" and row["failed_obligation"] is None
        assert not {"verdict", "distinct", "generated", "depth", "trace", "trace_validated"} & row.keys()

    for count in (1, 2, 5):
        cfg.write_text(original_cfg.replace("{r1, r2, r3}", "{" + ",".join(f"x{i}" for i in range(count)) + "}"))
        proved(run(tcommit, cfg, *opts, "--verify-predicate", proof))
        proved(run(tcommit, cfg, *opts, "--prove-inductive"))
        compare(tcommit, cfg, "TCTypeOK")
    cfg.write_text(original_cfg.replace("{r1, r2, r3}", "{}"))
    assert run(tcommit, cfg, *opts, "--verify-predicate", proof, code=2)["failed_obligation"] == "Init inclusion"
    proved(run(tcommit, cfg, *opts, "--prove-inductive"))

    cfg.write_text(original_cfg)
    for adjusted, expected in (({**candidate, "masks": []}, "Init inclusion"),
                               ({**candidate, "masks": [1]}, "Next closure"),
                               ({**candidate, "masks": [True]}, "invalid"),
                               ({**candidate, "masks": [1, 1]}, "duplicate"),
                               ({**candidate, "masks": [256]}, "invalid"),
                               ({**candidate, "statuses": ["??"]}, "labels")):
        proof.write_text(json.dumps(adjusted))
        row = run(tcommit, cfg, *opts, "--verify-predicate", proof, code=2)
        assert row["proof"] == "inconclusive" and expected in row["failed_obligation"]
    original = candidate["statuses"]
    candidate["statuses"] = original[::-1]
    candidate["masks"] = [int(f"{m:0{len(original)}b}"[::-1], 2) for m in candidate["masks"]]
    proof.write_text(json.dumps(candidate))
    proved(run(tcommit, cfg, *opts, "--verify-predicate", proof))

    arm = folder / "Arm.tla"
    arm.write_text(ARM)
    cfg.write_text("CONSTANT Id={p,q,r}\nINIT Start NEXT Step INVARIANTS Bounds Safe\nCHECK_DEADLOCK FALSE\n")
    row = compare(arm, cfg, "Bounds")
    assert (row["distinct"], row["generated"], row["depth"]) == (7, 7, 3)
    run(arm, cfg, "--type-invariant", "Bounds", "--write-predicate", proof)
    for flags in (("--verify-predicate", proof), ("--prove-inductive",)):
        row = run(arm, cfg, "--type-invariant", "Bounds", *flags, code=2)
        assert row["proof"] == "inconclusive" and row["failed_obligation"] == "Safe"
        assert not {"distinct", "generated", "depth", "trace", "trace_validated"} & row.keys()


EDITS = r'''---- MODULE Edits ----
EXTENDS TCommit
NoWorkingCommit == ~((\E r \in RM : rmState[r] = "working") /\ (\E r \in RM : rmState[r] = "committed"))
NonePrepared == \A r \in RM : rmState[r] # "prepared"
WithoutAbort == \E r \in RM :
      \/ Prepare(r)
      \/ /\ rmState[r] = "prepared" /\ canCommit /\ rmState' = [rmState EXCEPT ![r] = "committed"]
DirectCommit == TCNext \/ (\E r \in RM : rmState' = [rmState EXCEPT ![r] = "committed"])
====
'''


def prepare_edits(original, output, count):
    """Reproduce the measured source/configuration edits in a new directory."""
    if not 1 <= count <= 8192:
        raise ValueError("expected 1 to 8192 managers")
    cfg = (original / "TCommit.cfg").read_text()
    domain = "{r1, r2, r3}"
    spec = "SPECIFICATION TCSpec"
    if cfg.count(domain) != 1 or cfg.count(spec) != 1:
        raise ValueError("unexpected original TCommit configuration")
    cfg = cfg.replace(domain, "{" + ", ".join(f"r{i+1}" for i in range(count)) + "}")
    output.mkdir()
    (output / "TCommit.tla").write_bytes((original / "TCommit.tla").read_bytes())
    (output / "Edits.tla").write_text(EDITS)
    configs = {
        "original": cfg,
        "stronger": cfg + "\nINVARIANT NoWorkingCommit\n",
        "false": cfg + "\nINVARIANT NonePrepared\n",
        "deletion": cfg.replace(spec, "INIT TCInit\nNEXT WithoutAbort"),
        "addition": cfg.replace(spec, "INIT TCInit\nNEXT DirectCommit"),
    }
    for name, source in configs.items():
        (output / f"{name}.cfg").write_text(source)


if __name__ == "__main__":
    import argparse
    from pathlib import Path

    parser = argparse.ArgumentParser(description=prepare_edits.__doc__)
    parser.add_argument("original", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("managers", type=int)
    args = parser.parse_args()
    prepare_edits(args.original, args.output, args.managers)
