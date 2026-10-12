"""Rehash derived portable packets, execute the real reader, preserve originals."""
import argparse
import copy
import gzip
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

sha = lambda data: hashlib.sha256(data).hexdigest()
FINAL = '/tmp/dive-core-terminal-composed-runtime-process-resume-6b'
GATE = '/tmp/dive-core-terminal-qualified-epoch'
PARSER = argparse.ArgumentParser()
PARSER.add_argument('--base', type=Path, required=True)
PARSER.add_argument('--index-sha', required=True)
PARSER.add_argument('--out', type=Path, required=True)
args = PARSER.parse_args()
assert not args.out.exists()
args.out.mkdir()
original_index = (args.base / 'index.json').read_bytes()
assert sha(original_index) == args.index_sha
original = json.loads(original_index)
original_payload_sha = sha((args.base / 'payload.tar.gz').read_bytes())
assert original_payload_sha == original['payload_sha256']
with tarfile.open(args.base / 'payload.tar.gz', 'r:gz') as archive:
    blobs = {member.name[8:]: archive.extractfile(member).read() for member in archive.getmembers()}
assert all(sha(data) == digest for digest, data in blobs.items())
reader = args.base / 'verify.py'
assert sha(reader.read_bytes()) == original['reader_sha256']

def invoke(base, digest, stem):
    command = [sys.executable, '-I', str(reader), '--base', str(base), '--index-sha', digest]
    result = subprocess.run(command, capture_output=True)
    (args.out / (stem + '.stdout')).write_bytes(result.stdout)
    (args.out / (stem + '.stderr')).write_bytes(result.stderr)
    return result, command

positive, command = invoke(args.base, args.index_sha, 'genuine')
assert positive.returncode == 0, positive.stderr.decode()
rows = [{'name': 'genuine', 'exit': 0, 'command': command, 'index_sha256': args.index_sha}]

class Packet:
    def __init__(self):
        self.index = copy.deepcopy(original)
        self.new = {}
        self.changes = []
    def raw(self, path):
        digest = self.index['files'][path]['sha256']
        return self.new.get(digest, blobs.get(digest))
    def doc(self, path):
        return json.loads(self.raw(path))
    def replace(self, path, data):
        before = self.raw(path)
        assert before != data, path
        old, new = sha(before), sha(data)
        self.new[new] = data
        aliases = []
        for name, record in self.index['files'].items():
            if record['sha256'] == old:
                record['sha256'], record['bytes'] = new, len(data)
                aliases.append(name)
        self.changes.append({'target': path, 'before_sha256': old, 'after_sha256': new,
                             'aliases_rehashed': aliases})
    def replace_doc(self, path, doc):
        self.replace(path, (json.dumps(doc, indent=2) + '\n').encode())
    def save(self, base):
        used = {row['sha256'] for row in self.index['files'].values()}
        with (base / 'payload.tar.gz').open('wb') as raw:
            with gzip.GzipFile(fileobj=raw, mode='wb', filename='', mtime=0) as zipped:
                with tarfile.open(fileobj=zipped, mode='w|') as archive:
                    for digest in sorted(used):
                        data = self.new.get(digest, blobs.get(digest))
                        assert sha(data) == digest
                        info = tarfile.TarInfo('payload/' + digest)
                        info.size, info.mode, info.mtime = len(data), 0o444, 0
                        archive.addfile(info, io.BytesIO(data))
        self.index['payload_sha256'] = sha((base / 'payload.tar.gz').read_bytes())
        index = (json.dumps(self.index, indent=2) + '\n').encode()
        (base / 'index.json').write_bytes(index)
        return sha(index)

def report_path(packet, family):
    found = [p for p in packet.index['files'] if p.startswith(FINAL + '/' + family + '-reports/') and p.endswith('.json')]
    assert len(found) == 1
    return found[0]

def mutate_worker(packet, duplicate=False):
    path = report_path(packet, 'examples')
    doc = packet.doc(path)
    row = doc['models'][0]
    if duplicate:
        run = next(r for r in row['runs'] if r['workers'] == 8)
        run['workers'] = 4
    else:
        row['runs'] = [r for r in row['runs'] if r['workers'] != 8]
    packet.replace_doc(path, doc)

def proof_row(packet):
    prefix = FINAL + '/optional-periodic-hook-rows/'
    path = next(p for p in packet.index['files'] if p.startswith(prefix) and p.endswith('.json'))
    doc = packet.doc(path)
    assert doc['model_proof'] is False
    doc['model_proof'] = True
    packet.replace_doc(path, doc)

def proof_contract(packet):
    path = FINAL + '/optional-periodic-hook-receiving-contract.json'
    doc = packet.doc(path)
    assert doc['model_proof'] is False
    doc['model_proof'] = True
    packet.replace_doc(path, doc)

def false_fork(packet):
    row = packet.index['failed_runtime_fork']['retained_failure_row']
    assert row['exit'] == 1
    row['exit'] = 0
    packet.changes.append({'index_semantic_field': 'failed_runtime_fork.retained_failure_row.exit', 'before': 1, 'after': 0})

def false_origin(packet):
    row = packet.index['ordered_runtime_bindings'][21]
    assert row['producer_directory'] != FINAL
    before = row['producer_directory']
    row['producer_directory'] = FINAL
    packet.changes.append({'index_semantic_field': 'ordered_runtime_bindings[21].producer_directory', 'before': before, 'after': FINAL})

MODELS = FINAL + '/process-procedure-models/report.json'
def duplicate_setting(packet):
    doc = packet.doc(MODELS)
    setting = next(r for r in doc['rows'][0]['settings'] if r['workers'] == 8)
    setting['workers'] = 4
    packet.replace_doc(MODELS, doc)

def wrong_primary_count(packet):
    doc = packet.doc(MODELS)
    doc['rows'][0]['primary_counts']['generated'] += 1
    packet.replace_doc(MODELS, doc)

def replace_stream(packet, saved, mutate):
    path = FINAL + '/process-procedure-models/' + saved['stdout']['path']
    raw = gzip.decompress(packet.raw(path))
    doc = json.loads(raw)
    mutate(doc)
    changed = (json.dumps(doc, indent=2) + '\n').encode()
    assert raw != changed
    packet.replace(path, gzip.compress(changed, mtime=0))
    return sha(raw), sha(changed), saved['stdout']['path']

def generation_proof(packet):
    doc = packet.doc(MODELS)
    saved = doc['rows'][0]['generation']
    old, new, target = replace_stream(packet, saved, lambda word: word.__setitem__('model_proved', True))
    count = 0
    for row in doc['rows']:
        if row['generation']['stdout']['path'] == target:
            assert row['generation']['stdout']['sha256'] == old
            row['generation']['stdout']['sha256'] = new
            count += 1
    assert count >= 1
    packet.replace_doc(MODELS, doc)

def false_native_success(packet):
    doc = packet.doc(MODELS)
    row = next(r for r in doc['rows'] if r['id'].endswith('.violation'))
    saved = row['settings'][0]['capture']
    old, new, target = replace_stream(packet, saved, lambda word: word.__setitem__('verdict', 'success'))
    assert saved['stdout']['sha256'] == old and saved['exit_code'] in [10, 12]
    saved['stdout']['sha256'] = new
    saved['exit_code'] = 0
    packet.replace_doc(MODELS, doc)

def false_component_replay(packet):
    path = FINAL + '/terminal-outcome-receiving-contract.json'
    contract = packet.doc(path)
    row = next(r for r in contract['commands'] if r['exit'] == 1)
    word_path = row['command'][row['command'].index('--trace') + 1]
    stdout_path = word_path[:-5] + '.stdout'
    report = packet.doc(stdout_path)
    assert report['result'] == 'invalid'
    report['result'] = 'valid'
    packet.replace_doc(stdout_path, report)
    row['stdout_sha256'] = sha(packet.raw(stdout_path))
    row['exit'] = 0
    packet.replace_doc(path, contract)

def false_source(packet):
    path = GATE + '/source-index.json'
    source = packet.doc(path)
    source['producer'] = '0' * 40
    packet.replace_doc(path, source)

controls = [
    ('omitted-conformance-worker', mutate_worker, "assert row['outcome'] == 'agree'"),
    ('duplicate-conformance-worker', lambda p: mutate_worker(p, True), "assert row['outcome'] == 'agree'"),
    ('forged-component-model-proof', proof_row, "assert row['model_proof'] is False"),
    ('forged-contract-model-proof', proof_contract, "assert contract['model_proof'] is False"),
    ('failed-fork-retagged-success', false_fork, "assert fork['parent'] == failed_indexed"),
    ('original-log-retagged-final-origin', false_origin, "index['ordered_runtime_bindings'] == expected_bindings"),
    ('duplicate-finite-model-worker', duplicate_setting, "for r in row['settings']"),
    ('forged-primary-state-count', wrong_primary_count, "assert row['primary_counts'] == expected"),
    ('artifact-generation-claims-model-proof', generation_proof, "generated['model_proved'] is False"),
    ('invariant-violation-forged-success', false_native_success, "assert saved['exit_code'] =="),
    ('invalid-component-replay-forged-valid', false_component_replay, "assert (actual_valid, actual_invalid) == (valid, invalid)"),
    ('invented-original-producer', false_source, "assert source['producer'] == supplement['producer'] == PRODUCER"),
]
for name, mutation, expected in controls:
    packet = Packet()
    mutation(packet)
    assert packet.changes, name
    # Serial one-derived-archive limit; no full packet retained per mutation.
    with tempfile.TemporaryDirectory(prefix='dive-v3-derived-', dir=args.out) as tmp:
        derived = Path(tmp)
        digest = packet.save(derived)
        result, command = invoke(derived, digest, name)
        stderr = result.stderr.decode(errors='replace')
        correct = result.returncode != 0 and expected in stderr
        row = {'name': name, 'genuine_mutation': True, 'semantic_rejection_verified': correct,
               'exit': result.returncode, 'command': command, 'derived_index_sha256': digest,
               'expected_semantic_assertion': expected, 'changes': packet.changes,
               'stdout_sha256': sha(result.stdout), 'stderr_sha256': sha(result.stderr)}
        rows.append(row)
        (args.out / (name + '.changes.json')).write_text(json.dumps(row, indent=2) + '\n')
        (args.out / 'controls.json').write_text(json.dumps({'original_index_sha256': args.index_sha,
            'original_payload_sha256': original_payload_sha, 'reader_sha256': sha(reader.read_bytes()),
            'driver_sha256': sha(Path(__file__).read_bytes()), 'closed': False, 'rows': rows}, indent=2) + '\n')
        print(name, result.returncode, correct, flush=True)
        assert correct, stderr
assert (args.base / 'index.json').read_bytes() == original_index
assert sha((args.base / 'payload.tar.gz').read_bytes()) == original_payload_sha
receipt = {'original_index_sha256': args.index_sha, 'original_payload_sha256': original_payload_sha,
           'reader_sha256': sha(reader.read_bytes()), 'driver_sha256': sha(Path(__file__).read_bytes()),
           'closed': True, 'genuine': 1, 'semantic_negatives': len(controls),
           'all_rejected_at_intended_semantic_checks': True, 'original_unchanged': True,
           'Cargo_executed': False, 'full_derived_packets_retained': 0, 'rows': rows}
(args.out / 'controls.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({'genuine': 1, 'semantic_negatives': len(controls), 'closed': True}))
