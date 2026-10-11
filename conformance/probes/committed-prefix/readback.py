from pathlib import Path
import json,hashlib
b=Path(__file__).resolve().parent;r=json.loads((b/'capture.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert len(r['rows'])==24
assert {(x['scenario'],x['width']) for x in r['rows']}=={(s,w) for s in ['later-written','output-before-write','known-error-before','error-during-suspend','hook-false','hook-error','candidate-before-hook','actual-eof'] for w in [1,4,8]}
for name,h in r['sources'].items():assert sha(b/name)==h
checks=[]
for x in r['rows']:
 d=b/f"{x['scenario']}-{x['width']}";out=(d/'stdout').read_text();err=(d/'stderr').read_text();events=[line[6:] for line in out.splitlines() if line.startswith('EVENT\t')]
 assert events==x['events']
 for name,h in x['sha256'].items():assert sha(d/name)==h
 assert not x['timeout'];assert not any('injector-error' in e for e in events)
 if x['scenario']=='hook-error':
  assert x['exit']==2 and 'tlc2.tool.EvalException: The second argument of \\div is 0.' in err
  assert 'ModelChecker.doPeriodicWork' in err and 'AbstractChecker.runTLC' in err
  assert 'cleanup-joined' not in events and 'done' not in events
 else:
  assert x['exit']==0 and 'cleanup-joined' in events and 'done' in events
 if x['scenario']=='known-error-before':assert 'known-worker-error-before-request:2179' in events and 'error-code-at-return:2179' in events
 if x['scenario']=='error-during-suspend':assert 'suspend-enter' in events and 'error-code-at-return:2179' in events
 if x['scenario']=='hook-false':assert 'runTLC-result:2104' in events
 if x['scenario']=='candidate-before-hook':assert 'runTLC-result:2116' in events
 if x['scenario']=='output-before-write' and x['width']>1:
  assert any(e.startswith('root-record:before-request:0:1:') and e.endswith(':-1') for e in events)
  assert any(e.startswith('root-record:suspend-return:0:1:') and not e.endswith(':-1') for e in events)
 checks.append(dict(scenario=x['scenario'],width=x['width'],method_contract='selected actual Worker/queue method path',ordinary_schedule_certificate=False,cleanup_observed='cleanup-joined' in events,cleanup_not_runTLC=True))
receipt=dict(rows=checks,hashes={str(p.relative_to(b)):sha(p) for p in sorted(b.rglob('*')) if p.is_file() and p.name!='independent-readback.json'},ordinary_model_proof=False)
(b/'independent-readback.json').write_text(json.dumps(receipt,indent=2)+'\n');print('PASS',len(checks),len(receipt['hashes']))
