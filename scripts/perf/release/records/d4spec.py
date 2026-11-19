#!/usr/bin/env python3
"""Wrap d4record.py output as a mkrecord.py spec. d4spec.py <d4.json> <out-spec.json>"""
import json, sys
d = json.load(open(sys.argv[1]))
slush = d["rows"].get("specifications/SlushProtocol/SlushLarge.cfg")
d1_slush_ms = 6544780
extra = {}
if slush:
    extra = {"SlushLarge_with_D1_one_worker_time": {
        "note": "D4's 1-worker SlushLarge runs stop at dive-bench's 3570 s time limit; D1 timed the same binary on the same "
                "host type at 6544.78 s to completion. Speed-ups against that time:",
        "speedup4": round(d1_slush_ms / slush["w4"]["wall_ms_median"], 2),
        "speedup8": round(d1_slush_ms / slush["w8"]["wall_ms_median"], 2)}}
spec = {
    "measurement": "D4", "anchor": "d4-worker-scaling",
    "host": {"binding": "numactl --cpunodebind=0 --membind=0 (all 48 CPUs of the single NUMA node), so 8 workers have 48 CPUs"},
    "nodes": {"od-1..od-4": "the four on-demand D1 large-shard nodes, each on its own shard's models after its D1 pass",
              "spot-1..spot-5": "five spot instances of the same type, AMI and bootstrap (scripts/aws/bench-node.sh), "
                                "volume provisioned at 16000 IOPS and 1000 MiB/s, launched to run later repetitions in parallel; "
                                "dive and dive-bench copied from the first node and checked by SHA-256 before timing"},
    "dive_bench_sha256": "24189573d2872c63f9aead869ea6c60f988fa534cd9f9ef2199446a358ee7dfe",
    "command": "dive-bench --set <shard-set.toml> --dive-bin ~/bins/dive --dive-revision 08a14fbac83ff71a3247623fb10bf0584bc83be5 "
               "--profile default --workers 1,4,8 --reps 3 --timeout 3600 --xmx 24g on od-1..od-4; the parallel repetitions ran "
               "the same command with --reps 1 (and, for SlushLarge, one worker count per call) on a set file holding the same "
               "entries; each row records its repetition and node",
    "benchmark_set_sha256": d["set_sha256"],
    "selection": "The 48-model large class of the D1 record, less specifications/CheckpointCoordination/MCCheckpointCoordinationFailure.cfg: "
                 "dive-bench needs a success golden, and that model's golden is a violation.",
    "raw": d["raw"],
    "deviations": [
        "Repetitions were spread across nodes of one type to finish sooner: an on-demand shard node ran repetition 1 (and 2 on "
        "od-1, od-3 and od-4, and 3 on od-4), spot nodes ran the rest. A shard node's dive-bench was stopped once its last kept "
        "repetition was recorded; the run in progress at that moment was discarded, never recorded. Each row's "
        "runs_rep_node_wall_ms names the node of every repetition.",
        "dive-bench's golden check compares raw TLC counts and does not read conformance/expected-divergences.toml. At one worker "
        "MCCRDT, VoteProof, MCKVsnap and SpanTreeRandom differ from the golden exactly as registered there; the record names the "
        "entry (registered_divergence). Generated counts at more than one worker are diagnostics only (ADR 0086).",
        "Release binary built on one bench node and copied to the others (independent builds were not byte-identical); "
        "dive_binary_sha256 names the copied binary."],
    "summary": dict(d["summary"], **extra),
    "rows": d["rows"],
}
json.dump(spec, open(sys.argv[2], "w"), indent=1)
