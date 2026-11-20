#!/usr/bin/env python3
"""Differential checks and edit controls for the source-driven symbolic checker."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib

from test_fragment import check_fragment

SCRIPT = Path(__file__).resolve().with_name("check.py")
ROOT = SCRIPT.parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kvs", type=Path, nargs="?", help="pinned Examples KeyValueStore directory")
    parser.add_argument("--large", action="store_true")
    parser.add_argument("--growth", action="store_true", help="expand the actual Medium key domain")

    parser.add_argument("--dive", default="target/release/dive")
    parser.add_argument("--exporter", default="target/release/examples/symbolic_input")
    args = parser.parse_args()
    if args.kvs is None:
        pins = tomllib.loads((ROOT / "conformance/corpora.toml").read_text())
        commit = next(item["commit"] for item in pins["corpus"] if item["id"] == "examples")
        cache = Path(os.environ.get("DIVE_CACHE_DIR", Path.home() / ".cache/dive"))
        args.kvs = cache / "git/examples" / commit / "specifications/KeyValueStore"
    runner = [sys.executable, SCRIPT, "--dive", args.dive, "--exporter", args.exporter]

    def run(module, config, *flags, code=0):
        proc = subprocess.run([*runner, module, config, *flags], text=True, capture_output=True)
        if proc.returncode != code:
            raise RuntimeError((proc.returncode, proc.stdout, proc.stderr))
        rows = [json.loads(s) for s in proc.stdout.splitlines()]
        for row in rows:
            print(config.name, row, flush=True)
        return rows

    def explicit(module, config):
        proc = subprocess.run(
            [
                args.dive,
                "check",
                module,
                "--config",
                config,
                "--store",
                "exact",
                "--compat=tlc",
                "--seed",
                "0",
                "--fp",
                "0",
                "--workers",
                "1",
                "--output",
                "json",
            ],
            text=True,
            capture_output=True,
        )
        assert proc.stdout, proc.stderr
        return json.loads(proc.stdout)

    def compare(row, other):
        stats = other["statistics"]
        assert (row["verdict"], row["distinct"], row["generated"], row["depth"]) == (
            other["verdict"],
            stats["distinct"],
            stats["generated"],
            stats["depth"],
        )

    module = args.kvs / "MCKVS.tla"
    goldens = ROOT / "conformance/golden/tlc-2026.10.06.014338/examples/specifications/KeyValueStore"
    with tempfile.TemporaryDirectory() as temp:
        folder = Path(temp)
        check_fragment(folder, run, explicit, compare)
        edited = folder / "MCKVS.tla"
        edited.write_text(
            module.read_text().replace(
                "====",
                r"""Clean == \A t \in TxId \ tx : missed[t] = {}
NoWrites == \A t \in tx : written[t] = {}
OnlyOpenOrRollback == \E t \in TxId : OpenTx(t) \/ RollbackTx(t)
====""",
            )
        )
        (folder / "KeyValueStore.tla").write_bytes((args.kvs / "KeyValueStore.tla").read_bytes())
        for size in ["Small", "Medium", *(["Large"] if args.large else [])]:
            cfg = args.kvs / f"MCKVSSafety{size}.cfg"
            clean = folder / "clean.cfg"
            clean.write_text(cfg.read_text() + "\nINVARIANT Clean\n")
            bad = folder / "bad.cfg"
            bad.write_text(cfg.read_text() + "\nINVARIANT NoWrites\n")
            trace = folder / "witness.json"
            predicate = folder / "reached.json"
            row, holds, fails = run(
                module,
                cfg,
                "--write-predicate",
                predicate,
                "--recheck",
                edited,
                clean,
                "--recheck",
                edited,
                bad,
                "--trace",
                trace,
                code=1,
            )
            assert row["verdict"] == "success" and not row["reused"] and holds["reused"]
            assert holds["distinct"] == row["distinct"] and holds["generated"] == row["generated"]
            assert fails["reused"] and fails["trace_validated"] and fails["depth"] == 3
            assert fails["ending"] == {"kind": "invariant", "name": "NoWrites"}
            if size == "Large":
                assert (row["distinct"], row["generated"], row["depth"]) == (594616815, 13385471536, 30)
            else:
                result = json.loads((goldens / (cfg.name + ".json")).read_text())["result"]
                counts = result["counts"]
                assert (row["verdict"], row["distinct"], row["generated"], row["depth"]) == (
                    result["verdict"],
                    counts["distinct_states"],
                    counts["states_generated"],
                    counts["depth"],
                )

            # Each CLI invocation gets a fresh manager and rebinds the source.
            (proof,) = run(edited, clean, "--verify-predicate", predicate)
            assert proof["proof"] == "inductive-safety"
            (refused,) = run(edited, bad, "--verify-predicate", predicate, code=2)
            assert refused["proof"] == "inconclusive" and refused["failed_obligation"] == "NoWrites"
            (proof,) = run(module, cfg, "--prove-inductive")
            assert proof["proof"] == "inductive-safety"
            (refused,) = run(edited, bad, "--prove-inductive", code=2)
            assert refused["failed_obligation"] == "Next closure"
            corrupt = folder / "empty.json"
            corrupt.write_text(json.dumps(dict(format="finite-state-predicate-v1", nodes=[], root=-1)))
            (refused,) = run(module, cfg, "--verify-predicate", corrupt, code=2)
            assert refused["failed_obligation"] == "Init inclusion"

            # Skip the Open step so that real validation must reject the trace.
            witness = json.loads(trace.read_text())
            del witness["states"][1]
            trace.write_text(json.dumps(witness))
            invalid = subprocess.run(
                [
                    args.dive,
                    "validate-trace",
                    edited,
                    "--config",
                    bad,
                    "--trace",
                    trace,
                    "--compat=tlc",
                    "--output",
                    "json",
                ],
                capture_output=True,
                text=True,
            )
            assert json.loads(invalid.stdout)["result"] == "invalid"

        cfg = args.kvs / "MCKVSSafetyMedium.cfg"
        changed = folder / "changed.cfg"
        changed.write_text(
            cfg.read_text().replace("SPECIFICATION\n    Spec", "INIT Init\nNEXT OnlyOpenOrRollback")
        )
        run(module, cfg, "--write-predicate", predicate)
        (proof,) = run(edited, changed, "--verify-predicate", predicate)
        assert proof["proof"] == "inductive-safety"
        row, narrower = run(module, cfg, "--recheck", edited, changed)
        assert not narrower["reused"] and narrower["distinct"] < row["distinct"]
        compare(narrower, explicit(edited, changed))

        # Modify an imported action while keeping the entry point and cfg bytes identical.
        imported = folder / "disable-open"
        imported.mkdir()
        loader = imported / module.name
        loader.write_bytes(module.read_bytes())
        source = (args.kvs / "KeyValueStore.tla").read_text()
        guard = r"/\ t \notin tx"
        assert source.count(guard) == 1
        dependency = imported / "KeyValueStore.tla"
        dependency.write_text(source.replace(guard, r"/\ FALSE /\ t \notin tx"))
        _, dead = run(module, cfg, "--recheck", loader, cfg, code=1)
        assert dead["ending"] == {"kind": "deadlock"} and dead["depth"] == 1
        assert not dead["reused"] and dead["trace_validated"]
        dependency.write_text(
            source.replace(
                "]_<<store, tx, snapshotStore, written, missed>>", "]_<<store, tx, snapshotStore, written>>"
            )
        )
        (omitted,) = run(loader, cfg, "--prove-inductive", code=2)
        assert omitted["verdict"] == "unsupported" and "subscript" in omitted["reason"]

        if args.growth:
            from kvs_symbolic import transaction_history_counts

            for keys in (4, 5):
                wider = folder / f"keys-{keys}.cfg"
                key_names = ", ".join(f"k{i + 1}" for i in range(keys))
                wider.write_text(cfg.read_text().replace("k1, k2, k3", key_names))
                _, row = run(module, cfg, "--recheck", module, wider)
                assert not row["reused"] and (
                    row["distinct"],
                    row["generated"],
                ) == transaction_history_counts(keys, 3, 2, True)
            # The proof path also works above the exact minterm count limit.
            wider.write_text(cfg.read_text().replace("k1, k2, k3", ", ".join(f"k{i + 1}" for i in range(16))))
            (proof,) = run(module, wider, "--prove-inductive")
            assert proof["proof"] == "inductive-safety"

            # Separate program growth from state-space growth. All new definitions
            # are reachable from the new invariant; the transition is unchanged.
            library = folder / "Checks.tla"
            check_names = [f"Check{i}" for i in range(1024)]
            library.write_text(
                "\n".join(
                    [
                        "---- MODULE Checks ----",
                        "EXTENDS KeyValueStore",
                        *(name + r" == \A t \in TxId \ tx : missed[t] = {}" for name in check_names),
                        "AdditionalChecks ==",
                        *("    /\\ " + name for name in check_names),
                        "====",
                    ]
                )
            )
            edited.write_text(module.read_text().replace("EXTENDS KeyValueStore, TLC", "EXTENDS Checks, TLC"))
            grown = folder / "extra-checks.cfg"
            grown.write_text(cfg.read_text() + "\nINVARIANT AdditionalChecks\n")
            row, larger_program = run(module, cfg, "--recheck", edited, grown)
            assert larger_program["reused"] and larger_program["distinct"] == row["distinct"]
        # An unrelated program exercises two cyclically dependent output fields,
        # an unused choice (multiplicity), and inference without KVS field names.
        ring = folder / "Pair.tla"
        ring.write_text(r"""---- MODULE Pair ----
CONSTANTS Color, First, Second
VARIABLES current, prior
Bounds == current \in Color /\ prior \in Color
Init == current = First /\ prior = Second
Next == \E unused \in Color : current' = prior /\ prior' = current
====
""")
        config = folder / "Pair.cfg"
        config.write_text(
            "CONSTANTS Color = {red, blue} First = red Second = blue\nINIT Init\nNEXT Next\nINVARIANT Bounds\n"
        )
        (row,) = run(ring, config, "--type-invariant", "Bounds")
        compare(row, explicit(ring, config))
        # A disabled relation must report an independently validated deadlock.
        original = ring.read_text()
        ring.write_text(original.replace("current' = prior", "FALSE /\\ current' = prior"))
        (dead,) = run(ring, config, "--type-invariant", "Bounds", code=1)
        assert dead["ending"] == {"kind": "deadlock"} and dead["trace_validated"]
        ring.write_text(original)
        # The declared bounds are a storage proposal, never an implicit constraint.
        ring.write_text(ring.read_text().replace("current' = prior", "current' = {prior}"))
        (refused,) = run(ring, config, "--type-invariant", "Bounds", code=2)
        assert refused["verdict"] == "unsupported" and "scalar assignment" in refused["reason"]
        # Declared types cannot constrain an otherwise unbound initial variable.
        ring.write_text(
            original.replace("current = First", r"current = IF current \in Color THEN First ELSE current")
        )
        (refused,) = run(ring, config, "--type-invariant", "Bounds", "--prove-inductive", code=2)
        assert "unbound state read" in refused["reason"]
        swap = folder / "Swap.tla"
        swap.write_text(r"""---- MODULE Swap ----
CONSTANTS K, A, B
VARIABLE f
TypeInvariant == f \in [K -> K]
Init == f = [k \in K |-> k]
Next == f' = [k \in K |-> IF k = A THEN f[B] ELSE f[A]]
====
""")
        swap_cfg = folder / "Swap.cfg"
        swap_cfg.write_text(
            "CONSTANTS K={left,right} A=left B=right\nINIT Init NEXT Next INVARIANT TypeInvariant\n"
        )
        (row,) = run(swap, swap_cfg)
        assert (row["distinct"], row["generated"], row["depth"]) == (2, 3, 2)
        compare(row, explicit(swap, swap_cfg))

        asymmetric = folder / "Asymmetric.tla"
        asymmetric.write_text(r"""---- MODULE Asymmetric ----
EXTENDS TLC
CONSTANTS Full, Partial
VARIABLE f
TypeInvariant == f \in [Partial -> SUBSET {}]
Init == f = [k \in Partial |-> {}]
Next == UNCHANGED f
Sym == Permutations(Full)
====
""")
        config = folder / "Asymmetric.cfg"
        config.write_text(
            "CONSTANTS Full={a,b} Partial={a}\nINIT Init NEXT Next INVARIANT TypeInvariant SYMMETRY Sym\n"
        )
        (refused,) = run(asymmetric, config, code=2)
        assert "asymmetric finite index domain" in refused["reason"]

        receipts = folder / "Receipts.tla"
        receipts.write_text(r"""---- MODULE Receipts ----
CONSTANT IDs
VARIABLE got
TypeInvariant == got \subseteq IDs
Init == got = {}
Next == \/ \E i \in IDs \ got : got' = got \cup {i}
        \/ UNCHANGED got
====
""")
        config = folder / "Receipts.cfg"
        for width in (8, 24, 48):
            names = ", ".join(f"p{i}" for i in range(width))
            config.write_text(f"CONSTANT IDs = {{{names}}}\nINIT Init\nNEXT Next\nINVARIANT TypeInvariant\n")
            (row,) = run(receipts, config)
            total = 1 << width
            assert (row["distinct"], row["generated"], row["depth"]) == (
                total,
                1 + total + width * (total // 2),
                width + 1,
            )
            if width == 8:
                compare(row, explicit(receipts, config))
    print(
        "goldens, relation invalidation, fresh exact search, validated and damaged traces, layout refusal: pass"
    )


if __name__ == "__main__":
    main()
