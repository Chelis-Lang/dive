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
    folder = BASE / epoch / 'producer-git-objects'
    proof = json.loads((folder / 'index.json').read_text())
    assert proof['producer'] == producer
    assert proof['source_index_sha256'] == hashlib.sha256(raw(epoch + '/source-index.json')).hexdigest()
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
        value = raw(epoch + '/source/' + name)
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
