"""Verify distinct reference/supplied-hook epochs without ordinary proof credit."""
from pathlib import Path
import gzip
import hashlib
import json
import tarfile

ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / 'conformance/probes/periodic-hook'
load = lambda path: json.loads(path.read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
capture = load(BASE / 'capture.json')
receipt = load(BASE / 'independent-readback.json')
assert capture['oracle_sha256'] == '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert receipt['capture_sha256'] == sha((BASE / 'capture.json').read_bytes())
assert receipt['ordinary_schedule_certified'] is False
assert receipt['configured_binding_certified'] is False
assert capture['compile_exit'] == 0
with tarfile.open(BASE / 'primary.tar.gz', 'r:gz') as archive:
    raw = {}
    for member in archive:
        assert member.isfile() or member.isdir(), member.name
        assert not member.name.startswith('/') and '..' not in Path(member.name).parts
        if member.isfile():
            raw[member.name.removeprefix('./')] = archive.extractfile(member).read()
    for name, digest in capture['source_sha256'].items():
        assert sha(raw[name]) == digest, name
        assert sha((BASE / name).read_bytes()) == digest, name
    selected = load(BASE / 'selection.json')
    assert [row['case'] for row in capture['rows']] == selected
    assert len(selected) == receipt['rows'] == 59
    failures = {
        'integer': ('util.Assert$TLCRuntimeException', 1000),
        'string': ('util.Assert$TLCRuntimeException', 1000),
        'set': ('util.Assert$TLCRuntimeException', 1000),
        'tuple': ('util.Assert$TLCRuntimeException', 1000),
        'record': ('util.Assert$TLCRuntimeException', 1000),
        'divide-zero': ('tlc2.tool.EvalException', 2179),
        'state': ('util.Assert$TLCDetailedRuntimeException', 2280),
        'action': ('util.Assert$TLCDetailedRuntimeException', 2280),
        'temporal': ('util.Assert$TLCDetailedRuntimeException', 2261),
        'undefined': ('util.Assert$TLCRuntimeException', 2229),
        'arity': ('util.Assert$TLCRuntimeException', 2228),
        'constant-role': ('java.lang.ClassCastException', -1),
    }
    for index, row in enumerate(capture['rows']):
        case = row['case']
        for name, digest in row['files_sha256'].items():
            assert sha(raw[f'case-{index:03}/{name}']) == digest, (index, name)
        assert row['timeout'] is False
        events = [] if 'role' in case else ['setup-complete', 'start']
        error = None
        if 'role' in case:
            error = failures[case['id']]
        elif not case['suspended']:
            events += ['suspend']
            events += ['result:0', 'force-retained:false']
        else:
            if case['liveness']:
                events += ['growth']
            events += ['suspend']
            if case['liveness']:
                events += ['check-force:false']
            if case['check_code']:
                events += [f"result:{case['check_code']}", 'force-retained:false']
            else:
                stem = next(case['id'].removesuffix('-' + suffix) for suffix in ['invoke', 'suspend-false', 'live-error-first', 'checkpoint-after'] if case['id'].endswith('-' + suffix))
                error = failures.get(stem)
                if error is None:
                    false = stem in ['false', 'short-state']
                    if not false:
                        events += ['checkpoint' if case['checkpoint'] else 'resume']
                    events += [f"result:{2104 if false else 0}", 'force-retained:false']
        if error:
            events += [f'exception-class:{error[0]}', f'exception-code:{error[1]}']
        assert row['events'] == events, (case, row['events'], events)
        assert row['exit'] == (2 if error else 0), case
    assert receipt['files_verified'] == 4 * len(selected)
history = BASE / 'history-before-hook'
manifest = load(history / 'manifest.json')
for name, digest in manifest['matrices_sha256'].items():
    assert sha((history / 'matrices' / name).read_bytes()) == digest, name
for digest, name in manifest['source_archive_paths'].items():
    assert sha(gzip.decompress((ROOT / name).read_bytes())) == digest, name
for sources in manifest['source_by_matrix'].values():
    assert all(digest in manifest['source_archive_paths'] for digest in sources.values())
development = BASE / 'development-redirect-state-target'
record = load(development / 'record.json')
assert record['exit'] == 101
assert sha(gzip.decompress((development / 'test.rs.gz').read_bytes())) == record['source_sha256']
assert 'non-constant' in (development / 'tests-expanded-fixed.stdout').read_text()
development = BASE / 'development-stuttering-closing'
record = load(development / 'record.json')
assert record['exit'] == 101
assert sha(gzip.decompress((development / 'test.rs.gz').read_bytes())) == record['source_sha256']
assert 'left: Valid' in (development / 'replay-final.stdout').read_text()
matrix = BASE / 'source-matrix.json'
if matrix.exists():
    current = load(matrix)
    assert current['ordinary_schedule_certified'] is False and current['model_proof'] is False
    for name, digest in current['source_sha256'].items():
        assert sha((ROOT / name).read_bytes()) == digest, name
native = BASE / 'native-current'
if (native / 'manifest.json').exists():
    manifest = load(native / 'manifest.json')
    assert not manifest['ordinary_schedule_certified'] and not manifest['model_proof']
    assert len(set(manifest['rows'])) == len(manifest['rows']) == 40
    for name, digest in manifest['artifact_sha256'].items():
        assert sha((native / name).read_bytes()) == digest, name
    assert len(set(manifest['replay_rows'])) == len(manifest['replay_rows']) == 4
    for name in manifest['replay_rows']:
        replay = load(native / name)
        assert replay['width'] in [1, 3, 4, 8]
        assert replay['producer_profile'] == replay['fresh_validator_profile'] == 'tlc'
        assert replay['memory'] == replay['json'] == 'Valid'
        assert 'Next' in replay['wrong_closing_next'] and replay['wrong_closing_next'].startswith('Invalid(')
        assert replay['wrong_init'].startswith('Invalid(')
        assert replay['wrong_property'] != replay['malformed_ending'] != 'Valid'
        assert not replay['model_proof'] and not replay['ordinary_schedule_certified']
    for name in manifest['rows']:
        row = load(native / name)
        assert not row['model_proof'] and not row['ordinary_schedule_certified']
        assert row['supplied_expression'] is True and row['configured_hook'] is False
        width = int(name.rsplit('-w', 1)[1].removesuffix('.json'))
        assert all(i['actual_workers'] == width for i in row['invocations'])
        if name.startswith('live-first'):
            assert not row['hooks'] and row['safety_eof'] is None
            assert row['invocations'][0]['outcome'] == 'Ok(LivenessStopped(2116))'
        else:
            index = int(name.split('-')[1])
            if index in [0, 8]:
                assert row['safety_eof'] is not None and row['stop'] is None
                assert all(h['false_equality'] is False for h in row['hooks'])
                assert all(i['resume'] for i in row['invocations'])
            elif index == 1:
                assert row['hooks'][0]['false_equality'] is True
                assert row['safety_eof'] is None and not row['invocations'][0]['resume']
            else:
                assert row['hooks'][0]['error_code'] == (2179 if index == 7 else 1000)
                assert row['hooks'][0]['origin'] is not None
                assert row['safety_eof'] is None and not row['invocations'][0]['resume']
example = (ROOT / 'crates/dive-api/examples/periodic_hook.rs').read_text().rstrip()
assert '```rust\n' + example + '\n```' in (ROOT / 'docs/periodic-hook.md').read_text()
print('Verified59 actual pinned hook controls and preserved epochs; supplied native diagnostics grant no configured-hook or ordinary proof')
