"""Semantic corruption controls with recomputed pack hashes; never change originals."""
import argparse, base64, copy, gzip, hashlib, importlib.util, json, tempfile
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--base', type=Path, default=Path(__file__).resolve().parent)
parser.add_argument('--out', type=Path)
args = parser.parse_args()
# Explicit output retains the evidence for review; CI gets an isolated scratch copy.
scratch = tempfile.TemporaryDirectory(prefix='dive-procedure-delivery-controls-') if args.out is None else None
if scratch is not None:
    args.out = Path(scratch.name) / 'controls'
spec = importlib.util.spec_from_file_location('procedure_verifier', args.base / 'verify-delivery.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
digest = lambda data: hashlib.sha256(data).hexdigest()
delivery = json.loads((args.base / 'delivery.json').read_bytes())
original = {row['path']: base64.b64decode(row['data']) for row in
            json.loads(gzip.decompress((args.base / 'runtime-records.json.gz').read_bytes()))}
args.out.mkdir(exist_ok=False)
def json_record(records, path, mutate, zipped=False):
    raw = gzip.decompress(records[path]) if zipped else records[path]
    doc = json.loads(raw)
    mutate(doc)
    raw = json.dumps(doc, indent=2).encode() + b'\n'
    records[path] = gzip.compress(raw, mtime=0) if zipped else raw

def mutate(name, records):
    if name == 'source-byte':
        key = 'final-runtime/source/crates/dive-pluscal/src/translate/checksum.rs.gz'
        records[key] = gzip.compress(gzip.decompress(records[key]) + b'\n', mtime=0)
    elif name == 'first-source-retagged':
        json_record(records, 'first-runtime/producer.json', lambda doc: doc.update(source_commit=module.SOURCE, uncommitted=False))
    elif name == 'reference-input-byte':
        refs = json.loads(records['post-gate-reference-inputs/index.json'])
        key = 'post-gate-reference-inputs/source/' + next(iter(refs['inputs'])) + '.gz'
        records[key] = gzip.compress(gzip.decompress(records[key]) + b'wrong', mtime=0)
    elif name == 'gate-failed':
        json_record(records, 'full-gate-retry/report.json', lambda doc: doc.update(passed=False))
    elif name == 'gate-stage-drop':
        json_record(records, 'full-gate-retry/report.json', lambda doc: doc['completed'].pop())
    elif name == 'gate-skip':
        records['full-gate-retry/46.log'] = b'1191 tests run: 1190 passed, 1 skipped\n'
    elif name == 'failed-command-hidden':
        json_record(records, 'final-runtime/campaign.json', lambda doc: doc['rows'][8].update(exit_code=0))
    elif name == 'repair-raises-quota':
        json_record(records, 'final-runtime/prior-output-driver-repair/campaign.json',
                    lambda doc: doc['rows'][0]['command'].extend(['--larger-bound']))
    elif name == 'raw-drop':
        json_record(records, 'final-runtime/raw-original/report.json', lambda doc: doc['rows'].pop())
    elif name in ['raw-proof', 'raw-ast', 'raw-crc']:
        def change(doc):
            output = doc['rows'][0]['document']
            if name == 'raw-proof':
                output['model_proved'] = True
            elif name == 'raw-ast':
                output['checksum']['Ok']['serialized_ast'] += 'wrong'
            else:
                output['checksum']['Ok']['checksum'] = '0'
        json_record(records, 'final-runtime/raw-original/report.json', change)
    elif name in ['public-proof', 'public-exhaustive', 'public-profile']:
        report = json.loads(records['final-runtime/public-original/report.json'])
        key = 'final-runtime/public-original/' + report['rows'][0]['id'] + '.native.stdout.gz'
        field, value = {'public-proof': ('model_proved', True), 'public-exhaustive': ('exhaustive', True),
                        'public-profile': ('profile', 'unlabelled')}[name]
        json_record(records, key, lambda doc: doc.update({field: value}), zipped=True)
        report['rows'][0]['native']['stdout_sha256'] = digest(gzip.decompress(records[key]))
        records['final-runtime/public-original/report.json'] = json.dumps(report).encode()
    elif name == 'public-cst':
        report = json.loads(records['final-runtime/public-original/report.json'])
        key = 'final-runtime/public-original/' + report['rows'][0]['id'] + '.native-cst.stdout.gz'
        json_record(records, key, lambda doc: doc.update(tree={'wrong': True}), zipped=True)
    elif name == 'validator-divergence':
        json_record(records, 'final-runtime/public-original/report.json',
                    lambda doc: doc['rows'][0]['validator'].update(configured='DIVERGENCE'))
    elif name == 'model-count':
        json_record(records, 'final-runtime/models-new/report.json', lambda doc: doc['rows'][0]['primary_counts'].update(distinct=1))
    elif name == 'model-width-drop':
        json_record(records, 'final-runtime/models-new/report.json', lambda doc: doc['rows'][0]['settings'].pop())
    elif name == 'model-width-duplicated':
        json_record(records, 'final-runtime/models-new/report.json',
                    lambda doc: doc['rows'][0]['settings'].__setitem__(1, copy.deepcopy(doc['rows'][0]['settings'][0])))
    elif name == 'public-selection-duplicated':
        json_record(records, 'final-runtime/public-original/report.json',
                    lambda doc: doc['rows'].__setitem__(1, copy.deepcopy(doc['rows'][0])))
    elif name == 'replay-strict-credit':
        json_record(records, 'final-runtime/replay-new/report.json', lambda doc: doc['summary'].update({'tlc.valid': 1}))
    elif name == 'guide-false-proof':
        report = json.loads(records['markdown-guide-controls/native/report.json'])
        key = 'markdown-guide-controls/native/' + report['rows'][0]['id'] + '.stdout.gz'
        json_record(records, key, lambda doc: doc.update(model_proved=True), zipped=True)
        report['rows'][0]['stdout_sha256'] = digest(gzip.decompress(records[key]))
        records['markdown-guide-controls/native/report.json'] = json.dumps(report).encode()
    elif name == 'crc-proof':
        json_record(records, 'final-runtime/crc/report.json', lambda doc: doc['rows'][0].update(model_proved=True))
    elif name == 'forgery-accepted':
        json_record(records, 'final-runtime/forgeries/report.json', lambda doc: doc['rows'][1].update(exit_code=0))
    elif name == 'primary-mutation-accepted':
        json_record(records, 'primary-forgery-driver-replay/report.json', lambda doc: doc['rows'][1].update(exit_code=0))
    elif name != 'baseline':
        raise ValueError(name)

names = ['baseline', 'source-byte', 'reference-input-byte', 'first-source-retagged', 'gate-failed', 'gate-stage-drop', 'gate-skip',
         'failed-command-hidden', 'repair-raises-quota', 'raw-drop', 'raw-proof', 'raw-ast', 'raw-crc',
         'public-proof', 'public-exhaustive', 'public-profile', 'public-cst', 'validator-divergence',
         'model-count', 'model-width-drop', 'model-width-duplicated', 'public-selection-duplicated',
         'replay-strict-credit', 'guide-false-proof', 'crc-proof',
         'forgery-accepted', 'primary-mutation-accepted']
rows = []
for name in names:
    records = dict(original)
    mutate(name, records)
    with tempfile.TemporaryDirectory(prefix='dive-procedure-portable-forgery-') as tmp:
        base = Path(tmp)
        doc = copy.deepcopy(delivery)
        packed = gzip.compress(json.dumps([{'path': key, 'sha256': digest(data), 'data': base64.b64encode(data).decode()}
                                          for key, data in records.items()], separators=(',', ':')).encode(), mtime=0)
        doc.update(records_archive_sha256=digest(packed), records_count=len(records), raw_bytes=sum(map(len, records.values())),
                   files_sha256={key: digest(data) for key, data in records.items()})
        (base / 'runtime-records.json.gz').write_bytes(packed)
        (base / 'delivery.json').write_text(json.dumps(doc))
        try:
            result = module.verify(base)
            accepted, detail = True, result
        except (ValueError, KeyError, TypeError, AssertionError, FileNotFoundError) as error:
            accepted, detail = False, str(error)
        assert accepted == (name == 'baseline'), (name, detail)
        rows.append({'control': name, 'accepted': accepted, 'detail': detail})
(args.out / 'report.json').write_text(json.dumps({'schema_version': 1, 'source_commit': module.SOURCE,
    'driver_sha256': digest(Path(__file__).read_bytes()), 'verifier_sha256': digest((args.base / 'verify-delivery.py').read_bytes()),
    'selected': len(rows), 'rows': rows, 'family_certified': False, 'all_record_and_archive_hashes_recomputed_before_verification': True}, indent=2) + '\n')
print('Portable verifier accepted genuine evidence and rejected', len(rows) - 1, 'semantic corruptions')
