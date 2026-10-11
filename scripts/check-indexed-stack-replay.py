"""Verify preserved indexed-stack replay selection and actual semantic controls."""
import gzip
import hashlib
import itertools
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / 'conformance/probes/indexed-stack-replay/captured-epoch'
index = json.loads((BASE / 'index.json').read_text())
assert not index['family_certified']
for rel, digest in index['files_sha256'].items():
    assert hashlib.sha256((BASE / rel).read_bytes()).hexdigest() == digest, rel

def raw(rel):
    return gzip.decompress((BASE / (rel + '.gz')).read_bytes())

def data(rel):
    return json.loads(raw(rel))

def git_source_proof(epoch, source, producer, claims=None):
    """Check frozen selected blobs through recorded commit/tree objects without Git."""
    prefix = epoch + '/' if epoch else ''
    folder = BASE / epoch / 'producer-git-objects'
    proof = json.loads((folder / 'index.json').read_text())
    assert proof['producer'] == producer
    assert proof['source_index_sha256'] == hashlib.sha256(raw(prefix + 'source-index.json')).hexdigest()
    objects = {}
    for oid, row in proof['objects'].items():
        assert len(oid) == 40 and all(c in '0123456789abcdef' for c in oid)
        packed = (folder / 'objects' / (oid + '.gz')).read_bytes()
        assert hashlib.sha256(packed).hexdigest() == row['sha256']
        value = gzip.decompress(packed)
        assert row['kind'] in ['commit', 'tree']
        assert hashlib.sha1((row['kind'] + ' ' + str(len(value)) + '\0').encode() + value).hexdigest() == oid
        objects[oid] = (row['kind'], value)
    kind, commit = objects[producer]
    assert kind == 'commit'
    tree = commit.splitlines()[0].removeprefix(b'tree ').decode()
    assert tree == proof['tree']
    def entries(oid):
        kind, value = objects[oid]
        assert kind == 'tree'
        parsed = {}; cursor = 0
        while cursor < len(value):
            split = value.index(b' ', cursor); end = value.index(b'\0', split)
            mode = value[cursor:split].decode(); name = value[split+1:end].decode()
            child = value[end+1:end+21].hex()
            assert len(child) == 40 and name not in parsed
            parsed[name] = (mode, child); cursor = end + 21
        assert cursor == len(value)
        return parsed
    claims = proof['blob_oid'] if claims is None else claims
    assert set(claims) == set(source['inputs'])
    for name, record in source['inputs'].items():
        parts = Path(name).parts
        assert not Path(name).is_absolute() and '..' not in parts
        oid = tree
        for part in parts[:-1]:
            mode, oid = entries(oid)[part]
            assert mode == '40000'
        mode, blob = entries(oid)[parts[-1]]
        assert mode in ['100644', '100755'] and blob == claims[name]
        value = raw(prefix + 'source/' + name)
        assert hashlib.sha1(b'blob ' + str(len(value)).encode() + b'\0' + value).hexdigest() == blob
        assert hashlib.sha256(value).hexdigest() == record['sha256']
    return proof

for epoch, producer in [('before-native', 'c2964031b1cdd7aab314ed34dd0cc4be3cce3e7e'),
                        ('first-native', 'bad4a3802c09086a327fe7e895a7738c6db81d5b')]:
    receipt = data(epoch + '/producer.json')
    assert receipt['producer'] == producer and receipt['exit'] == 0
    source = data(epoch + '/source-index.json')
    assert source['producer'] == producer
    proof = git_source_proof(epoch, source, producer)
    forged = dict(proof['blob_oid']); first = sorted(forged)[0]; forged[first] = '0' * 40
    try:
        git_source_proof(epoch, source, producer, forged)
    except AssertionError:
        pass
    else:
        raise AssertionError('Forged path-to-blob claim accepted')
    assert hashlib.sha256(raw(epoch + '/source-index.json')).hexdigest() == receipt['source_index_sha256']
    for name, record in source['inputs'].items():
        assert hashlib.sha256(raw(epoch + '/source/' + name)).hexdigest() == record['sha256'], name

selection = data('before-native/declared-selection.json')['selection']
assert len(selection) == 40 and len({(r['family'], r['case']) for r in selection}) == 40
assert sum(r['family'] == 'sequential' for r in selection) == 32
assert sum(r['family'] == 'process' for r in selection) == 8
for row in selection:
    folder = f"before-native/inputs/{row['family']}/{row['case']}/"
    assert hashlib.sha256(raw(folder + row['module'] + '.tla')).hexdigest() == row['original_source_sha256']
    assert hashlib.sha256(raw(folder + row['module'] + '.cfg')).hexdigest() == row['original_config_sha256']
expected = {(r['family'], r['case'], profile) for r in selection for profile in ['tlc', 'dive']}
for epoch in ['before-native', 'first-native']:
    rows = data(epoch + '/census/receipt.json')['rows']
    assert len(rows) == 80 and {(r['family'], r['case'], r['profile']) for r in rows} == expected
    for row in rows:
        stem = f"{epoch}/census/{row['family']}-{row['case']}-{row['profile']}"
        assert data(stem + '.check.stdout')['verdict'] == row['check_verdict']
        old_refusal = epoch == 'before-native' and row['family'] == 'process' and row['profile'] == 'tlc'
        for kind, result in [('original', 'unsupported' if old_refusal else 'valid'),
                             ('mutant', 'unsupported' if old_refusal else 'invalid')]:
            observed = data(stem + '.' + kind + '.stdout')
            assert observed == row[kind]['result'] and observed['result'] == result
            assert row[kind]['exit'] == {'valid': 0, 'invalid': 1, 'unsupported': 3}[result]
            if result == 'invalid':
                assert observed['failure']['check'] in ['init', 'next', 'ending']
        old = data(f"before-native/census/{row['family']}-{row['case']}-{row['profile']}.check.stdout")
        new = data(f"first-native/census/{row['family']}-{row['case']}-{row['profile']}.check.stdout")
        old.pop('elapsed_ms'); new.pop('elapsed_ms')
        assert old == new
        assert raw(f"before-native/census/{row['family']}-{row['case']}-{row['profile']}.word.json") == raw(f"first-native/census/{row['family']}-{row['case']}-{row['profile']}.word.json")

process = {r['case'] for r in selection if r['family'] == 'process'}
primary = data('before-native/primary-process/receipt.json')
assert primary['clean_primary'] and not primary['dump_trace']
assert primary['workers'] == 1 and primary['seed'] == primary['fp'] == 0
assert primary['jar_sha256'] == '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert len(primary['rows']) == 8 and {r['case'] for r in primary['rows']} == process
for row in primary['rows']:
    assert row['exit'] == 12
    output = raw('before-native/primary-process/' + row['case'] + '.stdout')
    assert hashlib.sha256(output).hexdigest() == row['stdout_sha256']
    counts = re.findall(rb'([0-9,]+) states generated, ([0-9,]+) distinct states found, ([0-9,]+) states left on queue', output)
    depth = re.findall(rb'The depth of the complete state graph search is ([0-9,]+)', output)
    assert counts and depth
    observed = [int(n.replace(b',', b'')) for n in counts[-1]] + [int(depth[-1].replace(b',', b''))]
    assert observed == row['counts']
    native = next(r for r in data('first-native/census/receipt.json')['rows'] if r['case'] == row['case'] and r['profile'] == 'tlc')
    stats = native['statistics']
    assert observed == [stats[k] for k in ['generated', 'distinct', 'left_on_queue', 'depth']]

rows = data('first-native/width-store-transport/receipt.json')['rows']
assert len(rows) == 384
assert {(r['case'], r['profile'], r['store'], r['width'], r['chaos'], r['transport']) for r in rows} == set(itertools.product(process, ['tlc', 'dive'], ['fp', 'exact'], [1, 4, 8], [0, 7], ['v1', 'representation-v2']))
words = {}
for row in rows:
    stem = 'first-native/width-store-transport/' + f"{row['case']}-{row['profile']}-{row['store']}-{row['width']}-{row['chaos']}-{row['transport']}"
    check = data(stem + '.check.stdout')
    assert check['statistics'] == row['statistics']
    workers = row['actual_worker_options']
    assert workers['requested'] == row['width']
    assert workers['used'] == (1 if row['profile'] == 'tlc' else row['width'])
    assert workers['sequential'] == (row['profile'] == 'tlc')
    key = (row['case'], row['profile'], row['transport'])
    word = raw(stem + '.original.word.json')
    assert words.setdefault(key, word) == word
    for kind, result in [('original', 'valid'), ('mutant', 'invalid')]:
        observed = data(stem + '.' + kind + '.stdout')
        assert observed == row['replays'][kind]['result'] and observed['result'] == result
        if result == 'invalid':
            assert observed['failure']['check'] in ['init', 'next', 'ending']

rows = data('first-native/process-semantic-mutants/receipt.json')['rows']
assert len(rows) == 80
assert {(r['case'], r['profile'], r['mutant']) for r in rows} == set(itertools.product(process, ['tlc', 'dive'], ['initial', 'indexed-update', 'return-frame', 'final', 'ending']))
for row in rows:
    stem = 'first-native/process-semantic-mutants/' + f"process-{row['case']}-{row['profile']}.{row['mutant']}"
    assert data(stem + '.stdout') == row['result']
    assert row['exit'] == 1 and row['result']['result'] == 'invalid'
    assert row['result']['failure']['check'] in ['init', 'next', 'ending']

guide = data('first-native/guide-primary-cli/receipt.json')
assert guide['clean_no_dump_trace'] and len(guide['rows']) == 5
assert guide['rows'][0]['name'] == 'primary-clean' and guide['rows'][0]['exit'] == 12
assert guide['rows'][0]['counts'] == [3, 3, 0, 3]
for row in guide['rows'][1:]:
    assert row['counts'] == [3, 3, 0, 3] and row['replay_exit'] == 0 and row['replay'] == 'valid'

mutation = data('false-proof-mutations/receipt.json')
assert mutation['restored'] and len(mutation['rows']) == 2
for row in mutation['rows']:
    assert row['exit'] == 101 and row['original_sha256'] != row['mutated_sha256']

print('Verified preserved40-case before/native census,384 width/store/transport semantic pairs,80 indexed-frame mutants and actual false-proof controls; no family certification')

# Final runtime/source epoch is separately recorded; never relabel first-native.
BASE = ROOT / 'conformance/probes/indexed-stack-replay/qualified-epoch'
index = json.loads((BASE / 'index.json').read_text())
producer = 'dd4edf337c70be1c5b5efdcb72ca805425fd48b1'
assert index['producer'] == producer and not index['family_certified']
for rel, digest in index['files_sha256'].items():
    assert hashlib.sha256((BASE / rel).read_bytes()).hexdigest() == digest, rel
source = data('source-index.json')
assert source['producer'] == producer and len(source['inputs']) == 1461
proof = git_source_proof('', source, producer)
forged = dict(proof['blob_oid']); forged[sorted(forged)[0]] = '0' * 40
try:
    git_source_proof('', source, producer, forged)
except AssertionError:
    pass
else:
    raise AssertionError('Forged final path-to-blob claim accepted')
assert len(source['workspace_rust_paths']) == 485
assert len(source['embedded_references']) == 70 and len(source['embedded_inputs']) == 65
references = []
for path in source['workspace_rust_paths']:
    text = raw('source/' + path).decode()
    matches = list(re.finditer(r'\b(include_str|include_bytes|include)!\s*\(\s*"([^"\\]+)"\s*\)', text))
    assert len(matches) == len(list(re.finditer(r'\b(?:include_str|include_bytes|include)!\s*\(', text)))
    for match in matches:
        import posixpath
        target = posixpath.normpath(str(Path(path).parent / match.group(2)))
        assert not target.startswith('../') and not Path(target).is_absolute()
        references.append({'source': path, 'macro': match.group(1), 'literal': match.group(2),
                           'target': target, 'sha256': hashlib.sha256(raw('source/' + target)).hexdigest()})
assert references == source['embedded_references']
assert set(source['embedded_inputs']) == {row['target'] for row in references}
gate = data('gate/receipt.json')
assert gate['producer'] == producer and gate['profile'] == 'ci'
assert gate['jobs'] == gate['test_threads'] == 2 and gate['per_test_timeout_unchanged']
assert len(gate['rows']) == 8 and [r['stage'] for r in gate['rows']] == list(range(8))
assert all(r['exit'] == 0 for r in gate['rows'])
assert gate['source_index_sha256'] == hashlib.sha256(raw('source-index.json')).hexdigest()
ci = raw('source/.github/workflows/ci.yml').decode()
commands = re.findall(r'^\s+(?:run: )?(python3[^\n]+)$', ci, re.M)
assert commands == gate['ci_python_commands']
assert len(commands) == gate['ci_python_invocations'] == 69
assert len(set(commands)) == gate['ci_python_distinct_commands'] == 64
for row in gate['rows']:
    log = raw('gate/stage-' + str(row['stage']) + '.log')
    assert hashlib.sha256(log).hexdigest() == row['log_sha256']
assert re.findall(r'^COMMAND: (.+)$', raw('gate/stage-0.log').decode(), re.M) == ['cargo fmt --all --check', *commands]
assert re.search(rb'Summary \[[^\]]+\] 1698 tests run: 1698 passed, 0 skipped', raw('gate/stage-2.log'))
assert b'"selected_tests": 1698' in raw('gate/stage-0.log')
native = data('native/producer.json')
assert native['producer'] == producer and native['exit'] == 0
assert native['source_index_sha256'] == hashlib.sha256(raw('native/source-index.json')).hexdigest()
native_source = data('native/source-index.json')
assert native_source['inputs'].keys() == source['inputs'].keys()
assert all(native_source['inputs'][name]['sha256'] == row['sha256'] for name, row in source['inputs'].items())
runtime = data('runtime/receipt.json')
assert runtime['producer'] == producer and len(runtime['steps']) == 9
assert [r['name'] for r in runtime['steps']] == ['build', 'census', 'primary', 'semantic-mutants', 'width-store-transport', 'guide-cli-primary', 'guide-cargo', 'cm1', 'validation-smoke']
for row in runtime['steps']:
    assert row['exit'] == 0
    assert hashlib.sha256(raw('runtime/' + row['name'] + '.log')).hexdigest() == row['log_sha256']
for name, count in [('cm1', 21), ('validation-smoke', 16)]:
    log = raw('runtime/' + name + '.log').decode()
    entries = re.findall(r'^\[\d+/\d+\] (\S+) (.+)$', log, re.M)
    assert len(entries) == count and all(result == 'agree' for result, _ in entries)
    assert f'gate: {count} of {count} models run' in log and 'gate: passed' in log
    step = next(r for r in runtime['steps'] if r['name'] == name)
    assert '--workers' in step['command'] and step['command'][step['command'].index('--workers') + 1] == '1,4,8'
rows = data('native/census/receipt.json')['rows']
assert len(rows) == 80 and {(r['family'], r['case'], r['profile']) for r in rows} == expected
historical = ROOT / 'conformance/probes/indexed-stack-replay/captured-epoch/first-native/census'
for row in rows:
    stem = f"native/census/{row['family']}-{row['case']}-{row['profile']}"
    current = data(stem + '.check.stdout')
    old = json.loads(gzip.decompress((historical / (f"{row['family']}-{row['case']}-{row['profile']}.check.stdout.gz")).read_bytes()))
    current.pop('elapsed_ms'); old.pop('elapsed_ms'); assert current == old
    assert raw(stem + '.word.json') == gzip.decompress((historical / (f"{row['family']}-{row['case']}-{row['profile']}.word.json.gz")).read_bytes())
    for kind, result in [('original', 'valid'), ('mutant', 'invalid')]:
        observed = data(stem + '.' + kind + '.stdout')
        assert observed == row[kind]['result'] and observed['result'] == result
        if result == 'invalid': assert observed['failure']['check'] in ['init', 'next', 'ending']
primary = data('native/primary-process/receipt.json')
assert primary['clean_primary'] and not primary['dump_trace']
assert len(primary['rows']) == 8 and {r['case'] for r in primary['rows']} == process
for row in primary['rows']:
    assert row['exit'] == 12
    output = raw('native/primary-process/' + row['case'] + '.stdout')
    assert hashlib.sha256(output).hexdigest() == row['stdout_sha256']
    counts = re.findall(rb'([0-9,]+) states generated, ([0-9,]+) distinct states found, ([0-9,]+) states left on queue', output)
    depth = re.findall(rb'The depth of the complete state graph search is ([0-9,]+)', output)
    observed = [int(n.replace(b',', b'')) for n in counts[-1]] + [int(depth[-1].replace(b',', b''))]
    native_row = next(r for r in rows if r['case'] == row['case'] and r['profile'] == 'tlc')
    assert observed == row['counts'] == [native_row['statistics'][k] for k in ['generated', 'distinct', 'left_on_queue', 'depth']]
rows = data('native/width-store-transport/receipt.json')['rows']
assert len(rows) == 384
assert {(r['case'], r['profile'], r['store'], r['width'], r['chaos'], r['transport']) for r in rows} == set(itertools.product(process, ['tlc', 'dive'], ['fp', 'exact'], [1, 4, 8], [0, 7], ['v1', 'representation-v2']))
words = {}
for row in rows:
    stem = 'native/width-store-transport/' + f"{row['case']}-{row['profile']}-{row['store']}-{row['width']}-{row['chaos']}-{row['transport']}"
    checked = data(stem + '.check.stdout')
    assert checked['statistics'] == row['statistics']
    assert checked['options']['workers'] == row['actual_worker_options']
    workers = row['actual_worker_options']
    assert workers['requested'] == row['width'] and workers['used'] == (1 if row['profile'] == 'tlc' else row['width'])
    assert workers['sequential'] == (row['profile'] == 'tlc')
    key = (row['case'], row['profile'], row['transport']); word = raw(stem + '.original.word.json')
    assert words.setdefault(key, word) == word
    for kind, result in [('original', 'valid'), ('mutant', 'invalid')]:
        observed = data(stem + '.' + kind + '.stdout')
        assert observed == row['replays'][kind]['result'] and observed['result'] == result
        if result == 'invalid': assert observed['failure']['check'] in ['init', 'next', 'ending']
rows = data('native/process-semantic-mutants/receipt.json')['rows']
assert len(rows) == 80 and {(r['case'], r['profile'], r['mutant']) for r in rows} == set(itertools.product(process, ['tlc', 'dive'], ['initial', 'indexed-update', 'return-frame', 'final', 'ending']))
for row in rows:
    stem = 'native/process-semantic-mutants/' + f"process-{row['case']}-{row['profile']}.{row['mutant']}"
    assert data(stem + '.stdout') == row['result']
    assert row['result']['result'] == 'invalid' and row['result']['failure']['check'] in ['init', 'next', 'ending']
guide = data('native/guide-primary-cli/receipt.json')
assert guide['clean_no_dump_trace'] and len(guide['rows']) == 5
assert guide['rows'][0]['exit'] == 12 and guide['rows'][0]['counts'] == [3, 3, 0, 3]
for row in guide['rows'][1:]:
    assert row['counts'] == [3, 3, 0, 3] and row['replay_exit'] == 0 and row['replay'] == 'valid'
    observed = data('native/guide-primary-cli/' + row['name'] + '.replay.stdout')
    assert observed['result'] == 'valid'
assert b'CLOSED final current runtime9steps plus80exactreport/word equality' in raw('drivers/dive-indexed-stack-final-runtime.log')
print('Verified distinct dd4 frozen8stage/1698zero-skip gate, complete literal census, fresh80/384/80 semantic selections,8clean primary pairs, actual guides and37width gates; no family certification')

current = json.loads((BASE / 'current-source.json').read_text())
assert current['runtime_commit'] == producer
assert set(current['current_sha256']) == set(source['inputs'])
assert all(hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == digest for name, digest in current['current_sha256'].items())
rust = {name for name in source['inputs'] if name.endswith('.rs') or Path(name).name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']}
compiled = set(source['workspace_rust_paths']) | {name for name in rust if not name.endswith('.rs')}
for field, names in [('current_selected_inputs_equal_runtime', source['inputs']), ('current_rust_cargo_equals_runtime', rust), ('current_compiled_rust_cargo_equals_runtime', compiled), ('current_literal_inputs_equal_runtime', source['embedded_inputs'])]:
    actual = all(current['current_sha256'][name] == source['inputs'][name]['sha256'] for name in names)
    assert current[field] == actual, field
