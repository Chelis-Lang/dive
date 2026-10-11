"""Retain actual projection baselines, capture failures and the unsound mutation."""

import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "conformance/probes/fixed-tuple-projection-replay/preimplementation"
index = json.loads((BASE / "index.json").read_text())
assert not index["family_certified"]
assert index["native_baseline_producer"] == "f8b3b51521ba166dd5ce5f8031a8bfb7185acefb"
assert index["native_baseline_consumers"] == 12
for rel, digest in index["files_sha256"].items():
    assert hashlib.sha256((BASE / rel).read_bytes()).hexdigest() == digest, rel
read = lambda rel: gzip.decompress((BASE / (rel + ".gz")).read_bytes())
for folder in ["unchanged-alias", "conditional-assigned"]:
    closure = json.loads(read(folder + "/closure.json"))
    assert closure["all_consumers_closed"] and closure["consumer_count"] == 6
    assert closure["producer"]["source_commit"] == index["native_baseline_producer"]
    rows = json.loads(read(folder + "/rows.json"))["rows"]
    assert len(rows) == 6
    for row in rows:
        data = read(f"{folder}/{row['case']}/{row['label']}.stdout")
        assert hashlib.sha256(data).hexdigest() == row["stdout_sha256"]
        if row["label"].startswith("tlc-"):
            assert json.loads(data)["result"] == "unsupported" and row["exit"] == 3
        elif row["label"].startswith("dive-"):
            expected = "valid" if row["label"].endswith("original") else "invalid"
            assert json.loads(data)["result"] == expected
mutation = json.loads(read("drivers-and-controls/dive-fixed-tuple-projection-mutation-receipt.json"))
assert mutation["exit"] == 101 and mutation["actual"] == "Valid" and mutation["required"] == "Unsupported"
assert mutation["original_restored"]
assert b"left: Valid" in read("drivers-and-controls/dive-fixed-tuple-projection-unsound-permutation-mutant.log")
assert b"17 passed; 2 failed" in read("fixture-spacing-failure/focused.log")
assert b"20 passed; 0 failed" in read("depth-control-clippy-failure/passed20-tests.log")
print("verified original12 projection baselines, genuine false-proof mutation, retained development failures; no new runtime/family certificate")

# Added only after all original frozen consumers close.
EPOCH = ROOT / 'conformance/probes/fixed-tuple-projection-replay/implemented-epoch'
assert EPOCH.is_dir(), 'Implemented frozen qualification epoch is required'
if EPOCH.exists():
    implemented = json.loads((EPOCH/'index.json').read_text())
    assert implemented['producer'] == 'f910f770ab61664cc76dd1bf398f7fefdbba6a61'
    assert implemented['all_consumers_closed'] and not implemented['family_certified']
    for rel, digest in implemented['files_sha256'].items():
        assert hashlib.sha256((EPOCH/rel).read_bytes()).hexdigest() == digest, rel
    def epoch_read(rel):
        return gzip.decompress((EPOCH/(rel+'.gz')).read_bytes())
    for rel, digest in implemented['portable_compile_source_sha256'].items():
        assert hashlib.sha256(epoch_read('source/'+rel)).hexdigest() == digest, rel
    frozen_source=json.loads(epoch_read('source-index.json'))
    assert frozen_source['source_commit']==implemented['producer'] and frozen_source['source_tree']==implemented['source_tree']
    for rel,digest in implemented['portable_compile_source_sha256'].items():
        assert frozen_source['source_sha256'][rel]==digest, rel
    resume=json.loads(epoch_read('gate-resumption/closure.json'))
    assert resume['producer']==implemented['producer'] and resume['all8closed'] and resume['zero_skip_tests']==1219
    assert 'gate-resumption/interrupted-epoch/index.json.gz' in implemented['files_sha256']
    gates=json.loads(epoch_read('gate/steps.json'))
    assert len(gates)==8 and all(row['exit']==0 for row in gates)
    for ordinal,row in enumerate(gates):
        assert hashlib.sha256(epoch_read('gate/'+str(ordinal)+'.log')).hexdigest()==row['log_sha256']
    assert b'1219 tests run: 1219 passed' in epoch_read('gate/2.log') and b'0 skipped' in epoch_read('gate/2.log')
    offline=json.loads(epoch_read('offline/steps.json'))
    assert len(offline['steps'])==44 and all(row['exit']==0 for row in offline['steps'])
    for folder,countkey,count in [('runtime','consumer_count',68),('width-controls','replay_checks',528),('fixed-aliases','consumer_count',24),('conditional-refusal','consumer_count',6),('alias-width-controls','replay_checks',192),('procedure-study','steps',192)]:
        closure=json.loads(epoch_read(folder+'/closure.json'))
        assert closure['producer']['source_commit']==implemented['producer']
        assert closure['all_consumers_closed'] and closure[countkey]==count
    for folder,expected in [('fixed-aliases',('valid','invalid')),('conditional-refusal',('unsupported','unsupported'))]:
        rows=json.loads(epoch_read(folder+'/rows.json'))['rows']
        for row in rows:
            if row['label'].startswith('tlc-'):
                actual=json.loads(epoch_read(folder+'/'+row['case']+'/'+row['label']+'.stdout'))
                assert actual['result']==expected[row['label'].endswith('mutant')]
                assert row['exit']=={'valid':0,'invalid':1,'unsupported':3}[actual['result']]
    projection_widths=json.loads(epoch_read('alias-width-controls/replays.json'))['rows']
    assert len(projection_widths)==192
    assert sum(row['result']=='valid' for row in projection_widths)==96
    assert sum(row['result']=='invalid' for row in projection_widths)==96
    for model in ['flat','nested','parameter','let']:
        assert {(r['profile'],r['workers'],r['store'],r['chaos'],r['kind'])for r in projection_widths if r['model']==model}=={(p,w,s,c,k)for p in ['tlc','dive']for w in [1,4,8]for s in ['fp','exact']for c in [0,13]for k in ['original','mutant']}
    procedure=json.loads(epoch_read('procedure-study/report.json'))
    assert procedure['selected']==32 and procedure['checks']==64 and procedure['replay_controls']==128
    strict_valid=sum(row['profile']=='tlc' and row['control']=='original' and row['result']=='valid'for row in procedure['rows'])
    assert strict_valid==implemented['counts']['strict_procedure_valid']
    for corpus,count in [('examples',21),('validation-smoke',16)]:
        paths=[rel for rel in implemented['files_sha256']if rel.startswith('binary-production/report-'+corpus+'/') and rel.endswith('.json.gz')]
        assert len(paths)==1
        report=json.loads(gzip.decompress((EPOCH/paths[0]).read_bytes()))
        assert report['summary']['models_selected']==report['summary']['models']['agree']==count
        for model in report['models']:
            assert model['outcome']=='agree' and {r['workers']for r in model['runs']}=={1,4,8}
            assert all(r['outcome']=='agree' and not r['diffs']for r in model['runs'])
    print('Verified fixed projection qualification and original procedure study without broadening family status')

# Audited after the full gate: embedded non-Rust inputs were absent from the
# original selected source archive. Preserve that gap and qualify the supplement
# separately; do not assign it an earlier production/runtime timestamp.
import posixpath
import re
SUPPLEMENT = EPOCH.parent / 'embedded-input-supplement'
assert hashlib.sha256((SUPPLEMENT / 'index.json').read_bytes()).hexdigest() == '67ee13539e1be9068974accf9fab7b10b0ba8c1c5ad17bcfa09d0fcf839e6dbc', 'Post-gate Git-qualified input index changed'
supplement = json.loads((SUPPLEMENT / 'index.json').read_text())
assert supplement['producer'] == implemented['producer']
assert not supplement['family_certified'] and supplement['dynamic_includes'] == 0
assert supplement['original_source_index_sha256'] == hashlib.sha256(epoch_read('source-index.json')).hexdigest()
compiled = {rel: digest for rel, digest in frozen_source['source_sha256'].items()
            if rel.endswith('.rs') and (re.match(r'^crates/[^/]+/(src|tests|examples|benches)/', rel)
                or rel.startswith('xtask/src/') or re.match(r'^crates/[^/]+/build\.rs$', rel))}
assert supplement['compiled_rust_source_sha256'] == compiled
expected = []
for rel in sorted(compiled):
    text = epoch_read('source/' + rel).decode()
    matches = list(re.finditer(r'\b(include_str|include_bytes|include)!\s*\(\s*"([^"\\]+)"\s*\)', text))
    assert len(matches) == len(list(re.finditer(r'\b(?:include_str|include_bytes|include)!\s*\(', text))), rel
    for match in matches:
        target = posixpath.normpath(str(Path(rel).parent / match.group(2)))
        assert not target.startswith('../')
        data = gzip.decompress((SUPPLEMENT / 'inputs' / (target + '.gz')).read_bytes())
        digest = hashlib.sha256(data).hexdigest()
        expected.append({'source': rel, 'macro': match.group(1), 'literal': match.group(2),
                         'target': target, 'sha256': digest})
        if target in frozen_source['source_sha256']:
            assert frozen_source['source_sha256'][target] == digest
assert supplement['references'] == expected
assert len(expected) == 67 and len(compiled) == 446
assert supplement['input_sha256'] == {r['target']: r['sha256'] for r in expected}
assert len(supplement['input_sha256']) == 62
assert supplement['original_source_map_missing'] == sorted(set(supplement['input_sha256']) - set(frozen_source['source_sha256']))
assert len(supplement['original_source_map_missing']) == 33
assert supplement['files_sha256'] == {'inputs/' + p + '.gz': hashlib.sha256((SUPPLEMENT / 'inputs' / (p + '.gz')).read_bytes()).hexdigest() for p in supplement['input_sha256']}
gap = json.loads((SUPPLEMENT / 'original-archive-gap.json').read_text())
assert gap['producer'] == implemented['producer'] and gap['original_gate_results_unchanged']
assert gap['missing_from_original_source_map'] == supplement['original_source_map_missing']
assert gap['missing_from_selected_archive'] == sorted(set(supplement['input_sha256']) - set(implemented['portable_compile_source_sha256']))
print('Verified post-gate supplement: 67 literal includes, 62 inputs; original archive gap retained')

controls_dir = EPOCH.parent / 'embedded-input-controls'
controls_index = json.loads((controls_dir / 'index.json').read_text())
assert controls_index['producer'] == implemented['producer'] and not controls_index['new_runtime_credit']
for rel, digest in controls_index['files_sha256'].items():
    assert hashlib.sha256((controls_dir / rel).read_bytes()).hexdigest() == digest
controls = json.loads(gzip.decompress((controls_dir / 'receipt.json.gz').read_bytes()))
assert controls['producer'] == implemented['producer'] and controls['restored_verifier_exit'] == 0
assert {row['label'] for row in controls['controls']} == {'missing-embedded-input', 'rehashed-forged-input', 'omitted-include-site', 'concealed-original-gap', 'retagged-producer'}
assert all(row['exit'] != 0 for row in controls['controls'])
assert controls['index_sha256'] == hashlib.sha256((SUPPLEMENT / 'index.json').read_bytes()).hexdigest()
