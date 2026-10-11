"""Verify the complete declared primary selection without granting native credit."""
from pathlib import Path
import gzip, hashlib, json, re, zlib
root = Path(__file__).resolve().parent
sha = lambda data: hashlib.sha256(data).hexdigest()
count = streams = stages = valid = 0
for folder, expected in [('', 130), ('guide', 2)]:
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
            assert row['sany']['accepted'] is True
            assert row['header'][0] == row['stages']['translated']['crc']
            header = re.search(rb'chksum\(pcal\)\s*=\s*"([0-9a-f]+)"\s*/\\\s*chksum\(tla\)\s*=\s*"([0-9a-f]+)"', (base / 'primary' / (name + '.translated.tla')).read_bytes())
            assert header and [value.decode() for value in header.groups()] == row['header']
            valid += 1
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
assert count == 132 and valid == 118
exploded = 0
for folder, expected in [('', 130)]:
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
assert exploded == 130
selection = json.loads((root / 'model-selection.json').read_text())
assert selection['selected_before_execution'] is True and selection['family_certified'] is False
assert len(selection['source_ids']) == len(set(selection['source_ids'])) == 62
assert set(selection['source_ids']) <= {r['id'] for r in json.loads((root / 'selection.json').read_text())['rows']}
print(f'Verified {count} primary observations (130 retained plus 2 guides), {valid} valid reference artifacts, {stages} original stages, {streams} raw streams and {exploded} separate direct-Explode rows. No native/artifact/model/family credit follows.')
