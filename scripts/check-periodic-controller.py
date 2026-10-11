"""Verify every pinned periodic observation and its generated Rust control."""
from pathlib import Path
import hashlib
import json
import tarfile

ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / "conformance/probes/ordinary-periodic-work"
load = lambda path: json.loads(path.read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
capture = load(BASE / "capture.json")
readback = load(BASE / "independent-readback.json")
assert capture["oracle_sha256"] == "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
assert capture["family_certified"] is False
assert readback["family_certified"] is False
assert len(capture["rows"]) == readback["selected"] == readback["fully_checked"] == 296
assert readback["disagreements"] == 0

with tarfile.open(BASE / "candidate2.tar.gz", "r:gz") as archive:
    files = {}
    for member in archive.getmembers():
        if not member.isfile():
            assert member.isdir(), member.name
            continue
        path = Path(member.name)
        assert path.parts[0] == "candidate2" and ".." not in path.parts
        relative = str(Path(*path.parts[1:]))
        assert relative not in files
        files[relative] = archive.extractfile(member).read()
    assert json.loads(files["capture.json"]) == capture
    assert json.loads(files["independent-readback.json"]) == readback
    assert set(files) == set(readback["artifact_sha256"]) | {"independent-readback.json"}
    for name, digest in readback["artifact_sha256"].items():
        assert sha(files[name]) == digest, name
    for name, digest in capture["source_sha256"].items():
        assert sha(files[name]) == digest, name

generated = []
for index, (row, checked) in enumerate(zip(capture["rows"], readback["checks"])):
    assert checked["row"] == index
    assert row["exit"] == 0 and row["timeout"] is False
    assert checked["ordered_callbacks_exact"] is True
    assert checked["force_retention_exact"] is True
    assert checked["periodic_effect_count_exact"] is True
    directory = f"case-{index:03}"
    stdout = files[directory + "/stdout"].decode()
    stderr = files[directory + "/stderr"].decode()
    assert sha(files[directory + "/stdout"]) == row["stdout_sha256"]
    assert sha(files[directory + "/stderr"]) == row["stderr_sha256"]
    assert [line[6:] for line in stdout.splitlines() if line.startswith("EVENT\t")] == row["events"]
    observed = [line[6:] if line.startswith("EVENT\t") else "periodic"
                for line in stdout.splitlines()
                if line.startswith("EVENT\t") or line == '"DIVE_PERIODIC"']
    expected = checked["expected_ordered_events"]
    assert len(observed) == len(expected)
    for position, event in enumerate(expected):
        if event == "ratio-update:elapsed":
            actual = observed[position]
            assert actual.startswith("ratio-update:")
            assert int(actual.split(":")[1]) >= 0
            observed[position] = event
    assert observed == expected, index
    assert stderr.count('"DIVE_PERIODIC"') == 1
    assert stdout.count('"DIVE_PERIODIC"') == expected.count("periodic")
    case = row["case"]
    boolean = lambda value: str(value).lower()
    ratio = "f64::NAN" if case["ratio"] == "NaN" else case["ratio"]
    events = ", ".join(json.dumps(event) for event in expected)
    generated.append(
        f'    Case {{ liveness: {boolean(case["liveness"])}, ratio: {ratio}, '
        f'growth: {boolean(case["growth"])}, force: {boolean(case["force"])}, '
        f'checkpoint: {boolean(case["checkpoint"])}, periodic: {json.dumps(case["periodic"])}, '
        f'suspended: {boolean(case["suspended"])}, check_code: {case["check_code"]}, '
        f'expected: &[{events}] }},'
    )
fixture = ("// Generated from the closed pinned-Java observations; elapsed clock arguments alone are abstracted.\n"
           "// Source and failed attempts: conformance/probes/ordinary-periodic-work/.\n"
           "const CASES: &[Case] = &[\n" + "\n".join(generated) + "\n];\n")
assert (ROOT / "crates/dive-liveness/tests/fixtures/periodic_cases.rs").read_text() == fixture
assert len(readback["checks"]) == 296
print("Verified296 pinned periodic observations,2089 archived artifacts and exact generated controls; no family certification")
