"""Generate pinned TLC checks of the exact KIP-996 orbit, including original fairness enabledness."""

import argparse
import json
from pathlib import Path
import re

PROPERTIES = ("Fairness", "ValuesEventuallyAcked", "ReconfigurationNotStuck",
              "EventuallyLeaderElected", "NotStuckInProspective")


class Values:
    """Name shared mathematical values once to keep the generated module small."""

    def __init__(self):
        self.names = {}
        self.definitions = []

    def name(self, value):
        key = json.dumps(value, sort_keys=True)
        if key not in self.names:
            literal = self.literal(value)
            if isinstance(value, dict) and "model" not in value:
                name = "OrbitValue" + str(len(self.definitions))
                self.definitions.append(f"{name} == {literal}")
                self.names[key] = name
            else:
                self.names[key] = literal
        return self.names[key]

    def literal(self, value):
        if isinstance(value, bool):
            return str(value).upper()
        if isinstance(value, (int, str)):
            return json.dumps(value)
        if not isinstance(value, dict) or len(value) != 1:
            raise ValueError("unsupported trace value")
        kind, items = next(iter(value.items()))
        if kind == "model" and re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", items):
            return items
        if kind == "seq":
            return "<<" + ", ".join(map(self.name, items)) + ">>"
        if kind == "set":
            return "{" + ", ".join(map(self.name, items)) + "}"
        if kind == "record" and items and all(re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", n) for n in items):
            return "[" + ", ".join(n + " |-> " + self.name(v) for n, v in items.items()) + "]"
        if kind == "fcn":
            return "(" + " @@ ".join(f"({self.name(k)} :> {self.name(v)})" for k, v in items) + ")" if items else "<<>>"
        raise ValueError("unsupported trace value " + kind)


def action_sets(families, enabled):
    parts = []
    for name, pair in families:
        domain = r"AllServers \X AllServers" if pair else "AllServers"
        args = "_p[1], _p[2]" if pair else "_p"
        observation = ("ENABLED " if enabled else "") + f"<<{name}({args})>>_vars"
        parts.append(r"{_p \in " + domain + " : " + observation + "}")
    return "<<" + ", ".join(parts) + ">>"


# The ordered action instances of the fair suffix found by the restricted search.
ELECTION = ("HandleVoteRequest(1, 3)", "VoterElectionTimeout(1)", "HandleVoteRequest(3, 1)",
            "HandleVoteResponse(1, 3)", "ConcludePreVote(1)", "HandleVoteResponse(3, 1)",
            "ConcludePreVote(3)", "VoterElectionTimeout(3)")


def transition(states, back, scheduled, laps=None):
    values = Values()
    rows = [values.name({"record": s["values"]}) for s in states]
    fields = list(states[0]["values"])
    if not 1 <= back <= len(rows) or any(set(s["values"]) != set(fields) for s in states):
        raise ValueError("invalid orbit states or loop index")
    last = back - 1 + len(ELECTION) if scheduled else len(rows)
    if laps is not None:
        last = back + laps * len(ELECTION)
    following = f"IF orbitIndex = {last} THEN {back} ELSE orbitIndex + 1"
    head = [r"EXTENDS kraft_kip_996", "CONSTANTS h1,h2,h3,h4,h5,v1,v2,v3", "VARIABLE orbitIndex",
            *values.definitions, "OrbitRows == <<" + ", ".join(rows) + ">>",
            "OrbitFollowing == " + following, r"OrbitInit == /\ orbitIndex = 1"]
    head += [r"    /\ " + f"{k} = OrbitRows[1].{k}" for k in fields]
    head += [r"    /\ Init", "OrbitRecorded =="]
    head += [r"    /\ " + f"{k}' = OrbitRows[OrbitFollowing].{k}" for k in fields]
    head.append(r"    /\ Next")
    if scheduled:
        head.append("OrbitScheduled == CASE " + "\n    [] ".join(
            f"((orbitIndex - {back}) % {len(ELECTION)}) = {i} -> {act}" for i, act in enumerate(ELECTION)))
    head.append(r"OrbitTransition == /\ orbitIndex' = OrbitFollowing")
    if laps is not None:
        head.append(r"    /\ orbitIndex < " + str(last))
    head.append(r"    /\ " + (f"IF orbitIndex < {back} THEN OrbitRecorded ELSE OrbitScheduled"
                              if scheduled else "OrbitRecorded"))
    return head


def extract(trace, output, quotient=None):
    """Remove only the scheduling variable and wrapper labels from a concrete replay."""
    document = json.loads(trace.read_text())["document"]
    document["variables"].remove("orbitIndex")
    document["ending"] = {"kind": "behavior"}
    document.pop("properties", None)
    for state in document["states"]:
        del state["values"]["orbitIndex"]
        state["action"] = None
    if quotient is not None:
        back = json.loads((quotient / "parity-word.json").read_text())["ending"]["back_to"]
        end = back + len(ELECTION)
        states = document["states"]
        if not 0 <= back < end < len(states):
            raise ValueError("the replay must include a complete election lap and its closing state")
        for state in states[:end + 1]:
            state["values"]["aux_ctrs"]["record"]["prevote_ctr"] %= 2
        if states[back]["values"] != states[end]["values"]:
            raise ValueError("the replay does not close under the counter projection")
        document["states"] = states[:end]
        document["ending"] = {"kind": "lasso", "back_to": back}
        document["properties"] = ["ReconfigurationNotStuck", "EventuallyLeaderElected"]
    with output.open("x") as stream:
        json.dump(document, stream, separators=(",", ":"))


def generate(directory, scheduled, concrete, laps):
    word = json.loads((directory / "parity-word.json").read_text())
    states = word["states"]
    back = word["ending"]["back_to"] + 1
    if concrete:
        if laps < 1 or not scheduled:
            raise ValueError("concrete replay needs the election schedule and at least one lap")
        raw = json.loads(concrete.read_text())
        states = raw.get("document", raw)["states"][:back]
        directory = directory / "concrete"
    elif scheduled:
        states = states[:back]
    define = transition(states, back, scheduled, laps if concrete else None)
    source = (directory / "kraft_kip_996.tla").read_text()
    body = source.split("Fairness ==", 1)[1].split("Spec == Init", 1)[0]
    families = re.findall(r"WF_vars\((\w+)\(s(, peer)?\)\)", body)
    if len(families) != 15:
        raise ValueError("expected all 15 weak fairness families")
    tick = r" /\ WF_orbitIndex(orbitIndex' = OrbitFollowing)"
    config = (directory / "model_liveness.cfg").read_text().split("\nPROPERTY", 1)[0]
    config = config.replace("SPECIFICATION LivenessSpec", "SPECIFICATION OrbitSpec")
    if concrete:
        body = "\n".join(define) + "\n====\n"
        (directory / "KraftConcrete.tla").write_text("---- MODULE KraftConcrete ----\n" + body)
        cfg = config.replace("SPECIFICATION OrbitSpec", "INIT OrbitInit\nNEXT OrbitTransition")
        (directory / "concrete.cfg").write_text(cfg + "\nCHECK_DEADLOCK FALSE\n")
        return
    for label, diag in (("KraftOrbit", False), ("KraftEnabled", True)):
        body = list(define)
        if diag:
            obs = ', '.join(action_sets(families, enabled) for enabled in (True, False))
            body.append(f'    /\\ IF orbitIndex >= {back} THEN PrintT(<<"ORBIT", orbitIndex, {obs}>>) ELSE TRUE')
        body.append(r"OrbitSpec == OrbitInit /\ [][OrbitTransition]_<<vars, orbitIndex>>" + tick)
        (directory / (label + ".tla")).write_text(f"---- MODULE {label} ----\n" + "\n".join(body) + "\n====\n")
    (directory / "enabled.cfg").write_text(config + "\n")
    for prop in PROPERTIES:
        (directory / (prop + ".cfg")).write_text(config + "\nPROPERTY " + prop + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("quotient_directory", type=Path)
    parser.add_argument("--election-schedule", action="store_true", help="run the eight original action instances")
    parser.add_argument("--concrete", type=Path, help="original sample for finite replay in the original module directory")
    parser.add_argument("--laps", type=int, default=16)
    parser.add_argument("--extract", type=Path, help="export a concrete simulation trace from this envelope")
    parser.add_argument("--project-lasso", action="store_true", help="project the extracted election lap for native periodic validation")
    parser.add_argument("--output", type=Path, help="new trace file for independent validation")
    args = parser.parse_args()
    if args.extract and args.output:
        extract(args.extract, args.output, args.quotient_directory if args.project_lasso else None)
    elif args.extract or args.output or args.project_lasso:
        parser.error("--extract and --output must be used together")
    else:
        generate(args.quotient_directory, args.election_schedule, args.concrete, args.laps)
