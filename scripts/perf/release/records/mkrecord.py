#!/usr/bin/env python3
"""Assemble one release record: common fields + rows.

mkrecord.py <measurement-id> <spec.json> <out.json>
spec.json: {"measurement", "anchor", "command", "rows", "raw": {...}, "deviations": [...], "notes": [...], extra...}
"""
import json
import os
import sys

_, mid, spec_path, out = sys.argv
spec = json.load(open(spec_path))
HOST = {"instance": "c7i.metal-24xl, us-east-1", "cpu": "Intel Xeon Platinum 8488C", "smt": "off", "turbo": "off",
        "frequency_khz": 2400000, "governor": "performance", "numa_nodes": 1, "cpus": 48,
        "binding": "numactl --cpunodebind=0 --membind=0 (all 48 CPUs of the single NUMA node)",
        "storage": "gp3, 16000 IOPS, 1000 MiB/s", "thp": "madvise", "kernel": "7.0.0-1017-aws",
        "ami": "Ubuntu 24.04.5 LTS (Canonical, x86_64)"}
rec = {
    "measurement": spec["measurement"],
    "plan": "docs/developer/release-performance-plan.md#" + spec["anchor"],
    "date": "2026-11-18",
    "source": "08a14fbac83ff71a3247623fb10bf0584bc83be5",
    "dive_binary_sha256": "97675f9d446dbd188684f9f519e1313a86ae7e7faf57a7964d42d1115498e014",
    "oracle": "tlc-2026.10.06.014338",
    "corpus": "examples@6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f",
    "host": dict(HOST, **spec.get("host", {})),
    "command": spec["command"],
    "raw": spec["raw"],
    "deviations": spec.get("deviations", []),
}
for k, v in spec.items():
    if k not in rec and k not in ("anchor", "host", "rows"):
        rec[k] = v
rec["rows"] = spec["rows"]
json.dump(rec, open(out, "w"), indent=1)
open(out, "a").write("\n")
print(out, os.path.getsize(out))
