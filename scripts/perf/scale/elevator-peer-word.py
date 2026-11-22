#!/usr/bin/env python3
"""Translate Manyan's Elevator dumpTrace JSON into an independently checkable named lasso."""
import argparse
import json
from pathlib import Path


def typed_state(state):
    # TLC's JSON loses set/sequence and string/model distinctions. The domains
    # here are the original Elevator module's TypeInvariant.
    def mapping(entries):
        return {"fcn": [[{"model": key}, {"record": rec}] for key, rec in sorted(entries.items())]}

    cars = {key: {**value, "buttonsPressed": {"set": value["buttonsPressed"]}}
            for key, value in state["ElevatorState"].items()}
    people = {}
    for key, value in state["PersonState"].items():
        person = dict(value)
        if isinstance(person["location"], str):
            person["location"] = {"model": person["location"]}
        people[key] = person
    if set(state) != {"ActiveElevatorCalls", "ElevatorState", "PersonState"}:
        raise ValueError("unexpected Elevator variables")
    return {"ActiveElevatorCalls": {"set": [{"record": call} for call in state["ActiveElevatorCalls"]]},
            "PersonState": mapping(people), "ElevatorState": mapping(cars)}


def translate(dump):
    states = dump["counterexample"]["state"]
    actions = dump["counterexample"]["action"]
    if not states or len(states) != len(actions):
        raise ValueError("expected one outgoing edge per supplied lasso state")
    for i, (state, (source, _, target)) in enumerate(zip(states, actions, strict=True)):
        if state[0] != i + 1 or source != state or (i + 1 < len(states) and target != states[i + 1]):
            raise ValueError(f"inconsistent edge at state {i + 1}")
    target = actions[-1][2]
    if not 1 <= target[0] <= len(states) or target != states[target[0] - 1]:
        raise ValueError("loop target is not in the supplied prefix")
    return {"format": "dive-trace", "version": 1, "length": len(states), "variables": dump["vars"],
            "properties": ["TemporalInvariant"], "ending": {"kind": "lasso", "back_to": target[0] - 1},
            "states": [{"action": None, "values": typed_state(state)} for _, state in states]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manyan_dump", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    trace = translate(json.loads(args.manyan_dump.read_text()))
    with args.output.open("x") as out:
        json.dump(trace, out, separators=(",", ":"))
