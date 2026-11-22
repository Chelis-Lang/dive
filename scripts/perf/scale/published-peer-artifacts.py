#!/usr/bin/env python3
"""Join Recomp-Verify Figure 5 to original inputs and inspect saved Scimitar graphs."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from urllib.request import urlopen

RECOMP = "0cbd95f0002af10f70f25c87638e8f2673229f2e"
SCIMITAR = "f1b902dc0801cf435cb9dd6e6be495d5ed121787"
PAPER = "https://arxiv.org/html/2408.03488v2"
GRAPHS = {
    "TwoPhase_ind-proof-tree-sd3": "1cc2b4e4aa764646886ef4749cf08e6085df51523b3a8902e33bd57700c3a311",
    "AsyncRaft_OnePrimaryPerTerm/AsyncRaft_ind-proof-tree-sd1":
        "1dd7b8e7abced903c3f5d76b9a24856c1a45e711a94727603927e32c984918b9",
    "AsyncRaft_PrimaryHasEntriesItCreated/AsyncRaft_ind-proof-tree-sd4":
        "c71a7d7e4f036d97025c0c6091c9fe0f6bfec146b9eceeca7c0511870d7d5d16",
}


def fetch(url):
    with urlopen(url, timeout=30) as response:
        return response.read()


def measure(seconds):
    if seconds == "TO":
        return None
    match = re.fullmatch(r"(?:(\d+)m)?(\d+)s", seconds)
    if not match:
        raise ValueError(f"unrecognized paper time {seconds}")
    return int(match[1] or 0) * 60 + int(match[2])


def paper_rows():
    html = fetch(PAPER).decode()
    table = html.split('<table id="S7.F5.3"', 1)[1].split("</table>", 1)[0]
    rows = {}
    for row in re.findall(r"<tr\b[^>]*>(.*?)</tr>", table, re.S):
        cells = [re.sub(r"<[^>]+>", "", c).strip() for c in re.findall(r"<td\b[^>]*>(.*?)</td>", row, re.S)]
        if not cells:
            continue
        if len(cells) != 13 or cells[0] in rows:
            raise ValueError("Figure 5 schema changed")
        rows[cells[0]] = {"sequential_s": measure(cells[5]), "portfolio_s": measure(cells[9]),
                          "portfolio_strategy": cells[10], "portfolio_timeout": cells[9] == "TO"}
    if len(rows) != 33:
        raise ValueError("expected the 33 published experiments")
    return rows


def normalized(name):
    return re.sub(r"[-_/]", "", name)


def recomp():
    rows = paper_rows()
    tree = json.loads(fetch(f"https://api.github.com/repos/cmu-soda/recomp-verify/git/trees/{RECOMP}?recursive=1"))
    if tree["truncated"]:
        raise ValueError("truncated source tree")
    blobs = {entry["path"]: entry["sha"] for entry in tree["tree"] if entry["type"] == "blob"}
    base = "benchmarks/tla/"
    papers = {normalized(re.sub(r"^(pyv|tla|ex|i4)-", "", paper)): paper for paper in rows}
    # Figure 5 names the I4 protocol by provenance in the prefix.
    papers["learningswitchi443"] = papers.pop("learningswitch43")
    seen = set()
    results = []
    for cfg in sorted(p for p in blobs if p.startswith(base) and p.endswith(".cfg")):
        tla = cfg[:-4] + ".tla"
        name = normalized(str(Path(cfg.removeprefix(base)).parent))
        cfg_bytes = fetch(f"https://raw.githubusercontent.com/cmu-soda/recomp-verify/{RECOMP}/{cfg}")
        git_blob = hashlib.sha1(b"blob " + str(len(cfg_bytes)).encode() + b"\0" + cfg_bytes).hexdigest()
        if git_blob != blobs[cfg]:
            raise ValueError(f"CFG differs from source tree: {cfg}")
        properties = re.findall(r"^\s*INVARIANTS?\s+(\w+)", cfg_bytes.decode(), re.M)
        if len(properties) != 1:
            raise ValueError(f"expected a single named invariant in {cfg}")
        row = {"cfg": cfg, "cfg_git_blob": git_blob, "module": tla, "module_git_blob": blobs[tla],
               "property": properties[0], "max_rss_kib": None}
        if name in papers:
            paper = papers[name]
            seen.add(paper)
            row.update(paper_id=paper, **rows[paper])
        else:
            row.update(status="artifact-only")
        results.append(row)
    if seen != rows.keys() or len(results) != 34:
        raise ValueError(f"unmatched Figure 5 entries: {sorted(rows.keys() - seen)}")
    return results


def scimitar():
    rows = []
    for name, expected in GRAPHS.items():
        path = f"benchmarks/saved_results/{name}.proofgraph.json"
        data = fetch(f"https://raw.githubusercontent.com/will62794/scimitar/{SCIMITAR}/{path}")
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f"proof graph digest changed: {path}")
        saved = json.loads(data)
        graph = saved["proof_graph"]
        nodes = graph["nodes"].values()
        stats = saved["stats"]
        rows.append({"graph": path, "sha256": expected, "seed": saved["seed"], "property": graph["safety"],
                     "nodes": len(graph["nodes"]), "unique_edges": len(set(map(tuple, graph["edges"]))),
                     "lemmas": stats["num_lemma_nodes"], "failed_lemmas": stats["num_failed_lemma_nodes"],
                     "actions_with_cti_count": sum("total_initial_ctis" in n for n in nodes),
                     "action_initial_ctis_summed": sum(n.get("total_initial_ctis", 0) for n in nodes),
                     "artifact_recorded_seconds": stats["total_duration_secs"],
                     "invariant_check_seconds": stats["invcheck_duration_secs"],
                     "workers": None, "max_rss_kib": None, "status": "saved-graph"})
    return rows


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    report = {"schema": "dive-published-peer-artifacts-v1", "recomp_revision": RECOMP,
              "figure5": PAPER + "#S7.F5", "recomp": recomp(),
              "scimitar_revision": SCIMITAR, "scimitar": scimitar()}
    with args.output.open("x") as file:
        json.dump(report, file, indent=2)
        file.write("\n")
