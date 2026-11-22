#!/usr/bin/env python3
"""Construct a candidate MultiCarElevator lasso for a larger floor configuration.

The original check's trace and a strictly increasing map of the old floors are
the inputs. Validate the output on the new TLA+ module and CFG with dive
validate-trace. Changes to dispatch choices or fairness can invalidate it.
"""

import argparse
import json
from copy import deepcopy
from pathlib import Path


def map_state(original, floor_map):
    state = deepcopy(original)
    values = state["values"]
    if state.get("action") is not None or set(values) != {
        "PersonState", "ElevatorState", "ActiveElevatorCalls"
    }:
        raise ValueError("expected an unlabeled MultiCarElevator state")

    def floor(number):
        if type(number) is not int or not 1 <= number <= len(floor_map):
            raise ValueError(f"unknown original floor {number!r}")
        return floor_map[number - 1]

    for _, person in values["PersonState"]["fcn"]:
        record = person["record"]
        record["destination"] = floor(record["destination"])
        if type(record["location"]) is int:
            record["location"] = floor(record["location"])
    for _, car in values["ElevatorState"]["fcn"]:
        record = car["record"]
        record["floor"] = floor(record["floor"])
        buttons = record["buttonsPressed"]["set"]
        record["buttonsPressed"]["set"] = [floor(number) for number in buttons]
    for call in values["ActiveElevatorCalls"]["set"]:
        record = call["record"]
        record["floor"] = floor(record["floor"])
    return state


def moving_car(before, after):
    """Identify a one-floor move that preserved everything except one car's floor."""
    a, b = before["values"], after["values"]
    if any(a[name] != b[name] for name in ("ActiveElevatorCalls", "PersonState")):
        return None
    cars = zip(a["ElevatorState"]["fcn"], b["ElevatorState"]["fcn"], strict=True)
    changed = [(i, x, y) for i, (x, y) in enumerate(cars) if x != y]
    if len(changed) != 1:
        return None
    index, (old_key, old), (new_key, new) = changed[0]
    x, y = old["record"], new["record"]
    if old_key != new_key or x["direction"] == "Stationary":
        return None
    if abs(x["floor"] - y["floor"]) != 1 or {**x, "floor": y["floor"]} != y:
        return None
    return index


def lift(word, floor_map):
    if len(floor_map) < 2 or floor_map[0] != 1 or any(
        a >= b for a, b in zip(floor_map, floor_map[1:])
    ):
        raise ValueError("floor map must start at 1 and increase strictly")
    states = word["states"]
    loop = word["ending"]["back_to"]
    if (word["format"], word["version"], word["ending"]["kind"]) != ("dive-trace", 1, "lasso"):
        raise ValueError("expected a version 1 dive lasso")
    if len(states) != word["length"] or not 0 <= loop < len(states):
        raise ValueError("invalid donor length or loop target")
    mapped = [map_state(state, floor_map) for state in states]
    positions = []
    result = []
    for i, state in enumerate(mapped):
        positions.append(len(result))
        result.append(state)
        target = i + 1 if i + 1 < len(states) else loop
        car = moving_car(states[i], states[target])
        if car is None:
            continue
        first = state["values"]["ElevatorState"]["fcn"][car][1]["record"]["floor"]
        last = mapped[target]["values"]["ElevatorState"]["fcn"][car][1]["record"]["floor"]
        step = 1 if last > first else -1
        for floor in range(first + step, last, step):
            intermediate = deepcopy(state)
            intermediate["values"]["ElevatorState"]["fcn"][car][1]["record"]["floor"] = floor
            result.append(intermediate)
    return {**word, "states": result, "length": len(result),
            "ending": {"kind": "lasso", "back_to": positions[loop]}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path)
    parser.add_argument("floor_map", help="comma-separated new floor for each old floor, starting at 1")
    parser.add_argument("output", type=Path)
    options = parser.parse_args()
    word = lift(json.loads(options.trace.read_text()), [int(x) for x in options.floor_map.split(",")])
    with options.output.open("x") as output:
        json.dump(word, output, separators=(",", ":"))


if __name__ == "__main__":
    main()
