#!/usr/bin/env python3
"""Measure complete finite certificate transfer through installed Dive and Sail."""

import argparse
from copy import deepcopy
import json
from math import ceil
import os
from pathlib import Path
from statistics import median
from tempfile import TemporaryDirectory

from certify_asyncraft import SOURCE as RAFT_SOURCE, VOTE_GUARD, cfg_for, encode_table, ordinary_cfg
from certify_twophase import SOURCE as TWO_SOURCE, digest
from recomp_property_panel import measured


RAFT_ACTIONS = (
    "RequestVoteAction", "UpdateTermAction", "HandleRequestVoteRequestAction", "HandleRequestVoteResponseAction",
    "BecomeLeaderAction", "ClientRequestAction", "AppendEntriesAction", "AcceptAppendEntriesRequestAppendAction",
    "AcceptAppendEntriesRequestLearnCommitAction", "HandleAppendEntriesResponseAction", "AdvanceCommitIndexAction",
)
TWO_ACTIONS = (
    "TMCommitAction", "TMAbortAction", "TMRcvPreparedAction", "RMPrepareAction", "RMChooseToAbortAction",
    "RMRcvCommitMsgAction", "RMRcvAbortMsgAction",
)
CERTIFICATE = "sail-finite-induction-certificate-v1.json"


def fixtures(args, root, model):
    benchmarks = args.benchmarks
    source = root / (model + ".tla")
    upstream = benchmarks / source.name
    expected = TWO_SOURCE if model == "TwoPhase" else RAFT_SOURCE
    if digest(upstream) != expected:
        raise ValueError("unexpected source revision")
    text = upstream.read_text()
    source.write_text(text)
    if model == "TwoPhase":
        cfg = (benchmarks / "TwoPhase.cfg").read_text() + "\n"
        proposed = "TypeOK"
        actions, drop = TWO_ACTIONS, {"TMCommitAction", "RMPrepareAction"}
        before, after = '/\\ tmState = "init"', '/\\ tmState = "init" /\\ rm \\notin tmPrepared'
        edits = text.split("TMRcvPrepared(rm) ==", 1)
        imported = edits[0] + "TMRcvPrepared(rm) ==" + edits[1].replace(before, after, 1)
        small = cfg.replace("{rm1,rm2,rm3}", "{rm1,rm2}")
    else:
        cfg = ordinary_cfg(cfg_for(benchmarks / "AsyncRaft.cfg", 3))
        proposed = "H_CurrentTermAtLeastAsLargeAsLogTerms"
        actions, drop = RAFT_ACTIONS, {"ClientRequestAction"}
        before, after = '/\\ state[i] = Leader', '/\\ state[i] = Leader /\\ Len(log[i]) = 0'
        edits = text.split("ClientRequest(i) ==", 1)
        imported = edits[0] + "ClientRequest(i) ==" + edits[1].replace(before, after, 1)
        small = ordinary_cfg(cfg_for(benchmarks / "AsyncRaft.cfg", 2))
    assert text != imported
    module = root / "Transfer.tla"
    restricted = "\n    \\/ " + "\n    \\/ ".join(action for action in actions if action not in drop)
    source_text = f"---- MODULE Transfer ----\nEXTENDS {model}\nRestrictedNext == {restricted}\n"
    seed = None
    if args.certificates:
        proof = json.loads((args.certificates / f"{model}-3.json").read_text())
        manifest = proof["manifest"]
        if manifest["source"] != expected or manifest["actions"] != list(actions):
            raise ValueError("producer certificate has a different source or inventory")
        if model == "TwoPhase":
            # The independent producer checked this formula's cardinality, initiality
            # and closure. Bind the same formula on the transferred predicate.
            learned = r" /\ ".join(f"({clause})" for clause in proof["conjunction"])
            source_text += fr"LearnedConjunction == TypeOK /\ {learned}" + "\n"
            cfg += "INVARIANT LearnedConjunction\n"
            small += "INVARIANT LearnedConjunction\n"
        columns, codes = encode_table(proof["rows"])
        resolved = [{k: action[k] for k in ("name", "bindings", "label")} for action in proof["resolved_actions"]]
        seed = root / "from-producer.json"
        seed.write_text(json.dumps({"format": "dive-finite-induction-certificate-v1",
                                    "variables": manifest["variables"], "actions": resolved,
                                    "boundary": manifest["boundary"],
                                    "predicate": {"columns": columns, "inside": [[i - 1 for i in row] for row in codes],
                                                  "boundary": []}}, separators=(",", ":")))
    module.write_text(source_text + "====\n")
    variants = {
        "base": cfg, "property": cfg + f"INVARIANT {proposed}\n",
        "restricted": cfg.replace("NEXT Next", "NEXT RestrictedNext"), "wrong_carrier": small,
    }
    for name, config in variants.items():
        (root / f"{name}.cfg").write_text(config)
    return module, source, text, imported, variants, seed


class Panel:
    def __init__(self, args, root, model):
        self.args = args
        self.root = root
        self.model = model
        self.module, self.source, self.original, self.import_edit, self.configs, self.seed = fixtures(args, root, model)
        self.key = root / "key"
        with open(self.key, "xb", opener=lambda path, flags: os.open(path, flags, 0o600)) as stream:
            stream.write(os.urandom(32))

    def run(self, command, signed=True, expected=0):
        env = ["env", "-u", "DIVE_SYMBOLIC_PROOF_KEY"]
        if signed:
            env.append(f"DIVE_SYMBOLIC_PROOF_KEY={self.key}")
        with TemporaryDirectory(dir=self.root) as folder:
            work = Path(folder)
            code, wall, data, peak = measured([*env, *command], work, self.args.seconds + 2, self.args.cgroup_parent)
            stderr = (work / "stderr").read_text(errors="replace")[-1600:]
        try:
            doc = json.loads(data)
        except ValueError as error:
            raise ValueError((code, data[-1800:], stderr)) from error
        if code != expected:
            raise ValueError((code, expected, doc.get("proof", doc.get("verdict")),
                              doc.get("failed_obligation", doc.get("reason")), stderr))
        fields = ("proof", "verdict", "reused_obligations", "transition_obligations", "freshly_checked",
                  "authorized_closure", "failed_obligation", "action_inventory")
        return dict(wall_s=wall, memory_peak_bytes=peak, **{k: doc[k] for k in fields if k in doc}), doc

    def dive(self, config, option, artifact, signed=True, expected=0):
        cfg = self.root / (config + ".cfg")
        return self.run([self.args.dive, "check", self.module, "--config", cfg, "--output", "json",
                         "--strategy", "finite-induction", "--time-limit", f"{self.args.seconds}s",
                         option, artifact], signed, expected)[0]

    def sail(self, config, session, signed=True, expected=0):
        cfg = self.root / (config + ".cfg")
        return self.run([self.args.sail, "runtime", "symbolic-check", "--dive", self.args.dive, "--session", session,
                         "--module", self.module, "--config", cfg, "--strategy", "finite-induction",
                         "--time-limit", str(self.args.seconds)], signed, expected)

    def core(self, config, expected=0):
        cfg = self.root / (config + ".cfg")
        measured_run, report = self.run([self.args.dive, "check", self.module, "--config", cfg, "--output", "json",
                                         "--store", "exact", "--workers", "1", "--seed", "0", "--fp", "0",
                                         "--time-limit", f"{self.args.seconds}s", "--exit-codes", "native"],
                                         False, 10 if expected else 0)
        assert report["verdict"] == ("success" if expected == 0 else "invariant-violation")
        return measured_run

    def search(self, config, name):
        return self.sail(config, self.root / name)[0]

    def verify_word(self, result, config):
        assert result["trace_validated"]
        trace = result["trace"]
        cfg = self.root / (config + ".cfg")
        trace_path = self.root / "replay.json"
        command = [self.args.dive, "validate-trace", self.module, "--config", cfg, "--trace", trace_path, "--output", "json"]
        trace_path.write_text(json.dumps(trace))
        assert self.run(command, False)[1]["result"] == "valid"
        damaged = deepcopy(trace)
        damaged["states"][-1]["values"] = damaged["states"][0]["values"]
        trace_path.write_text(json.dumps(damaged))
        assert self.run(command, False, 1)[1]["result"] == "invalid"
        return len(trace["states"])

    def panel(self):
        session = self.root / "base-session"
        artifact = session / CERTIFICATE
        if self.seed is None:
            establishment, original_result = self.sail("base", session)
        else:
            session.mkdir()
            establishment, original_result = self.run(
                [self.args.dive, "check", self.module, "--config", self.root / "base.cfg",
                 "--strategy", "finite-induction", "--output", "json", "--time-limit", f"{self.args.seconds}s",
                 "--verify-predicate", self.seed, "--write-predicate", artifact]
            )
        assert establishment["proof"] == "inductive-safety"
        original = json.loads(artifact.read_text())
        if self.seed and self.model == "TwoPhase":
            assert any(item["name"] == "LearnedConjunction" and item["source_tokens"]
                       for item in original["producer_targets"])
        size = artifact.stat().st_size
        assert original_result["action_inventory"] == [row["name"] for row in original["actions"]]
        actions = establishment["transition_obligations"]
        samples = []
        # Each operation has a fresh OS process and an isolated process-family memory measurement.
        for repetition in range(self.args.rounds):
            ops = {
                "cold_certify": lambda: self.search("property", f"cold-{repetition}"),
                "unsigned_revalidate": lambda: self.dive("property", "--verify-predicate", artifact, False),
                "authorized_sail": lambda: self.sail("property", session)[0],
                "ordinary_core": lambda: self.core("property"),
            }
            row = {name: ops[name]() for name in list(ops)[::1 if repetition % 2 == 0 else -1]}
            assert row["authorized_sail"]["reused_obligations"] == actions
            assert row["unsigned_revalidate"]["reused_obligations"] == 0
            samples.append(row)
        median_time = {mode: median(row[mode]["wall_s"] for row in samples) for mode in samples[0]}
        saved = median_time["cold_certify"] - median_time["authorized_sail"]
        base = self.sail("base", session)[0]
        assert base["reused_obligations"] == actions
        narrowed = self.sail("restricted", session)[0]
        assert narrowed["reused_obligations"] == 0 and narrowed["transition_obligations"] < actions
        restricted_session = self.root / "restricted"
        self.sail("restricted", restricted_session)
        before_expansion = self.dive("base", "--verify-predicate", restricted_session / CERTIFICATE, expected=2)
        assert "action closure" in before_expansion["failed_obligation"]
        expanded = self.sail("base", restricted_session)[0]
        assert expanded["reused_obligations"] == 0 and expanded["transition_obligations"] == actions
        self.source.write_text(self.import_edit)
        import_changed = self.sail("base", session)[0]
        assert import_changed["reused_obligations"] == 0
        import_core = self.core("base")
        self.source.write_text(self.original)
        restored = self.sail("base", session)[0]
        assert restored["reused_obligations"] == actions
        edit = {
            "unchanged": base, "property": samples[0]["authorized_sail"],
            "restriction": narrowed, "restriction_cold_core": self.core("restricted"),
            "expansion_previous_candidate": before_expansion, "expansion": expanded,
            "expansion_cold_core": self.core("base"), "import_edit": import_changed, "import_edit_cold_core": import_core,
            "undo": restored, "undo_cold_core": self.core("base"),
        }
        controls = {"wrong_carrier": self.dive("wrong_carrier", "--verify-predicate", artifact, expected=2)}
        for name in ("clause", "missing_action", "seal"):
            mutated = deepcopy(original)
            if name == "clause":
                mutated["predicate"]["inside"].pop(0)
            elif name == "missing_action":
                mutated["actions"].pop()
            else:
                mutated["seal"] = "0" * 64
            path = self.root / (name + ".json")
            path.write_text(json.dumps(mutated, separators=(",", ":")))
            expected = 0 if name == "seal" else 2
            controls[name] = self.dive("base", "--verify-predicate", path, expected=expected)
        assert "Init inclusion" in controls["clause"]["failed_obligation"]
        assert "action inventory" in controls["missing_action"]["failed_obligation"]
        assert controls["seal"]["reused_obligations"] == 0
        if self.model == "AsyncRaft":
            assert self.original.count(VOTE_GUARD) == 1
            self.source.write_text(self.original.replace(VOTE_GUARD, r"/\ TRUE"))
            mutation, violated = self.sail("base", session, expected=1)
            mutation["independently_replayed_states"] = self.verify_word(violated, "base")
            mutation["cold_core"] = self.core("base", 1)
            self.source.write_text(self.original)
            controls["reachable_vote_mutation"] = mutation

        return {
            "model": self.model, "source_sha256": digest(self.source), "cfg": self.configs, "establishment": establishment,
            "exported_producer_candidate": self.seed is not None,
            "producer_targets": original["producer_targets"],
            "artifact_bytes": size, "admitted_rows": len(original["predicate"]["inside"]),
            "boundary_rows": len(original["predicate"]["boundary"]), "samples": samples, "median_wall_s": median_time,
            "break_even_including_establishment": ceil(establishment["wall_s"] / saved) if saved > 0 else None,
            "sequence": edit, "controls": controls,
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("benchmarks", type=lambda v: Path(v).resolve())
    parser.add_argument("--dive", required=True, type=lambda v: Path(v).resolve())
    parser.add_argument("--sail", required=True, type=lambda v: Path(v).resolve())
    parser.add_argument("--seconds", type=int, default=60)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--model", choices=("TwoPhase", "AsyncRaft"))
    parser.add_argument("--certificates", type=Path, help="independently certified producer outputs")
    parser.add_argument("--cgroup-parent", type=Path)
    args = parser.parse_args()
    if args.rounds <= 0 or args.seconds <= 0:
        parser.error("positive repetitions and deadline required")
    models = (args.model,) if args.model else ("TwoPhase", "AsyncRaft")
    for model in models:
        with TemporaryDirectory() as temporary:
            print(json.dumps(Panel(args, Path(temporary), model).panel()), flush=True)


if __name__ == "__main__":
    main()
