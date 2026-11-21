"""Count the finite reachable graph of the pinned EmergencyCall safety model.

Run with the symbolic worker's Python and PYTHONPATH. The positional inputs are
the original TLA+ module and a safety CFG. The exporter resolves both inputs.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

from induction import write_atomic
from engine import Unsupported, require
from finite_values import FiniteValueModel

SOURCE_SHA = "76236976cf936bd81a8a30e57144ee9ed20b1a3498f9224a55e96147ab339a5e"
STANDARD_SOURCES = {
    "Naturals": "73f2a68412039aa86f37ad08f0d0fef50fa8327d0ad8806f515548c75e9d9d84",
    "Sequences": "f613cffe14133769f0972b6f6e0139aaf0773004a51bac88ada382426e3eb7f4",
    "FiniteSets": "d94cb545256539bf782fc1914842dfe8c19c1d3d4e48b8327dd0c47b74922e1e",
    "TLC": "944ece590eec81ace059881c2a62064c06041efa81539a788cd40fa7d1da41ce",
}
# Extra cases change a single finite set literal. Identifying them by the
# complete source digest keeps the initial-witness argument tied to those edits.
VARIANTS = {
    "published": SOURCE_SHA,
    "ordinary-1": "4f9be5695f4c713ca445ff765fe341d559cd70f350374252cd3e9d0b521d180a",
    "ordinary-2": "1c730ad1c2a34bc0de48767a482fdadf657a69a90a73f13fd068deb05ed47d36",
    "local-1": "d6c2291f6747d4614e829b9282cdc18a406741399a4cf8fa420a9624647dba8e",
    "local-2": "6a14b5eea015ac557b8267dab6eb02d10af410ce141aa64c3f9a85ae62232f77",
}
PARTITION = (
    "amf_0_normal_attach_accept_include_enl",
    "amf_1_normal_attach_accept_include_enl",
    "amf_0_emergency_attach_accept_include_enl",
    "amf_1_emergency_attach_accept_include_enl",
    "amf_home_reject_or_allow_unmarked_emergency_request",
    "amf_roam_reject_or_allow_unmarked_emergency_request",
)


class PhoneModel(FiniteValueModel):
    """Published reference with its six named partition dimensions."""

    def __init__(self, document):
        layout = next(d["body"] for d in document["ir"]["defs"] if d["name"] == "TypeInvariant")
        super().__init__(document, layout)

    def check(self, cell, keep_trace=False):
        values = () if cell is None else [
            (self.names.index(name), (cell >> bit) & 1) for bit, name in enumerate(PARTITION)
        ]
        return self.check_values(values, keep_trace) | {"cell": cell}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path)
    parser.add_argument("config", type=Path)
    parser.add_argument("--exporter", type=Path, required=True)
    parser.add_argument("--variant", choices=VARIANTS, default="published")
    parser.add_argument("--cell", type=int, choices=range(64))
    parser.add_argument(
        "--trace",
        type=Path,
        help="save a validated shortest counterexample from the selected initial set",
    )
    parser.add_argument(
        "--validator", type=Path, help="dive executable for validate-trace"
    )
    args = parser.parse_args()
    if bool(args.validator) != bool(args.trace):
        parser.error("supply --trace and --validator together")
    if args.trace and any(
        args.trace.resolve() == source.resolve()
        for source in (args.module, args.config, args.exporter, args.validator)
    ):
        parser.error("the trace destination must be separate from the inputs")
    require(
        hashlib.sha256(args.module.read_bytes()).hexdigest() == VARIANTS[args.variant],
        "unrecognized EmergencyCall source",
    )
    bound = json.loads(
        subprocess.check_output([args.exporter.resolve(), args.module, args.config])
    )
    expected = STANDARD_SOURCES | {"EmergencyCall_Model_auto": VARIANTS[args.variant]}
    files = bound["ir"]["files"]
    require(
        len(files) == len(expected)
        and all(item["standard"] == (item["module"] in STANDARD_SOURCES) for item in files)
        and {
            item["module"]: hashlib.sha256(item["source"].encode()).hexdigest()
            for item in files
        } == expected,
        "unrecognized bound EmergencyCall module or import",
    )
    model = PhoneModel(bound)
    row = model.check(args.cell, keep_trace=args.trace is not None)
    trace = row.pop("trace", None)
    if trace is not None:
        data = json.dumps(trace)
        checked = subprocess.run(
            [
                args.validator.resolve(),
                "validate-trace",
                args.module,
                "--config",
                args.config,
                "--trace",
                "/dev/stdin",
                "--compat=tlc",
                "--fp",
                "0",
                "--seed",
                "0",
                "--output",
                "json",
            ],
            input=data,
            text=True,
            capture_output=True,
        )
        require(
            checked.returncode == 0 and json.loads(checked.stdout)["result"] == "valid",
            "independent source trace validation failed",
        )
        write_atomic(args.trace, (data + "\n").encode())
        row.update(trace_validated=True, trace_file=str(args.trace))
    print(json.dumps(dict(variant=args.variant, **row)))
    return int(trace is not None)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (Unsupported, KeyError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(f"unsupported: {error}")
