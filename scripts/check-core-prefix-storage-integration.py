"""Read the closed receiving epoch without promoting it to family parity."""
import gzip
import hashlib
import json
import posixpath
import re
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRODUCER = '2bff2728cc065209ef9743e419dee56ed8ca4ccb'
BASE = ROOT / 'conformance/probes/core-prefix-storage-integration/epochs' / PRODUCER
sha = lambda data: hashlib.sha256(data).hexdigest()
assert sha((BASE / 'index.json').read_bytes()) == 'bd19df272a3e3e937434cf57b65c2c37eff0c38721bf497477ed37dafe6fe926'
index = json.loads((BASE / 'index.json').read_text())
assert index['producer'] == PRODUCER and index['all_receiving_consumers_closed']
assert not index['family_certified']
archives = {}
for name, row in index['archives'].items():
    path = BASE / name
    assert sha(path.read_bytes()) == row['sha256']
    with tarfile.open(path, 'r:gz') as archive:
        members = archive.getmembers()
        assert len({m.name for m in members}) == len(members)
        assert all(m.isfile() and not m.name.startswith('/') and '..' not in Path(m.name).parts for m in members)
        data = {m.name: archive.extractfile(m).read() for m in members}
    assert {name: sha(blob) for name, blob in data.items()} == row['members_sha256']
    archives[name] = data
source_archive = archives['source-and-gate.tar.gz']
source = json.loads(source_archive['source-index.json'])
assert source['source_commit'] == PRODUCER and source['source_tree'] == index['source_tree']
for name, blob in source_archive.items():
    if name.startswith('source/'):
        assert source['source_sha256'][name[7:]] == sha(blob)
gates = json.loads(source_archive['gate/steps.json'])
assert len(gates) == 8 and all(row['exit'] == 0 for row in gates)
for i, row in enumerate(gates):
    assert sha(source_archive['gate/' + str(i) + '.log']) == row['log_sha256']
assert re.search(rb'1267 tests run:\s*1267 passed(?: \(\d+ slow\))?, 0 skipped', source_archive['gate/2.log'])
offline = json.loads(source_archive['offline/steps.json'])
assert len(offline['steps']) == 49 and all(row['exit'] == 0 for row in offline['steps'])
for name in ['dive-core-prefix-storage-nested-resume-2.tar.gz', 'dive-core-prefix-storage-tuple-runtime.tar.gz']:
    closure = json.loads(archives[name]['closure.json'])
    assert closure['source_commit'] == PRODUCER and closure['all_consumers_closed']
# All corrected runtime epochs remain separate from their failed predecessors.
assert len(archives) == 8
supplement_base = BASE / 'embedded-input-supplement'
assert sha((supplement_base / 'index.json').read_bytes()) == '41e1e5ac19d9db74c39dd21b834ef73aaf633b8c3415bf8d305d04a0dee8c08e'
supplement = json.loads((supplement_base / 'index.json').read_text())
assert supplement['producer'] == PRODUCER and not supplement['family_certified']
assert supplement['original_source_index_sha256'] == sha(source_archive['source-index.json'])
compiled = {rel: h for rel, h in source['source_sha256'].items() if rel.endswith('.rs') and
            (re.match(r'^crates/[^/]+/(src|tests|examples|benches)/', rel) or rel.startswith('xtask/src/')
             or re.match(r'^crates/[^/]+/build\.rs$', rel))}
assert supplement['compiled_rust_source_sha256'] == compiled
references = []
for rel in sorted(compiled):
    text = source_archive['source/' + rel].decode()
    matches = list(re.finditer(r'\b(include_str|include_bytes|include)!\s*\(\s*"([^"\\]+)"\s*\)', text))
    assert len(matches) == len(list(re.finditer(r'\b(?:include_str|include_bytes|include)!\s*\(', text))), rel
    for match in matches:
        target = posixpath.normpath(str(Path(rel).parent / match.group(2)))
        assert not target.startswith('../')
        digest = sha(gzip.decompress((supplement_base / 'inputs' / (target + '.gz')).read_bytes()))
        references.append({'source': rel, 'macro': match.group(1), 'literal': match.group(2), 'target': target, 'sha256': digest})
        if target in source['source_sha256']:
            assert digest == source['source_sha256'][target]
assert references == supplement['references']
assert len(references) == 70 and len(compiled) == 460
assert supplement['input_sha256'] == {r['target']: r['sha256'] for r in references}
assert len(supplement['input_sha256']) == 65
assert supplement['original_source_map_missing'] == sorted(set(supplement['input_sha256']) - set(source['source_sha256']))
assert len(supplement['original_source_map_missing']) == 33
for rel, digest in supplement['files_sha256'].items():
    assert sha((supplement_base / rel).read_bytes()) == digest
print('Verified prior receiving epoch: 1267 zero-skip tests, 49 guards, preserved runtime continuations and embedded-input gap; no later integration or family credit')
