"""Reject rehashed corruptions of a receiving receipt, without running Cargo."""
import gzip
import hashlib
import importlib.util
import io
import json
import tempfile
import tarfile
from pathlib import Path


def encode(value):
    return (json.dumps(value, indent=2) + '\n').encode()


def sha(blob):
    return hashlib.sha256(blob).hexdigest()


def read_archive(path):
    with tarfile.open(path, 'r:gz') as archive:
        return {m.name: archive.extractfile(m).read() for m in archive.getmembers()}


def rewrite_archive(base, index, name, members):
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode='w', format=tarfile.PAX_FORMAT) as archive:
        for rel, blob in sorted(members.items()):
            entry = tarfile.TarInfo(rel)
            entry.size = len(blob)
            entry.mode = 0o644
            entry.mtime = 0
            archive.addfile(entry, io.BytesIO(blob))
    blob = gzip.compress(buffer.getvalue(), mtime=0)
    (base / name).write_bytes(blob)
    index['archives'][name] = {
        'sha256': sha(blob),
        'members_sha256': {rel: sha(data) for rel, data in members.items()},
    }


def controls(reader_path, original):
    spec = importlib.util.spec_from_file_location('receiving_reader', reader_path)
    reader = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(reader)
    original_index = json.loads((original / 'index.json').read_text())
    digest = sha((original / 'index.json').read_bytes())
    reader.verify(original, digest)
    cases = ['family-credit', 'failed-gate', 'skipped-test', 'missing-ci-command',
             'missing-literal', 'wrong-runtime-producer', 'valid-mutant', 'missing-worker']
    rows = []
    with tempfile.TemporaryDirectory(prefix='dive-composed-corruptions-') as temporary:
        for case in cases:
            base = Path(temporary) / case
            base.mkdir()
            index = json.loads(json.dumps(original_index))
            for name in index['archives']:
                (base / name).symlink_to((original / name).resolve())
            if case == 'family-credit':
                index['family_certified'] = True
            elif case in ['failed-gate', 'skipped-test', 'missing-ci-command', 'missing-literal']:
                name = 'source-and-gate.tar.gz'
                data = read_archive(original / name)
                gate = json.loads(data['gate-receipt.json'])
                if case == 'failed-gate':
                    gate['rows'][-1]['exit'] = 1
                    data['gate-receipt.json'] = encode(gate)
                elif case == 'skipped-test':
                    log = data['stage-2.log']
                    assert b'0 skipped' in log
                    data['stage-2.log'] = log.replace(b'0 skipped', b'1 skipped')
                    next(r for r in gate['rows'] if r['name'] == 'stage-2')['log_sha256'] = sha(data['stage-2.log'])
                    data['gate-receipt.json'] = encode(gate)
                elif case == 'missing-ci-command':
                    supplement = json.loads(data['ci-supplement-receipt.json'])
                    supplement['rows'][0]['command'] = ['python3', '-I', 'scripts/unrun.py']
                    data['ci-supplement-receipt.json'] = encode(supplement)
                else:
                    source = json.loads(data['source-index.json'])
                    selected = source['embedded_references'][0]['target']
                    data.pop(source['inputs'][selected]['archive'])
                (base / name).unlink()
                rewrite_archive(base, index, name, data)
            else:
                name = 'fresh-runtime.tar.gz'
                data = read_archive(original / name)
                closure = json.loads(data['closure.json'])
                steps = json.loads(data['steps.json'])
                if case == 'wrong-runtime-producer':
                    closure['producer'] = '0' * 40
                elif case == 'valid-mutant':
                    selected = 'disk-independent-witnesses.stdout'
                    witness = json.loads(data[selected])
                    witness['rows'][0]['mutants'][0]['validation'] = 'valid'
                    data[selected] = encode(witness)
                    next(r for r in steps['rows'] if r['name'] == 'disk-independent-witnesses')['stdout_sha256'] = sha(data[selected])
                    closure['steps'] = steps['rows']
                    data['steps.json'] = encode(steps)
                else:
                    selected = next(n for n in data if n.startswith('examples-reports/') and n.endswith('.json'))
                    report = json.loads(data[selected])
                    report['models'][0]['runs'] = [r for r in report['models'][0]['runs'] if r['workers'] != 8]
                    data[selected] = encode(report)
                data['closure.json'] = encode(closure)
                (base / name).unlink()
                rewrite_archive(base, index, name, data)
            (base / 'index.json').write_bytes(encode(index))
            rejected = False
            try:
                reader.verify(base, sha((base / 'index.json').read_bytes()))
            except (AssertionError, KeyError, ValueError):
                rejected = True
            assert rejected, case
            rows.append({'case': case, 'result': 'rejected'})
    return {'genuine': 'accepted', 'controls': rows, 'family_certified': False}


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    root = Path(__file__).resolve().parents[1]
    parser.add_argument('--reader', type=Path, default=root / 'scripts/check-core-composed-epoch.py')
    parser.add_argument('--base', type=Path, default=root / 'conformance/probes/core-prefix-storage-integration/epochs' / '05b3a1ef498ea1609c0413383b81bebb1b411749')
    args = parser.parse_args()
    print(json.dumps(controls(args.reader, args.base), sort_keys=True))
