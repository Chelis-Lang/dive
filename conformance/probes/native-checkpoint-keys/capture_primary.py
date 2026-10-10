"""Capture clean pinned TLC and the pre-extension native safety controls.

Usage: python3 -I capture_primary.py JAVA JAR DIVE FRESH_OUTPUT
The DIVE argument is the frozen d311b8cb baseline. No goldens are changed.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[3]
java, jar, binary, out = (Path(x).resolve() for x in sys.argv[1:])
out.mkdir(parents=True, exist_ok=False)
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
assert sha(jar) == "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
record = {"schema_version": 1, "source_parent_commit": "d311b8cbb364b6e2954e8608a992119951990f6b",
          "oracle_sha256": sha(jar), "java_binary_sha256": sha(java),
          "native_binary_sha256": sha(binary), "capture_script_sha256": sha(Path(__file__)), "cases": []}
env = os.environ.copy()
for key in ("DIVE_COMPAT", "DIVE_CHAOS"):
    env.pop(key, None)
for name in ("CpView", "CpSymView", "CpSymConcrete"):
    source = Path("conformance/probes/native-checkpoint-keys/specs") / name / (name + ".tla")
    cfg = source.with_suffix(".cfg")
    entry = {"name": name, "source_sha256": {str(source): sha(root/source), str(cfg): sha(root/cfg)}, "runs": {}}
    record["cases"].append(entry)
    with tempfile.TemporaryDirectory(prefix="dive-cp-key-tlc-") as meta:
        args = ["-cp", str(jar), "tlc2.TLC", "-tool", "-workers", "1", "-seed", "7", "-fp", "0",
                "-cleanup", "-metadir", meta, "-config", str(root/cfg), str(root/source)]
        run = subprocess.run([str(java), *args], cwd=root, capture_output=True, timeout=30, env=env)
    for stage, result in [("tlc", run)]:
        leaf = f"{name}-{stage}"
        (out/(leaf+".stdout")).write_bytes(result.stdout)
        (out/(leaf+".stderr")).write_bytes(result.stderr)
        entry["runs"][stage] = {"exit": result.returncode, "stdout": leaf+".stdout", "stderr": leaf+".stderr",
                                "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(), "stderr_sha256": hashlib.sha256(result.stderr).hexdigest()}
    args = ["check", str(source), "--config", str(cfg), "--compat=tlc", "--seed", "7", "--fp", "0", "--workers", "1", "--output", "json"]
    for stage, extra in [("ordinary", []), ("native-before", ["--checkpoint", str(out/(name+".cp")), "--checkpoint-after-levels", "0"])]:
        result = subprocess.run([str(binary), *args, *extra], cwd=root, capture_output=True, timeout=30, env=env)
        leaf = f"{name}-{stage}"
        (out/(leaf+".stdout")).write_bytes(result.stdout)
        (out/(leaf+".stderr")).write_bytes(result.stderr)
        value = json.loads(result.stdout)
        entry["runs"][stage] = {"exit": result.returncode, "verdict": value["verdict"], "stdout": leaf+".stdout", "stderr": leaf+".stderr",
                                "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(), "stderr_sha256": hashlib.sha256(result.stderr).hexdigest()}
    (out/"index.json").write_text(json.dumps(record, indent=2)+"\n")
    text = (out/(name+"-tlc.stdout")).read_text()
    counts = re.search(r"(\d+) states generated, (\d+) distinct states found, (\d+) states left on queue", text)
    depth = re.search(r"The depth of the complete state graph search is (\d+)", text)
    assert counts and depth and entry["runs"]["tlc"]["exit"] == 12, name
    expected = tuple(map(int, counts.groups())) + (int(depth[1]),)
    ordinary = json.loads((out/(name+"-ordinary.stdout")).read_text())
    got = tuple(ordinary["statistics"][k] for k in ("generated", "distinct", "left_on_queue", "depth"))
    assert got == expected and ordinary["verdict"] == "invariant-violation", (name, got, expected, ordinary)
    assert entry["runs"]["native-before"]["verdict"] == "unsupported", name
    assert not (out/(name+".cp")).exists(), name
    entry["tlc_counts"] = expected
    print(name, expected, "ordinary agrees, native refuses", flush=True)
(out/"index.json").write_text(json.dumps(record, indent=2)+"\n")
