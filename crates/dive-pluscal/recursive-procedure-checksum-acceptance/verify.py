"""Verify the complete declared primary selection without granting native credit."""
from pathlib import Path
import gzip, hashlib, json, re, zlib
root = Path(__file__).resolve().parent
sha = lambda data: hashlib.sha256(data).hexdigest()
count = streams = stages = valid = 0
for folder, expected in [('', 162), ('tail-ownership', 8), ('tail-ownership-first', 8)]:
    base = root / folder
    selected = json.loads((base / 'selection.json').read_text())
    manifest = json.loads((base / 'primary/manifest.json').read_text())
    assert manifest['oracle_sha256'] == '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
    assert manifest['selected'] == expected == len(manifest['rows']) == len(selected['rows'])
    assert [r['id'] for r in selected['rows']] == [r['id'] for r in manifest['rows']]
    assert len({r['id'] for r in manifest['rows']}) == expected
    assert manifest['selection_sha256'] == sha((base / 'selection.json').read_bytes())
    for row in manifest['rows']:
        name = row['id']
        assert sha((base / 'cases' / (name + '.tla')).read_bytes()) == row['input_sha256']
        assert sha((base / 'primary' / (name + '.translated.tla')).read_bytes()) == row['output_sha256']
        if row['translated'] and not row['reference_errors']:
            assert row['header'][0] == row['stages']['translated']['crc']
            header = re.search(rb'chksum\(pcal\)\s*=\s*"([0-9a-f]+)"\s*/\\\s*chksum\(tla\)\s*=\s*"([0-9a-f]+)"', (base / 'primary' / (name + '.translated.tla')).read_bytes())
            assert header and [value.decode() for value in header.groups()] == row['header']
            if row['sany']['accepted'] is True:
                valid += 1
            else:
                assert name in {'RecursiveHeader_C_zero_param_self_tail', 'RecursiveHeader_P_zero_param_self_tail'}
                assert row['sany']['accepted'] is False
        for tag, stage in row['stages'].items():
            for suffix in ['ast', 'json']:
                assert sha((base / 'primary' / (name + '.' + tag + '.' + suffix)).read_bytes()) == stage[suffix + '_sha256']
            assert format(zlib.crc32((base / 'primary' / (name + '.' + tag + '.ast')).read_bytes()), 'x') == stage['crc']
            stages += 1
        for tag in ['primary', 'stages_process', 'sany', 'validator']:
            if tag not in row:
                continue
            for stream in ['stdout', 'stderr']:
                meta = row[tag][stream]
                if isinstance(meta, str):
                    assert tag == 'validator' and stream == 'stdout'
                    assert gzip.decompress((base / 'primary' / (name + '.validator.stdout.gz')).read_bytes()) == meta.encode()
                else:
                    assert sha(gzip.decompress((base / 'primary' / meta['path']).read_bytes())) == meta['sha256']
                streams += 1
        count += 1
assert count == 178 and valid == 159
exploded = 0
for folder, expected in [('', 162), ('tail-ownership', 8), ('tail-ownership-first', 8)]:
    base = root / folder
    manifest = json.loads((base / 'exploded/manifest.json').read_text())
    selection = json.loads((base / 'selection.json').read_text())
    assert manifest['native_credit'] is False and manifest['family_certified'] is False
    assert manifest['selected'] == expected == len(manifest['rows'])
    assert manifest['selection_sha256'] == sha((base / 'selection.json').read_bytes())
    assert [r['id'] for r in selection['rows']] == [r['id'] for r in manifest['rows']]
    for row in manifest['rows']:
        name = row['id']
        assert sha((base / 'cases' / (name + '.tla')).read_bytes()) == row['input_sha256']
        for tag, stage in row['stages'].items():
            for suffix in ['ast', 'json']:
                assert sha((base / 'exploded' / (name + '.' + tag + '.' + suffix)).read_bytes()) == stage[suffix + '_sha256']
            assert format(zlib.crc32((base / 'exploded' / (name + '.' + tag + '.ast')).read_bytes()), 'x') == stage['crc']
        for stream in ['stdout', 'stderr']:
            meta = row[stream]
            assert sha(gzip.decompress((base / 'exploded' / meta['path']).read_bytes())) == meta['sha256']
        exploded += 1
assert exploded == 178
pilot = root / 'declaration-wrapper-negative'
selection = json.loads((pilot / 'selection.json').read_text())
manifest = json.loads((pilot / 'primary/manifest.json').read_text())
assert len(manifest['rows']) == len(selection['rows']) == 16
assert manifest['selection_sha256'] == sha((pilot / 'selection.json').read_bytes())
for row in manifest['rows']:
    name = row['id']
    assert not row['translated'] and row['reference_errors'] and row['header'] is None
    assert sha((pilot / 'cases' / (name + '.tla')).read_bytes()) == row['input_sha256']
    for stream in ['stdout', 'stderr']:
        meta = row['primary'][stream]
        assert sha(gzip.decompress((pilot / 'primary' / meta['path']).read_bytes())) == meta['sha256']
    assert b'Expected "{" but found "L"' in gzip.decompress((pilot / 'primary' / row['primary']['stdout']['path']).read_bytes())
main = json.loads((root / 'primary/manifest.json').read_text())
for syntax in ['C', 'P']:
    row = next(r for r in main['rows'] if r['id'] == f'RecursiveHeader_{syntax}_zero_param_self_tail')
    assert row['translated'] and not row['reference_errors'] and row['sany']['accepted'] is False
print(f'Verified {count} original primary observations, {valid} SANY-valid artifacts, {stages} stored stages, {streams} raw streams and {exploded} separate exploded rows. Retained16 first-wrapper errors and2 primary-invalid self-tail artifacts have no positive credit; no native/family/model proof follows.')

first = json.loads((root / 'tail-ownership-first/primary/manifest.json').read_text())
assert len(first['rows']) == 8
failed = [r for r in first['rows'] if r['reference_errors']]
assert {r['id'] for r in failed} == {'RecursiveTail_C_self_tail_local', 'RecursiveTail_C_mutual_tail_local', 'RecursiveTail_C_mutual_tail_reverse'}
assert all(not r['translated'] and r['header'] is None for r in failed)
corrected = json.loads((root / 'tail-ownership/primary/manifest.json').read_text())
assert len(corrected['rows']) == 8 and all(r['sany']['accepted'] for r in corrected['rows'])
print('Corrected8 tail-ownership controls preserve first8 captures with3 primary C grammar errors; no old errors gain positive credit.')
