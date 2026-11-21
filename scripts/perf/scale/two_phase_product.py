"""Count the pinned TwoPhase specification and admitted edits to its seven action bodies."""

import argparse
from dataclasses import dataclass, field
from hashlib import sha256
import json
from pathlib import Path
import re

from phase_product import explore, mixed_states

PREFIX = "ae0fa15d9ba08cdbd1b1a49556fcc98b971880dfeefdcfa5b5384b75b6572ac5"
SUFFIX = "82998fd5fbc50016c7ab16b2f933dfe402348f4687831bf173fad7096537a48b"
TCOMMIT = "98974d8c5ee007f4f62188737c20c7dcdb05af099cc53a59c8b8651331fd925d"
ACTIONS = ("TMRcvPrepared(rm)", "TMCommit", "TMAbort", "RMPrepare(rm)",
           "RMChooseToAbort(rm)", "RMRcvCommitMsg(rm)", "RMRcvAbortMsg(rm)")
VARIABLES = {"rmState", "tmState", "tmPrepared", "msgs"}
STATUS = r'"(working|prepared|aborted|committed)"'
COORD = r'"(init|aborted|committed)"'
TAG = r'\[type \|-> "(Commit|Abort)"\]'
PREPARED = '[type |-> "Prepared", rm |-> rm]'
BITS = {"Commit": 1, "Abort": 2}


def require(value, reason):
    if not value:
        raise ValueError(reason)


@dataclass
class Rule:
    owner: bool
    guards: list = field(default_factory=list)
    updates: dict = field(default_factory=dict)
    all_logged: bool = False

    def assign(self, var, value):
        require(var not in self.updates, "repeated assignment to " + var)
        self.updates[var] = value

    def clause(self, expr):
        if expr in ("TRUE", "FALSE"):
            self.guards.append(("constant", expr == "TRUE"))
        elif match := re.fullmatch("tmState = " + COORD, expr):
            self.guards.append(("tm", match[1]))
        elif match := re.fullmatch(TAG + r" \\in msgs", expr):
            self.guards.append(("broadcast", BITS[match[1]]))
        elif self.owner and (match := re.fullmatch(r"rmState\[rm\] = " + STATUS, expr)):
            self.guards.append(("rm", match[1]))
        elif self.owner and expr == PREPARED + r" \in msgs":
            self.guards.append(("prepared", True))
        elif not self.owner and expr == "tmPrepared = RM":
            self.all_logged = True
        elif self.owner and expr == r"tmPrepared' = tmPrepared \cup {rm}":
            self.assign("tmPrepared", True)
        elif self.owner and (match := re.fullmatch(r"rmState' = \[rmState EXCEPT !\[rm\] = " + STATUS + r"\]", expr)):
            self.assign("rmState", match[1])
        elif self.owner and expr == r"msgs' = msgs \cup {" + PREPARED + "}":
            self.assign("msgs", True)
        elif not self.owner and (match := re.fullmatch("tmState' = " + COORD, expr)):
            self.assign("tmState", match[1])
        elif not self.owner and (match := re.fullmatch(r"msgs' = msgs \\cup \{" + TAG + r"\}", expr)):
            self.assign("msgs", BITS[match[1]])
        elif match := re.fullmatch(r"UNCHANGED <<([A-Za-z]+(?:, [A-Za-z]+)*)>>", expr):
            for var in match[1].split(", "):
                require(var in VARIABLES, "unknown variable in frame")
                self.assign(var, None)
        else:
            raise ValueError("unsupported action clause " + expr)

    def enabled(self, phase, value):
        tm, bits = phase
        rm, prepared, _ = value
        observed = {"tm": tm, "rm": rm, "constant": True, "prepared": prepared}
        return all(bool(bits & expected) if kind == "broadcast" else observed[kind] == expected
                   for kind, expected in self.guards)

    def local_target(self, value):
        rm, prepared, logged = value
        return (self.updates["rmState"] or rm, prepared or bool(self.updates["msgs"]),
                logged or bool(self.updates["tmPrepared"]))

    def global_target(self, phase):
        tm, bits = phase
        return self.updates["tmState"] or tm, bits | (self.updates["msgs"] or 0)


def read_actions(source):
    prefix, start, rest = source.partition(b"TMRcvPrepared(rm) ==")
    body, end, suffix = rest.partition(b"TPNext ==")
    require(start and end and sha256(prefix).hexdigest() == PREFIX
            and sha256(end + suffix).hexdigest() == SUFFIX,
            "unsupported change to declarations, type, Init, Next, Spec or theorems")
    rules = {}
    current = None
    for line in (start + body).decode("ascii").split("\n"):
        stripped = line.strip(" ")
        if not stripped:
            continue
        if len(stripped) >= 4 and stripped.startswith("(*") and stripped.endswith("*)") and not any(
                token in stripped[2:-2] for token in ("(*", "*)")):
            continue
        if line.rstrip().endswith(" =="):
            name = line.rstrip()[:-3]
            require(name in ACTIONS and name not in rules, "altered action signature")
            current = rules[name] = Rule(name.endswith("(rm)"))
            continue
        require(current is not None and line.startswith("  /\\ "), "unsupported action layout")
        current.clause(line[5:].rstrip(" "))
    require(tuple(rules) == ACTIONS, "altered action definitions")
    require(all(rule.updates.keys() == VARIABLES for rule in rules.values()),
            "each action must assign or frame every variable once")
    return list(rules.values())


def owner_count(config):
    match = re.fullmatch(
        r"CONSTANT RM = \{(r[1-9][0-9]*(?:, r[1-9][0-9]*)*)\}\n"
        r"INVARIANT TPTypeOK\nSPECIFICATION TPSpec\n\s*", config)
    require(match, "expected the original CFG with a finite untyped participant set")
    ids = match[1].split(", ")
    require(len(ids) == len(set(ids)), "duplicate owner identifier")
    return len(ids)


def check(path, cfg):
    require(path.name == "TwoPhase.tla" and
            sha256(path.with_name("TCommit.tla").read_bytes()).hexdigest() == TCOMMIT,
            "expected pinned TwoPhase module and TCommit dependency")
    rules = read_actions(path.read_bytes())
    owners = owner_count(cfg.read_text())
    initial = ("working", False, False)

    def local(phase, value):
        return [r.local_target(value) for r in rules if r.owner and r.enabled(phase, value)]

    def global_edges(phase):
        for rule in rules:
            if not rule.owner and rule.enabled(phase, initial):
                yield rule.global_target(phase), lambda value, logged=rule.all_logged: not logged or value[2]

    result = explore(owners, ("init", 0), initial, local, global_edges)
    result["owners"] = owners
    result["invariant"] = "TPTypeOK"
    result["mixed_committed_aborted"] = sum(mixed_states(p["local_values"], owners, "committed", "aborted")
                                             for p in result["phases"])
    return result


def compact(value):
    """Preserve small integers. Large counts include an exact-expression recipe in the phase rows."""
    if isinstance(value, int) and value.bit_length() > 3000:
        return {"bits": value.bit_length(),
                "sha256_unsigned_big_endian": sha256(value.to_bytes((value.bit_length()+7)//8, "big")).hexdigest()}
    if isinstance(value, dict):
        return {k: compact(v) for k, v in value.items()}
    if isinstance(value, list):
        return [compact(v) for v in value]
    return value


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path)
    parser.add_argument("config", type=Path)
    args = parser.parse_args()
    try:
        print(json.dumps(compact(check(args.module, args.config)), sort_keys=True))
    except (ValueError, OSError) as error:
        parser.exit(2, f"unsupported: {error}\n")
