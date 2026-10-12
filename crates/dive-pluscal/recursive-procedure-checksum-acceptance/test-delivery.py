"""Semantic corruption controls with recomputed pack hashes; never change originals."""
import argparse, base64, copy, gzip, hashlib, importlib.util, json, tempfile
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--base', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
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
        key = 'producer/source/crates/dive-pluscal/src/translate/checksum.rs.gz'
        records[key] = gzip.compress(gzip.decompress(records[key]) + b'\n', mtime=0)
    elif name == 'first-source-retagged':
        json_record(records, 'declaration-first/index.json', lambda doc: doc.update(native_credit=True))
    elif name == 'reference-input-byte':
        refs = json.loads(records['post-closure-reference-supplement/index.json'])
        key = 'post-closure-reference-supplement/' + refs['inputs'][next(iter(refs['inputs']))]['archive']
        records[key] = gzip.compress(gzip.decompress(records[key]) + b'wrong', mtime=0)
    elif name == 'supplement-retagged':
        json_record(records, 'post-closure-reference-supplement/index.json', lambda doc: doc.update(not_original_producer_inputs=False))
    elif name == 'driver-restore-byte':
        records['command-path-repairs/drivers/compare.py'] += b'wrong'
    elif name == 'model-selection-drop':
        json_record(records, 'final-runtime/models-new/report.json', lambda doc: doc['selected_source_ids'].pop())
    elif name == 'replay-duplicated':
        json_record(records, 'final-runtime/replay-new/report.json', lambda doc: doc['rows'].__setitem__(1, copy.deepcopy(doc['rows'][0])))
    elif name == 'bounded-proof':
        report = json.loads(records['final-runtime/bounded-nontermination/report.json']);key = 'final-runtime/bounded-nontermination/' + report['rows'][0]['id'] + '.stdout.gz'
        json_record(records,key,lambda doc: doc.update(model_proved=True),zipped=True)
        report['rows'][0]['stdout_sha256'] = digest(gzip.decompress(records[key]));records['final-runtime/bounded-nontermination/report.json'] = json.dumps(report).encode()
    elif name == 'gate-failed':
        json_record(records, 'full-gate/report.json', lambda doc: doc.update(passed=False))
    elif name == 'gate-stage-drop':
        json_record(records, 'full-gate/report.json', lambda doc: doc['completed'].pop())
    elif name == 'gate-skip':
        records['full-gate/48.log'] = b'1203 tests run: 1202 passed, 1 skipped\n'
    elif name == 'failed-command-hidden':
        json_record(records, 'final-runtime/campaign.json', lambda doc: doc['rows'][10].update(exit_code=0))
    elif name == 'repair-raises-quota':
        json_record(records, 'command-path-repairs/campaign.json',
                    lambda doc: doc['rows'][0]['command'].extend(['--larger-bound']))
    elif name == 'gate-repair-extra-flags':
        json_record(records, 'final-runtime/commands.json', lambda doc: doc[19].__setitem__(-1, doc[19][-1].replace("'--dive-bin'", "'--larger-bound', '--dive-bin'")))
    elif name == 'raw-drop':
        json_record(records, 'final-runtime/raw-new/report.json', lambda doc: doc['rows'].pop())
    elif name in ['raw-proof', 'raw-ast', 'raw-crc']:
        def change(doc):
            output = doc['rows'][0]['document']
            if name == 'raw-proof':
                output['model_proved'] = True
            elif name == 'raw-ast':
                output['checksum']['Ok']['serialized_ast'] += 'wrong'
            else:
                output['checksum']['Ok']['checksum'] = '0'
        json_record(records, 'final-runtime/raw-new/report.json', change)
    elif name in ['public-proof', 'public-exhaustive', 'public-profile']:
        report = json.loads(records['final-runtime/public-new/report.json'])
        key = 'final-runtime/public-new/' + report['rows'][0]['id'] + '.native.stdout.gz'
        field, value = {'public-proof': ('model_proved', True), 'public-exhaustive': ('exhaustive', True),
                        'public-profile': ('profile', 'unlabelled')}[name]
        json_record(records, key, lambda doc: doc.update({field: value}), zipped=True)
        report['rows'][0]['native']['stdout_sha256'] = digest(gzip.decompress(records[key]))
        records['final-runtime/public-new/report.json'] = json.dumps(report).encode()
    elif name == 'public-cst':
        report = json.loads(records['final-runtime/public-new/report.json'])
        key = 'final-runtime/public-new/' + report['rows'][0]['id'] + '.native-cst.stdout.gz'
        json_record(records, key, lambda doc: doc.update(tree={'wrong': True}), zipped=True)
    elif name == 'validator-divergence':
        json_record(records, 'final-runtime/public-new/report.json',
                    lambda doc: doc['rows'][0]['validator'].update(configured='DIVERGENCE'))
    elif name == 'model-count':
        json_record(records, 'final-runtime/models-new/report.json', lambda doc: doc['rows'][0]['primary_counts'].update(distinct=1))
    elif name == 'model-width-drop':
        json_record(records, 'final-runtime/models-new/report.json', lambda doc: doc['rows'][0]['settings'].pop())
    elif name == 'model-width-duplicated':
        json_record(records, 'final-runtime/models-new/report.json',
                    lambda doc: doc['rows'][0]['settings'].__setitem__(1, copy.deepcopy(doc['rows'][0]['settings'][0])))
    elif name == 'public-selection-duplicated':
        json_record(records, 'final-runtime/public-new/report.json',
                    lambda doc: doc['rows'].__setitem__(1, copy.deepcopy(doc['rows'][0])))
    elif name == 'replay-strict-credit':
        json_record(records, 'final-runtime/replay-new/report.json', lambda doc: doc['summary'].update({'tlc.valid': 1}))
    elif name == 'guide-false-proof':
        report = json.loads(records['final-runtime/guides-final/report.json'])
        key = 'final-runtime/guides-final/' + report['rows'][0]['id'] + '.stdout.gz'
        json_record(records, key, lambda doc: doc.update(model_proved=True), zipped=True)
        report['rows'][0]['stdout_sha256'] = digest(gzip.decompress(records[key]))
        records['final-runtime/guides-final/report.json'] = json.dumps(report).encode()
    elif name == 'crc-proof':
        json_record(records, 'final-runtime/crc/report.json', lambda doc: doc['rows'][0].update(model_proved=True))
    elif name == 'forgery-accepted':
        json_record(records, 'final-runtime/forgeries/report.json', lambda doc: doc['rows'][1].update(exit_code=0))
    elif name == 'primary-mutation-accepted':
        json_record(records, 'primary-integrity-mutations/report.json', lambda doc: doc['rows'][1].update(exit_code=0))
    elif name != 'baseline':
        raise ValueError(name)

names = ['baseline', 'supplement-retagged', 'driver-restore-byte', 'model-selection-drop', 'replay-duplicated', 'bounded-proof', 'source-byte', 'reference-input-byte', 'first-source-retagged', 'gate-failed', 'gate-stage-drop', 'gate-skip',
         'failed-command-hidden', 'repair-raises-quota', 'gate-repair-extra-flags', 'raw-drop', 'raw-proof', 'raw-ast', 'raw-crc',
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
    'records_archive_sha256': digest((args.base / 'runtime-records.json.gz').read_bytes()), 'delivery_json_sha256': digest((args.base / 'delivery.json').read_bytes()), 'selected': len(rows), 'rows': rows, 'family_certified': False, 'all_record_and_archive_hashes_recomputed_before_verification': True}, indent=2) + '\n')
print('Portable verifier accepted genuine evidence and rejected', len(rows) - 1, 'semantic corruptions')
