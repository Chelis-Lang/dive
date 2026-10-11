"""Complete terminal reference histories and current source integrity; no ordinary proof."""
from pathlib import Path
import json,hashlib,gzip
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/terminal-periodic-service'
load=lambda p:json.loads(p.read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
for name in ['primary-development0','primary','outer-queue']:
    folder=BASE/name
    for path,digest in load(folder/'closed-index.json')['files'].items():
        assert sha((folder/path).read_bytes())==digest,(name,path)
    capture=load(folder/'capture.json');selection=load(folder/'selection.json')
    assert len(capture['rows'])==len(selection)==60
    assert [r['id']for r in capture['rows']]==[r['id']for r in selection]
    assert capture['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
    assert capture['ordinary_native_proof']is False
    for row in capture['rows']:
        assert not row['timeout']
        for path,digest in row['artifact_sha256'].items():
            assert sha((folder/row.get('directory',row['id'])/path).read_bytes())==digest,(name,row['id'],path)
original={r['id']:r for r in load(BASE/'primary-development0/capture.json')['rows']}
primary={r['id']:r for r in load(BASE/'primary/capture.json')['rows']}
observed={r['id']:r for r in load(BASE/'outer-queue/capture.json')['rows']}
for w in [1,4,8]:
    for hook in ['absent','true','false','integer','division','state']:
        key=f'safe-{hook}-w{w}'
        assert original[key]['exit']==13
        assert primary[key]['exit']==0
        assert 'live-finalCheck-return:0'in observed[key]['events']
        assert 'queue-suspendAll-return:false:size=0'in observed[key]['events']
    for hook in ['absent','true','false','error']:
        key=f'violating-{hook}-w{w}'
        events=observed[key]['events']
        assert primary[key]['exit']==13
        assert 'live-finalCheck-return:2116'in events
        assert not any(e.startswith('live-check-enter')for e in events)
        assert events.index('queue-suspendAll-return:false:size=0')<events.index('live-finalCheck-enter')
    for case in ['init-error-absent','init-error-false','next-error-absent','next-error-false']:
        assert not any(e.startswith('live-finalCheck-enter')for e in observed[f'{case}-w{w}']['events'])
    for case in ['missing-next-absent','missing-next-false','missing-hook','nonzero-hook']:
        assert observed[f'{case}-w{w}']['exit']==1
        assert not observed[f'{case}-w{w}']['events']
assert 'cannot inherit from final FastTool'in(BASE/'observer-compile-failed1/compile.log').read_text()
assert 'mismatched types'in(BASE/'word-compile-failed0/compile.log').read_text()
word=(BASE/'word-corrected1/run.log').read_text()
assert 'memory=Report { validation: Valid'in word and 'json=Report { validation: Valid'in word
assert sum(line.startswith('mutant=')and line.count('validation: Invalid')==2 for line in word.splitlines())==4
controlled_folder=BASE/'claimed-queue-controlled3'
for path,digest in load(controlled_folder/'closed-index.json')['files'].items():
    assert sha((controlled_folder/path).read_bytes())==digest,path
controlled=load(controlled_folder/'capture.json')
assert len(controlled['rows'])==12 and not controlled['ordinary_native_proof']
assert [r['id']for r in controlled['rows']]==[r['id']for r in load(controlled_folder/'selection.json')]
for row in controlled['rows']:
    assert not row['timeout']
    for path,digest in row['artifact_sha256'].items():
        assert sha((controlled_folder/row['directory']/path).read_bytes())==digest,(row['id'],path)
    e=row['events']
    assert 'controlled-held-actual-claim:queue-size=0'in e
    assert 'queue-suspendAll-return:true:size=1'in e
    assert e.index('controlled-held-actual-claim:queue-size=0')<e.index('queue-suspendAll-return:true:size=1')
    if row['case']=='safe-true':
        assert 'live-check-return:0'in e and 'live-finalCheck-return:0'in e
        assert any(x.startswith('queue-resumeAll-enter')for x in e)
    else:
        assert not any(x.startswith('live-finalCheck-enter')or x.startswith('queue-resumeAll-enter')for x in e)
        assert ('live-check-return:2116'if row['case']=='violating-false'else 'live-check-return:0')in e
        assert 'checker-error-at-return:0'in e and row['exit']==0
        assert any(x.startswith('worker-at-return:')and x.endswith(':true')for x in e)
    assert e.index('modelCheck-return')<e.index('observer-cleanup-enter')
plain=BASE/'plain-cli-false-hook'
for path,digest in load(plain/'closed-index.json')['files'].items():
    assert sha((plain/path).read_bytes())==digest,path
p=load(plain/'capture.json')
assert p['exit']==10 and not p['timeout']
assert p['oracle_sha256']==controlled['oracle_sha256']
for path,digest in p['artifact_sha256'].items():assert sha((plain/path).read_bytes())==digest,path
codes=[c[0]for c in p['codes']]
assert 2104 not in codes and 2193 not in codes
assert '3913590 states generated, 3913590 distinct states found, 1 states left on queue.'in(plain/'stdout').read_text()
example=(ROOT/'crates/dive-api/examples/terminal_periodic_candidate.rs').read_text().rstrip()
assert '```rust\n'+example+'\n```'in(ROOT/'docs/terminal-periodic-service.md').read_text()
matrix=load(BASE/'source-matrix.json')
assert not matrix['model_proof']and not matrix['ordinary_schedule_certified']and not matrix['family_certified']
for path,digest in matrix['source_sha256'].items():assert sha((ROOT/path).read_bytes())==digest,path
history=load(BASE/'history-before-terminal/manifest.json')
for path,digest in history['matrices_sha256'].items():
    raw=(BASE/'history-before-terminal/matrices'/path).read_bytes()
    assert sha(raw)==digest,path
    assert json.loads(raw)['source_sha256']==history['source_by_matrix'][path]
for digest,path in history['source_archive_paths'].items():
    assert sha(gzip.decompress((ROOT/path).read_bytes()))==digest,path
ledger=load(ROOT/'conformance/parity-features.json')
f=next(f for f in ledger['families']if f['id']=='F-023')
assert f['implementation_status']=='missing'and f['certification']=='audit-required'and 222 in f['issues']
print('Complete192 distinct raw reference/observer rows plus separate unmodified CLI exit10 and independent stutter controls preserved; no ordinary proof')
closed=load(BASE/'gate-current/after.json');compiled=load(BASE/'gate-current/before.json')
assert closed['all_steps_passed']and closed['build_inputs_unchanged']and closed['expected_steps']==47
assert len(compiled['source_sha256'])==483
assert len(compiled['literal_include_sources'])==50 and not compiled['unresolved_include_macros']
for path,digest in compiled['source_sha256'].items():
    assert sha(gzip.decompress((BASE/'gate-current/source'/(path+'.gz')).read_bytes()))==digest,path
assert '1078 tests run: 1078 passed, 0 skipped'in(BASE/'gate-current/workspace.log').read_text()
runtime=BASE/'gate-current/runtime-current'
steps=load(runtime/'runtime-steps.json');assert len(steps)==15 and all(step['exit']==0 for step in steps)
selection=load(runtime/'selection.json');manifest=load(runtime/'native-manifest.json')
assert len(selection['rows'])==len(manifest['rows'])==70
assert {row['id']for row in selection['rows']}=={row['file'].removesuffix('.json')for row in manifest['rows']}
words=mutants=joined=refused=0
for row in manifest['rows']:
    path=runtime/'native-current'/row['file'];assert sha(path.read_bytes())==row['sha256']
    outcome=load(path);assert outcome['model_proof']is False and outcome['ordinary_schedule_certified']is False
    if not outcome['execution_retained']:refused+=1
    if outcome.get('terminal_service')is not None:
        t=outcome['terminal_service'];joined+=1
        assert t['joins_confirmed']and t['left_on_queue']==0 and not t['model_proof']
        assert t['actual_pool'][0]==outcome['requested_width']
    if 'document'in outcome:
        words+=1;assert 'validation: Valid'in outcome['memory_validation']and 'validation: Valid'in outcome['json_validation']
        for mutant in outcome['mutants']:
            mutants+=1;assert 'validation: Invalid'in mutant['memory_validation']and 'validation: Invalid'in mutant['json_validation']
assert (words,mutants,joined,refused)==(12,48,40,6)
for line in(runtime/'default-release-build.log').read_text().splitlines():
    try:artifact=json.loads(line)
    except json.JSONDecodeError:continue
    if artifact.get('reason')=='compiler-artifact':assert 'test-rendezvous'not in artifact.get('features',[])
history=load(BASE/'history-production-586/manifest.json')
for path,digest in history['matrices_sha256'].items():assert sha((BASE/'history-production-586/matrices'/path).read_bytes())==digest,path
for digest,path in history['source_archive_paths'].items():assert sha(gzip.decompress((ROOT/path).read_bytes()))==digest,path
print('Closed47-stage/1078-test producer; complete70 native rows,12EOF words/48strict mutants each independently replayed twice; no ordinary proof')
