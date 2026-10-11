"""Check separate source/queue epochs without ordinary scheduler or verdict credit."""
from pathlib import Path
import gzip, hashlib, itertools, json
ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / 'conformance/probes/native-periodic-lifecycle'
load = lambda p: json.loads(p.read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
q = BASE / 'queue-primary'
for name in ['independent-readback.json', 'empty-readback.json']:
    receipt = load(q / name)
    for path, digest in receipt['artifacts_sha256'].items():
        assert sha((q / path).read_bytes()) == digest, path
pin = '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert load(q / 'capture.json')['oracle_sha256'] == pin
assert load(q / 'empty-capture.json')['oracle_sha256'] == pin
rows = [json.loads(line) for line in (q / 'stage1.stdout').read_text().splitlines()]
selection = list(itertools.product((1, 4, 8), range(8), ('resume', 'finished-before', 'finished-during')))
assert len(rows) == len(selection) == 72
for row, (width, repeat, scenario) in zip(rows, selection):
    resume = scenario == 'resume'
    assert (row['width'], row['repeat'], row['scenario']) == (width, repeat, scenario)
    assert row['suspended'] == resume
    assert row['waiting'] == (width if resume else 0)
    assert row['states_taken'] == (width if resume else 0)
    assert row['size'] == (0 if resume else width)
empty = [json.loads(line) for line in (q / 'empty-stage1.stdout').read_text().splitlines()]
assert len(empty) == 24
for row, (width, repeat) in zip(empty, itertools.product((1, 4, 8), range(8))):
    assert row == dict(width=width, repeat=repeat, suspended=False, finished=width, size=0)
d = BASE / 'development-final-snapshot'
for path, digest in load(d / 'source.json').items():
    assert sha(gzip.decompress((d / 'source' / (path + '.gz')).read_bytes())) == digest, path
assert load(d / 'record.json')['exit'] == 101
assert sha((d / 'focused.log').read_bytes()) == load(d / 'record.json')['log_sha256']
assert 'index out of bounds' in (d / 'focused.log').read_text()
d = BASE / 'development-default-profile'
assert 'require TLC value/comparison/exploration mode' in (d / 'focused.log').read_text()
h = BASE / 'history-before-lifecycle'
m = load(h / 'manifest.json')
for path, digest in m['matrices_sha256'].items():
    assert sha((h / 'matrices' / path).read_bytes()) == digest, path
for paths in m['source_by_matrix'].values():
    for path, digest in paths.items():
        assert sha(gzip.decompress((h / 'source-by-digest' / (digest + '.gz')).read_bytes())) == digest, path
if (BASE / 'source-matrix.json').exists():
    for path, digest in load(BASE / 'source-matrix.json')['source_sha256'].items():
        assert sha((ROOT / path).read_bytes()) == digest, path
if (BASE / 'native-current/manifest.json').exists():
    d = BASE / 'native-current'; m = load(d / 'manifest.json')
    assert m['model_proof'] is False and m['ordinary_schedule_certified'] is False
    assert len(m['rows']) == len(set(m['rows'])) == 40
    for path, digest in m['artifact_sha256'].items(): assert sha((d / path).read_bytes()) == digest, path
    assert len(m['replay_rows']) == len(set(m['replay_rows'])) == 8
    for path in m['replay_rows']:
        replay = load(d / path)
        assert replay['producer_profile'] == 'tlc' and replay['validator_profile'] in ['Tlc', 'Dive']
        assert replay['width'] in [1,3,4,8] and replay['word_initial'] is False
        assert replay['memory'] == replay['json'] == 'Valid' and replay['model_proof'] is False
        assert replay['wrong_init'].startswith('Invalid(')
        assert replay['wrong_closing_next'].startswith('Invalid(') and 'Next' in replay['wrong_closing_next']
        assert replay['malformed_ending'].startswith('Invalid(') and replay['wrong_property'] != 'Valid'
    for path in m['rows']:
        row = load(d / path)
        assert not row['model_proof'] and not row['ordinary_schedule_certified']
        eof = row['safety_eof']
        if row['final_checks']: assert eof is not None and eof['queue'] == 0
        if eof is None: assert not row['final_checks']
        if row['safety_complete']: assert row['stop'] is None and eof is not None
        for call in row['invocations']:
            assert call['workers'] in [1,3,4,8] and call['time_millis'] > 0
            assert call['generation'] > 0 or call['workers'] == 1
        if 'default-refusal' in path:
            assert row['stop']['workers']['used'] == 0 and eof is None and not row['invocations']
example = (ROOT / 'crates/dive-api/examples/periodic_lifecycle.rs').read_text().split('\n', 1)[1].rstrip()
assert '```rust\n' + example + '\n```' in (ROOT / 'docs/native-periodic-lifecycle.md').read_text()
print('Verified96 real selected queue controls, preserved failures and source epochs; native commit-clock/EOF diagnostics grant no ordinary proof')
