"""Read back historical producer bytes separately from current integrated code."""
from pathlib import Path
import gzip, hashlib, json
BASE = Path(__file__).resolve().parent.parent
ROOT = BASE.parents[2]
def sha(data):
    return hashlib.sha256(data).hexdigest()
legacy = json.loads((BASE / 'delivery.json').read_text())
assert legacy['source_commit'] is None
assert legacy['source_status'] == 'uncommitted-frozen-during-gates-and-consumers'
assert legacy['family_certified'] is False
assert len(legacy['checked_sources']) == 318
assert len(legacy['unchanged_build_inputs']) == 26
assert len(legacy['archived_evidence']) == 28410
for name, digest in legacy['checked_sources'].items():
    assert sha((BASE / 'uncommitted-producer-source' / name).read_bytes()) == digest, name
history = BASE / 'integration-history'
for name, digest in legacy['unchanged_build_inputs'].items():
    assert sha(gzip.decompress((history / 'original-unchanged-build-inputs' / (name + '.gz')).read_bytes())) == digest, name
for name, digest in legacy['archived_evidence'].items():
    assert sha((ROOT / name).read_bytes()) == digest, name
focus = history / 'focused-controls'
record = json.loads((focus / 'receipt.json').read_text())
assert record['source_commit'] is None and record['family_certified'] is False
assert record['tests_passed'] == 16 and record['offline_checks'] == 24
assert len(record['production_input_sha256']) == 412
for name, digest in record['production_input_sha256'].items():
    assert sha(gzip.decompress((focus / 'source' / (name + '.gz')).read_bytes())) == digest, name
for name, digest in record['compressed_log_sha256'].items():
    assert sha((focus / name).read_bytes()) == digest, name
archive = json.loads((focus / 'source-archive.json').read_text())
assert archive['focused_receipt_sha256'] == sha((focus / 'receipt.json').read_bytes())
for name, digest in archive['archive_sha256'].items():
    assert sha((history / name).read_bytes()) == digest, name
print('Verified original 318 source inputs, 26 unchanged build inputs, 28410 evidence files and 412 focused integration inputs; no current runtime or family certification')
