#!/usr/bin/env python3
"""Find and independently replay a full match for a pinned CCF consistency history."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory
import time

from trace_match_support import Unsupported, replay, run, tla_literal, unique_fields


PINS = {
    "TraceMultiNodeReads.tla": "4c333d9ea385a308bbe74894e6838db7aea92a0cd760c640b38c5ac3b8fcb42a",
    "TraceMultiNodeReads.cfg": "41415359ff02859e705bb24e89a7ec762ed43413dccae44d080e62bd0e0f2697",
    "MultiNodeReads.tla": "efa7b9213f61534bbb826719e48643c8ef1750829fd355b724a7dd5e919546e1",
    "MultiNode.tla": "dfae3b4e0cc47c743365e1fe00495437c1324bb657266572ccab799e8bf589cf",
    "SingleNode.tla": "76d89beda180945967369a2e42d6e6aed5303dd6315afd881b8b1f2d598767c8",
    "SingleNodeReads.tla": "064a1a7499f50f672133eafd255200e145781fab0d7a3934349889c3488f3b88",
    "ExternalHistory.tla": "8ff72d4bc2b1d43e346a8fe0df0ff8430b72da8010228199a46b8b0c938e8334",
    "ExternalHistoryInvars.tla": "eb43f21778ab16b4f689a37fed7aae06044e858ff4f746535111cc6164505ec5",
}

# Only fields read by the original observation relation are required. In
# particular, unobserved response data is still chosen by the protocol.
REQUIRED = {
    "RwTxRequestAction": {"type": str, "tx": int},
    "RwTxExecuteAction": {"tx": int, "tx_id": list},
    "RwTxResponseAction": {"type": str, "tx": int, "tx_id": list},
    "StatusCommittedResponseAction": {"type": str, "status": str, "tx_id": list},
    "StatusInvalidResponseAction": {"type": str, "status": str, "tx_id": list},
    "RoTxRequestAction": {"type": str, "tx": int},
    "RoTxResponseAction": {"type": str, "tx": int, "tx_id": list},
}
TYPES = {"RwTxRequest", "RwTxResponse", "TxStatusReceived", "RoTxRequest", "RoTxResponse"}
STATUSES = {"CommittedStatus", "InvalidStatus"}


def read_events(log):
    events = []
    size = 0
    with log.open("rb") as stream:
        while line := stream.readline(65537):
            size += len(line)
            if len(line) > 65536 or size > 4 * 1024 * 1024 or len(events) >= 512:
                raise Unsupported("CCF trace input limit exceeded")
            event = json.loads(line, object_pairs_hook=unique_fields)
            position = len(events) + 1
            action = event.get("action") if isinstance(event, dict) else None
            if not isinstance(action, str) or action not in REQUIRED:
                raise Unsupported(f"unrecognized CCF action at observation {position}")
            for field, kind in REQUIRED[action].items():
                if type(event.get(field)) is not kind:
                    raise Unsupported(f"missing or malformed {field} at observation {position}")
            if "type" in REQUIRED[action] and event["type"] not in TYPES:
                raise Unsupported(f"unknown transaction type at observation {position}")
            if "status" in REQUIRED[action] and event["status"] not in STATUSES:
                raise Unsupported(f"unknown status at observation {position}")
            if "tx_id" in REQUIRED[action]:
                tx_id = event["tx_id"]
                if len(tx_id) != 2 or any(type(v) is not int or v < 1 for v in tx_id):
                    raise Unsupported(f"expected positive view and sequence number at observation {position}")
            events.append(event)
    if not events:
        raise Unsupported("empty CCF trace")
    return events


def prepare(directory, source, events):
    values = {}
    for name, digest in PINS.items():
        data = (source / name).read_bytes()
        if hashlib.sha256(data).hexdigest() != digest:
            raise Unsupported(f"unrecognized upstream CCF source: {name}")
        if name.startswith("TraceMultiNodeReads."):
            values[name] = data.decode()
        else:
            (directory / name).write_bytes(data)

    upstream = values["TraceMultiNodeReads.tla"]
    body = upstream[upstream.index("TraceInit =="):upstream.index("TraceSpec ==")]
    # The pinned source's TLC register write is accounting. The CFG's TLC
    # exit and queue checks are evaluated in the separate upstream run.
    body = body.replace("    /\\ TLCSet(0, Max({l', TLCGet(0)}))\n", "")
    original_cfg = values["TraceMultiNodeReads.cfg"]
    constants = "CONSTANTS\n" + original_cfg.split("\nCONSTANTS\n", 1)[1]
    common = "EXTENDS MultiNodeReads, Sequences, SequencesExt\nVARIABLE l\n"
    common += "JsonLog == " + tla_literal(events) + "\n" + body
    common += r"ObservedNext == /\ l <= Len(JsonLog) /\ TraceNext" + "\n"
    behavior = r"ProtocolNext == NextMultiNodeReadsAction \/ UNCHANGED vars" + "\n"
    search = common + behavior + r"MatchNext == ObservedNext /\ ProtocolNext" + "\n"
    pending = "LogPending == l <= " + str(len(events)) + "\n"
    (directory / "Observation.cfg").write_text(
        "INIT TraceInit\nNEXT ObservedNext\nINVARIANT LogPending\nCHECK_DEADLOCK FALSE\n" + constants
    )
    (directory / "Observation.tla").write_text("---- MODULE Observation ----\n" + common + pending + "====\n")
    (directory / "Protocol.tla").write_text(
        "---- MODULE Protocol ----\nEXTENDS MultiNodeReads\n" + behavior + "====\n"
    )
    (directory / "Protocol.cfg").write_text("INIT Init\nNEXT ProtocolNext\nCHECK_DEADLOCK FALSE\n" + constants)
    (directory / "Match.cfg").write_text(
        "INIT TraceInit\nNEXT MatchNext\nINVARIANT LogPending\nCHECK_DEADLOCK FALSE\n" + constants
    )
    return "---- MODULE Match ----\n" + search


def search_prefix(binary, directory, source, prefix, options, seconds):
    (directory / "Match.tla").write_text(source + f"LogPending == l <= {prefix}\n====\n")
    budget = f"{max(1, int(seconds * 1000))}ms"
    code, row = run(binary, directory, "check", "Match", [*options, "--time-limit", budget], seconds)
    verdict = row.get("verdict")
    stats = row.get("statistics") or {}
    if code == 12 and verdict == "invariant-violation" and row.get("property") == "LogPending":
        return "matched", row
    if code == 0 and verdict == "success" and stats.get("left_on_queue") == 0 and row["options"].get("store") == "exact":
        return "no-completion", row
    if verdict in {"timeout", "stopped", "resource-limit", "memory-limit", "state-limit", "depth-limit"} or code == 5:
        return "inconclusive", row
    raise Unsupported(row.get("message") or f"unexpected Core check verdict {verdict}")


def match(args, directory):
    deadline = time.monotonic() + args.seconds

    def remaining():
        duration = deadline - time.monotonic()
        if duration < 0.001:
            raise subprocess.TimeoutExpired("CCF full-history check and replay", args.seconds)
        return duration

    events = read_events(args.log)
    source = prepare(directory, args.source, events)
    options = ["--engine", "core", "--store", "exact", "--workers", "1",
               "--exit-codes", "tlc", "--max-memory", "1g"]
    if args.max_states is not None:
        options.extend(["--max-states", str(args.max_states)])
    status, report = search_prefix(args.dive, directory, source, len(events), options, remaining())
    out = {"result": status, "events": len(events), "search": report["verdict"],
           "search_statistics": report.get("statistics")}
    if status == "inconclusive":
        return {**out, "reason": report.get("message") or report["verdict"]}
    if status == "no-completion":
        matched, impossible = 0, len(events)
        while matched + 1 < impossible:
            pivot = (matched + impossible) // 2
            status, row = search_prefix(args.dive, directory, source, pivot, options, remaining())
            if status == "inconclusive":
                return {**out, "result": status, "reason": row.get("message") or row["verdict"]}
            if status == "matched":
                matched = pivot
            else:
                impossible = pivot
        return {**out, "matched_observations": matched, "first_incompatible_observation": impossible}

    trace = report["counterexample"]["trace"]
    cursors = [state["values"]["l"] for state in trace["states"]]
    if cursors[0] != 1 or cursors[-1] != len(events) + 1 or any(
        y - x not in (0, 1) for x, y in zip(cursors, cursors[1:])
    ):
        raise Unsupported("word did not consume each observation in order")
    for state in trace["states"]:
        state["action"] = None
    replay(args.dive, directory, "Observation", "observations.json", trace, remaining())

    # Retain every hidden field and intermediate ledger action. The projection
    # erases only the observation cursor and the search-ending invariant.
    trace["variables"].remove("l")
    trace["ending"] = {"kind": "behavior"}
    for state in trace["states"]:
        del state["values"]["l"]
    replay(args.dive, directory, "Protocol", "protocol.json", trace, remaining())
    if args.out:
        shutil.copytree(directory, args.out)
    return {**out, "matched_observations": len(events), "witness_states": len(cursors),
            "observation_replay": "valid", "protocol_replay": "valid"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="pinned Microsoft CCF tla/consistency directory")
    parser.add_argument("log", type=Path, help="ordered CCF NDJSON observations")
    parser.add_argument("--dive", default="dive", help="ordinary Dive executable")
    parser.add_argument("--seconds", type=int, default=30, help="total search and validation time budget")
    parser.add_argument("--max-states", type=int, help="discovered-state cap per exact search")
    parser.add_argument("--out", type=Path, help="new directory for both words and their replay models")
    args = parser.parse_args()
    if args.seconds <= 0 or (args.max_states is not None and args.max_states < 1):
        parser.error("budgets must be positive")
    if args.out and args.out.exists():
        parser.error("--out must name a new directory")
    try:
        binary = shutil.which(args.dive)
        if binary is None:
            raise Unsupported("Dive executable not found")
        args.dive = Path(binary).resolve()
        with TemporaryDirectory(prefix="dive-ccf-history-") as tmp:
            result = match(args, Path(tmp))
    except subprocess.TimeoutExpired:
        result = {"result": "inconclusive", "reason": "CCF search or independent replay timed out"}
    except (ValueError, TypeError, KeyError, IndexError, RecursionError, OSError) as exc:
        result = {"result": "unsupported", "reason": str(exc)}
    print(json.dumps(result))
    return {"matched": 0, "no-completion": 1, "unsupported": 3, "inconclusive": 5}[result["result"]]


if __name__ == "__main__":
    raise SystemExit(main())
