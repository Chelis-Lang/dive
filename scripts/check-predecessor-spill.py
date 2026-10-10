"""Verify byte-exact native predecessor controls and delivery source history."""
import gzip, hashlib, json, sys
from pathlib import Path
ROOT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent
BASE = ROOT / 'conformance/probes/predecessor-spill'
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def load(p): return json.loads(p.read_text())
def observed(j):
    j = json.loads(json.dumps(j))
    j.pop('elapsed_ms', None); j.pop('predecessor_storage', None)
    for key in ['predecessor_memory_bytes', 'predecessor_path_steps_limit']: j['options'].pop(key, None)
    return j
m = load(BASE/'delivery.json')
assert m['oracle_sha256'] == '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
for path, digest in m['source_sha256'].items():
    assert hashlib.sha256(gzip.decompress((BASE/'historical-delivery-source'/(path+'.gz')).read_bytes())).hexdigest() == digest, path
for path, digest in m['preserved_logs_sha256'].items(): assert sha(BASE/path) == digest, path
assert sha(BASE/'capture_controls.py') == m['capture_script_sha256']
r = load(BASE/'raw/index.json')
assert r['native_binary_sha256'] == m['native_binary_sha256']
assert len(r['controls']) == 936
references = {}; seen = set(); bounded = 0
for c in r['controls']:
    identity = tuple(c[k] for k in ['case', 'profile', 'store', 'workers', 'chaos'])
    key = identity + (c['budget'],); assert key not in seen; seen.add(key)
    p = BASE/'raw'/c['stdout']; assert sha(p) == c['stdout_sha256'], p
    assert (BASE/'raw'/c['stderr']).exists()
    j = load(p); assert j['options']['profile'] == c['profile']
    assert j['options']['workers']['chaos'] == c['chaos']
    assert j['verdict'] not in ['unsupported', 'internal-error', 'resource-limit']
    if c['budget'] is None: references[identity] = (c['exit'], observed(j))
    else:
        assert c['budget'] in [0, 24]
        assert (c['exit'], observed(j)) == references[identity], key
        assert j['predecessor_storage']['cleanup_completed']
        assert j['predecessor_storage']['failures'] == []
        bounded += 1
assert len(references) == 312 and bounded == 624
current = load(BASE/'current-integration-source.json')
for history in current.get('historical_integration_sources', []):
 metadata = BASE/history['matrix']
 assert hashlib.sha256(metadata.read_bytes()).hexdigest() == history['matrix_sha256'], metadata
 for source, digest in load(metadata)['source_sha256'].items():
  archived = BASE/history['source_archive']/(source+'.gz')
  assert hashlib.sha256(gzip.decompress(archived.read_bytes())).hexdigest() == digest, archived

for path, digest in current['source_sha256'].items(): assert sha(ROOT/path) == digest, path
print('verified 936 original runs, 624 bounded comparisons, complete delivery source history and current integration hashes')
