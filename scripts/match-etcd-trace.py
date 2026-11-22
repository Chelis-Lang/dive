#!/usr/bin/env python3
"""Match a finite etcd NDJSON event history against the pinned source trace specification."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

from trace_match_support import Unsupported, replay, run, tla_literal, unique_fields


PINS = {
    "etcdraft.tla": "15c445684529c59a59877951c7fb7df9a40da250aaa445f056ef48f56bb27044",
    "Traceetcdraft.tla": "93e46f774c6602674d6f6db90c75ddc4144cfadc101c2f1cb31fd1ce2e90b0bc",
    "Traceetcdraft.cfg": "f146d25a0311f6944f5f3b3a89dcd66409d7fc3aac9326373b7cebf70a0b1795",
}
# Events dispatched by the upstream TraceNext relation. Unknown input must
# not be interpreted as a well-defined event having an empty successor set.
EVENTS = set("""
    InitState ApplyConfChange BecomeFollower BecomeCandidate BecomeLeader Ready
    Commit Replicate ChangeConf SendRequestVoteRequest SendRequestVoteResponse
    ReceiveRequestVoteRequest ReceiveRequestVoteResponse SendAppendEntriesRequest
    SendAppendEntriesResponse ReceiveAppendEntriesRequest ReceiveAppendEntriesResponse
    ReceiveSnapshot ReduceNextIndex
""".split())
MAX_EVENTS = 512
MAX_LINE_BYTES = 65536
MAX_INPUT_BYTES = 4 * 1024 * 1024


def check_fields(record, fields):
    if not isinstance(record, dict):
        raise Unsupported("expected a trace record")
    for key, kind in fields.items():
        if type(record.get(key)) is not kind:
            raise Unsupported(f"missing or malformed trace field {key}")


def validate_event(event, position):
    if not isinstance(event, dict) or event.get("name") not in EVENTS:
        raise Unsupported(f"unrecognized event at trace position {position}")
    check_fields(event, {"nid": str, "role": str, "state": dict, "log": int, "conf": list})
    check_fields(event["state"], {"term": int, "commit": int, "vote": str})
    if event["role"] not in {"StateFollower", "StateCandidate", "StateLeader"}:
        raise Unsupported(f"unknown Raft role at trace position {position}")
    if len(event["conf"]) != 2 or any(
        not isinstance(group, list) or any(not isinstance(node, str) for node in group)
        for group in event["conf"]
    ):
        raise Unsupported("expected two arrays of node names in conf")
    if event["name"].startswith(("Send", "Receive")):
        message = event.get("msg")
        check_fields(message, {"type": str, "from": str, "to": str, "term": int, "logTerm": int,
                               "commit": int, "index": int, "entries": int, "reject": bool})
        if message["type"] not in {"MsgVote", "MsgVoteResp", "MsgApp", "MsgAppResp", "MsgSnap",
                                   "MsgHeartbeat", "MsgHeartbeatResp"}:
            raise Unsupported(f"unsupported Raft message at trace position {position}")


def read_events(path, prefix):
    events = []
    consumed = 0
    with path.open("rb") as log:
        while prefix is None or len(events) < prefix:
            line = log.readline(MAX_LINE_BYTES + 1)
            if not line:
                break
            consumed += len(line)
            if len(line) > MAX_LINE_BYTES or consumed > MAX_INPUT_BYTES:
                raise Unsupported("input byte limit exceeded")
            row = json.loads(line, object_pairs_hook=unique_fields)
            if not isinstance(row, dict):
                raise Unsupported("NDJSON row is not a record")
            if row.get("tag") != "trace":
                continue
            event = row.get("event")
            validate_event(event, len(events) + 1)
            events.append({"event": event})
            if len(events) > MAX_EVENTS:
                raise Unsupported("trace event limit exceeded")
    if not events or (prefix is not None and len(events) != prefix):
        raise Unsupported("input contains fewer trace events than requested")
    return events


def prepare(directory, source, events):
    sources = {}
    for name, pin in PINS.items():
        raw = (source / name).read_bytes()
        if hashlib.sha256(raw).hexdigest() != pin:
            raise Unsupported(f"unrecognized upstream source: {name}")
        sources[name] = raw.decode()
    (directory / "etcdraft.tla").write_text(sources["etcdraft.tla"])
    ref = sources["Traceetcdraft.tla"]
    # Only the effect-free argument of this one eager constant wrapper is used.
    bootstrap = ref[ref.index("TraceServer =="):ref.index("OneMoreMessage(msg) ==")]
    bootstrap = bootstrap.replace("TraceServer == TLCEval(", "TraceServer == (")
    common = (ref[:ref.index("JsonFile ==")] + "TraceLog == " + tla_literal(events) + "\n\n"
              + bootstrap)
    matcher = (common + ref[ref.index("OneMoreMessage(msg) =="):ref.index("StepToNextTrace ==")]
               + r"StepToNextTrace == /\ l' = l + 1 /\ pl' = l" + "\n\n"
               + ref[ref.index("StepToNextTraceIfMessageIsProcessed"):ref.index("TraceView ==")]
               + "LogPending == l <= Len(TraceLog)\n====\n")
    behavior = common + r"ReplayNext == NextDynamic \/ UNCHANGED vars" + "\n====\n"
    constants = sources["Traceetcdraft.cfg"].split("\nCONSTANTS\n", 1)[1].split("\nINVARIANTS\n", 1)[0]
    for name, text, goals in (
        ("EventMatch", matcher, "INIT TraceInit\nNEXT TraceNext\nINVARIANT LogPending"),
        ("RaftBehavior", behavior, "INIT Init\nNEXT ReplayNext"),
    ):
        (directory / f"{name}.tla").write_text(text.replace("MODULE Traceetcdraft", f"MODULE {name}", 1))
        (directory / f"{name}.cfg").write_text(goals + "\nCHECK_DEADLOCK FALSE\nCONSTANTS\n" + constants + "\n")


def match(args, directory):
    events = read_events(args.log, args.prefix)
    prepare(directory, args.source, events)
    options = ["--engine", "core", "--store", "exact", "--workers", "1", "--exit-codes", "tlc",
               "--time-limit", f"{args.seconds}s", "--max-memory", "1g"]
    if args.max_states is not None:
        options.extend(["--max-states", str(args.max_states)])
    code, result = run(args.dive, directory, "check", "EventMatch", options, args.seconds + 5)
    verdict = result.get("verdict")
    stats = result.get("statistics") or {}
    out = {"scope": "prefix" if args.prefix else "full-input", "events": len(events), "search": verdict, "distinct": stats.get("distinct"),
           "generated": stats.get("generated"), "depth": stats.get("depth")}
    if code == 0 and verdict == "success" and stats.get("left_on_queue") == 0 and result["options"].get("store") == "exact":
        return {**out, "result": "no-completion"}
    if verdict in {"timeout", "stopped", "resource-limit", "memory-limit", "state-limit", "depth-limit"} or code == 5:
        return {**out, "result": "inconclusive", "reason": result.get("message") or verdict}
    if code != 12 or verdict != "invariant-violation" or result.get("property") != "LogPending":
        return {**out, "result": "unsupported", "reason": result.get("message") or verdict}
    trace = result["counterexample"]["trace"]
    replay(args.dive, directory, "EventMatch", "events.json", trace, args.seconds)
    if trace["states"][-1]["values"]["l"] != len(events) + 1:
        raise Unsupported("replayed word did not consume the requested history")

    # Erase only the source trace specification's cursor variables and labels.
    # Check the entire path again under the upstream Raft behavior relation.
    trace["variables"] = [name for name in trace["variables"] if name not in ("l", "pl")]
    trace["ending"] = {"kind": "behavior"}
    for state in trace["states"]:
        del state["values"]["l"], state["values"]["pl"]
        state["action"] = None
    replay(args.dive, directory, "RaftBehavior", "behavior.json", trace, args.seconds)
    if args.out:
        shutil.copytree(directory, args.out)
    return {**out, "result": "matched", "states": len(trace["states"]), "event_replay": "valid",
            "raft_replay": "valid", "witness_dir": str(args.out) if args.out else None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="pinned etcd-io/raft tla directory")
    parser.add_argument("log", type=Path, help="ordered NDJSON from the upstream trace logger")
    parser.add_argument("--dive", default="dive", help="ordinary dive executable")
    parser.add_argument("--prefix", type=int, help=f"check exactly this many trace events (1..{MAX_EVENTS})")
    parser.add_argument("--seconds", type=int, default=30, help="time budget for each checker or replay")
    parser.add_argument("--max-states", type=int, help="cap discovered states during matching")
    parser.add_argument("--out", type=Path, help="new directory for both complete words and their replay inputs")
    args = parser.parse_args()
    if args.seconds <= 0 or (args.max_states is not None and args.max_states < 1):
        parser.error("budgets must be positive")
    if args.prefix is not None and not 1 <= args.prefix <= MAX_EVENTS:
        parser.error(f"--prefix must be between 1 and {MAX_EVENTS}")
    if args.out and args.out.exists():
        parser.error("--out must name a new directory")
    try:
        binary = shutil.which(args.dive)
        if binary is None:
            raise Unsupported("dive executable not found")
        args.dive = Path(binary).resolve()
        with tempfile.TemporaryDirectory(prefix="dive-etcd-trace-") as tmp:
            result = match(args, Path(tmp))
    except subprocess.TimeoutExpired:
        result = {"result": "inconclusive", "reason": "checker or independent replay timed out"}
    except (ValueError, TypeError, KeyError, IndexError, RecursionError, OSError) as exc:
        result = {"result": "unsupported", "reason": str(exc)}
    print(json.dumps(result))
    return {"matched": 0, "no-completion": 1, "unsupported": 3, "inconclusive": 5}[result["result"]]


if __name__ == "__main__":
    raise SystemExit(main())
