"""Retain complete controlled primary epochs, source ownership and nonproof contracts."""
from pathlib import Path
import gzip
import hashlib
import json
import tarfile

ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / 'conformance/probes/committed-prefix'
sha = lambda data: hashlib.sha256(data).hexdigest()
load = lambda path: json.loads(path.read_text())
index = load(BASE/'primary-index.json')
pin = '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert index['oracle_sha256'] == pin and index['selection'] == 24
assert index['development_epochs'] == [0,1] and index['primary_epoch'] == 2
assert not index['model_proof'] and not index['ordinary_schedule_certified']
for name,digest in index['artifact_sha256'].items():
    assert sha((BASE/name).read_bytes()) == digest,name
with tarfile.open(BASE/'primary-candidate2.tar.gz') as archive:
    raw = {}
    for item in archive:
        assert not item.name.startswith('/') and '..' not in Path(item.name).parts
        assert item.isfile() or item.isdir()
        if item.isfile():raw[item.name] = archive.extractfile(item).read()
capture = load(BASE/'capture.json')
assert raw['capture.json'] == (BASE/'capture.json').read_bytes()
assert capture['oracle_sha256'] == pin
cases = ['later-written','output-before-write','known-error-before','error-during-suspend','hook-false','hook-error','candidate-before-hook','actual-eof']
assert [(r['width'],r['scenario']) for r in capture['rows']] == [(w,c) for w in [1,4,8] for c in cases]
for name,digest in capture['sources'].items():assert sha(raw[name]) == digest == sha((BASE/name).read_bytes()),name
for row in capture['rows']:
    case = row['scenario']; width = row['width']; prefix = f'{case}-{width}/'
    assert not row['timeout'] and row['exit'] == (2 if case == 'hook-error' else 0)
    for name,digest in row['sha256'].items():assert sha(raw[prefix+name]) == digest,(prefix,name)
    events = [line[6:] for line in raw[prefix+'stdout'].decode().splitlines() if line.startswith('EVENT\t')]
    assert events == row['events'] and not any(e.startswith('injector-error') for e in events)
    assert 'init-result:0' in events and 'start-workers' in events
    if case == 'hook-error':
        assert 'exception-class:tlc2.tool.EvalException' in events and 'exception-code:2179' in events
        assert 'cleanup-joined' not in events and 'done' not in events
    else:
        assert 'cleanup-joined' in events and 'done' in events
        returned = next(e for e in events if e.startswith('runTLC-result:'))
        assert events.index('cleanup-joined') > events.index(returned)
    if case == 'hook-false':assert 'runTLC-result:2104' in events
    if case == 'candidate-before-hook':assert 'runTLC-result:2116' in events
    if case.startswith('known-error') or case == 'error-during-suspend':
        assert 'error-code-at-return:2179' in events and 'keep-call-stack-at-return:true' in events
readback = load(BASE/'independent-readback.json')
assert len(readback['rows']) == 24 and not readback['ordinary_model_proof']
for name,digest in readback['hashes'].items():assert sha(raw[name]) == digest,name
example = (ROOT/'crates/dive-api/examples/committed_prefix.rs').read_text().rstrip()
assert '```rust\n'+example+'\n```' in (ROOT/'docs/committed-periodic-prefix.md').read_text()
history = BASE/'history-before-prefix/manifest.json'
if history.exists():
    h = load(history)
    for name,digest in h['matrices_sha256'].items():
        matrix = BASE/'history-before-prefix/matrices'/name
        assert sha(matrix.read_bytes()) == digest,name
        assert load(matrix)['source_sha256'] == h['source_by_matrix'][name]
    for digest,name in h['source_archive_paths'].items():assert sha(gzip.decompress((ROOT/name).read_bytes())) == digest,name
current = BASE/'source-matrix.json'
if current.exists():
    d = load(current)
    assert not d['model_proof'] and not d['ordinary_schedule_certified'] and not d['family_certified']
    for name,digest in d['source_sha256'].items():assert sha((ROOT/name).read_bytes()) == digest,name
development = BASE/'development/manifest.json'
if development.exists():
    record = load(development)
    for epoch,files in record['epochs'].items():
        for name,hashes in files.items():
            raw = (BASE/'development'/epoch/name).read_bytes()
            assert sha(raw) == hashes['sha256'],(epoch,name)
            content = gzip.decompress(raw) if name.endswith('.gz') else raw
            assert sha(content) == hashes['uncompressed_sha256'],(epoch,name)
    for name,digest in record['logs_sha256'].items():assert sha((BASE/'development'/name).read_bytes()) == digest,name
native = BASE/'native-current/manifest.json'
if native.exists():
    record = load(native)
    assert not record['ordinary_schedule_certified'] and not record['model_proof']
    for name,digest in record['artifact_sha256'].items():assert sha((BASE/name).read_bytes()) == digest,name
    published = 0
    for name in record['rows']:
        row = load(BASE/name)
        assert not row['model_proof'] and not row['ordinary_schedule_certified'],name
        for pause in row['pauses']:
            assert pause['frontier_appended_start'] == pause['committed_start']
            assert pause['frontier_appended_end'] == pause['committed_end']
            assert pause['predicate_workers'] <= 1
            committed = set(range(pause['committed_start'],pause['committed_end']))
            assert committed.issubset(pause['completed']),name
            if name.split('/')[-1].startswith('published-'):published += bool(committed)
    assert published > 0,'genuine RequestPrefix discriminator missing'
    for name in record['valid_replays']:
        row = load(BASE/name)
        assert row['memory'] == row['json'] == 'Valid'
        assert row['wrong_init'].startswith('Invalid(') and row['wrong_closing_next'].startswith('Invalid(')
        assert 'Next' in row['wrong_closing_next']
        assert row['wrong_property'] != 'Valid' and row['malformed_ending'] != 'Valid'
    for name in record['refused_replays']:
        row = load(BASE/name)
        assert 'quantifier domain materialization work' in row['memory'] and 'quantifier domain materialization work' in row['json']
        assert row['memory'].startswith('Unsupported(') and row['json'].startswith('Unsupported(')
print('Verified24 actual controlled Worker/queue rows, unchanged failed epochs and native prefix nonproof boundary')
