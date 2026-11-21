"""Repeat a counter-translated segment of a KIP-996 sampled trace for independent replay."""

import argparse
import copy
import json
from pathlib import Path


def without_counter(state):
    values = copy.deepcopy(state["values"])
    counter = values["aux_ctrs"]["record"].pop("prevote_ctr")
    return json.dumps(values, sort_keys=True, separators=(",", ":")), counter


def extend(trace, copies):
    if copies < 1:
        raise ValueError("copies must be positive")
    seen = {}
    for end, state in enumerate(trace["states"]):
        key, after = without_counter(state)
        if key in seen:
            start, before = seen[key]
            if after > before:
                break
        seen.setdefault(key, (end, after))
    else:
        raise ValueError("trace has no segment differing only by an increased prevote_ctr")
    states = trace["states"][:start + 1]
    segment = trace["states"][start + 1:end + 1]
    for repetition in range(copies):
        for original in segment:
            shifted = copy.deepcopy(original)
            shifted["values"]["aux_ctrs"]["record"]["prevote_ctr"] += repetition * (after - before)
            states.append(shifted)
    result = dict(trace, states=states, ending={"kind": "behavior"})
    if "length" in result:
        result["length"] = len(states)
    return result, {"start_index": start, "end_index": end, "entry_counter": before,
                    "increment": after - before, "copies": copies, "states": len(states)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path, help="dive-trace JSON or a trace-dir envelope")
    parser.add_argument("output", type=Path, help="new output file for validate-trace")
    parser.add_argument("--copies", type=int, default=16)
    args = parser.parse_args()
    with args.trace.open() as input_file:
        data = json.load(input_file)
        document, stats = extend(data.get("document", data), args.copies)
    with args.output.open("x") as output_file:
        json.dump(document, output_file, separators=(",", ":"))
    print(json.dumps(stats))
