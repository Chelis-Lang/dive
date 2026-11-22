#!/usr/bin/env python3
"""Exercise the pinned public trace, two independent replays, a corrupted vote, and explicit stops."""

import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile


MATCHER = Path(__file__).with_name("match-etcd-trace.py")
INPUT_SHA = "cd02768bd1849a717d9abaf29612d7901efe8048d2b3fa25d9a66ba3df7f7db7"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dive", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    args = parser.parse_args()
    binary = args.dive.resolve()
    source = args.source.resolve()
    log = source / "example.ndjson"
    assert hashlib.sha256(log.read_bytes()).hexdigest() == INPUT_SHA
    records = [json.loads(line) for line in log.read_text().splitlines()[:81]]
    assert len(records) == 81 and all(r["tag"] == "trace" for r in records)
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        modified = directory / "edited.ndjson"
        output = directory / "witness"

        def run(case, path, status, outcome, *options):
            result = subprocess.run([sys.executable, MATCHER, "--dive", binary, *options, source, path],
                                    capture_output=True, text=True, timeout=110)
            data = json.loads(result.stdout)
            assert (result.returncode, data["result"]) == (status, outcome), (case, data)
            print(json.dumps({"case": case, **{k: v for k, v in data.items() if k != "witness_dir"}}))
            return data

        def save(changed):
            modified.write_text("".join(json.dumps(r) + "\n" for r in changed))

        normal = run("published", log, 0, "matched", "--prefix", "81", "--out", output)
        assert (normal["events"], normal["states"], normal["event_replay"], normal["raft_replay"]) == (81, 82, "valid", "valid")
        run("state bound", log, 5, "inconclusive", "--prefix", "81", "--max-states", "20")

        # The candidate can really issue this term-three message, but cannot
        # do it immediately after the recorded term-two election.
        vote = copy.deepcopy(records[76])
        assert vote["event"]["name"] == "SendRequestVoteRequest"
        vote["event"]["msg"]["term"] = vote["event"]["state"]["term"] = 3
        save(records[:76] + [vote])
        run("globally inconsistent vote", modified, 1, "no-completion")
        run("prefix before inconsistent vote", modified, 0, "matched", "--prefix", "76")
        election = copy.deepcopy(records[75])
        assert election["event"]["name"] == "BecomeCandidate"
        election["event"]["state"]["term"] = 3
        save(records[:76] + [election, vote])
        run("term-three vote after another election", modified, 0, "matched")

        forged = json.loads((output / "behavior.json").read_text())
        forged["states"][-1]["values"]["pendingMessages"] = forged["states"][0]["values"]["pendingMessages"]
        forged_path = directory / "forged.json"
        forged_path.write_text(json.dumps(forged))
        replay = subprocess.run([binary, "validate-trace", "RaftBehavior.tla", "--config", "RaftBehavior.cfg",
                                 "--seed", "0", "--fp", "0", "--output", "json", "--trace", forged_path],
                                cwd=output, capture_output=True, text=True, timeout=30)
        assert (replay.returncode, json.loads(replay.stdout)["result"]) == (1, "invalid")
        print(json.dumps({"case": "tampered complete Raft word", "result": "invalid"}))

        extra = copy.deepcopy(records[76])
        extra["event"]["name"] = "UnknownProducerEvent"
        save(records[:76] + [extra])
        run("unknown event", modified, 3, "unsupported")
        save(records[:76])
        run("missing requested event", modified, 3, "unsupported", "--prefix", "77")

        # Pinning prevents accepting another protocol with the same filenames.
        (directory / "etcdraft.tla").write_text((source / "etcdraft.tla").read_text() + "\n")
        unpinned = subprocess.run([sys.executable, MATCHER, "--dive", binary, directory, modified],
                                  capture_output=True, text=True)
        assert (unpinned.returncode, json.loads(unpinned.stdout)["result"]) == (3, "unsupported")


if __name__ == "__main__":
    main()
