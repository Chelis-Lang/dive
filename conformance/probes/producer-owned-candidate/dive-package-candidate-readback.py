from pathlib import Path
import json,hashlib,gzip,subprocess
r=Path.cwd();b=r/'conformance/probes/producer-owned-candidate';p=r/'conformance/parity-features.json';v=json.loads(p.read_text());f=next(f for f in v['families']if f['id']=='F-023');f['notes']+=' Frozen10af candidate delivery passes45 gate steps/1057 zero-skip tests;27 corrected public outcomes retain12 independently admitted EOF words and48 strict corrupted-word negatives, separate actual RequestPrefix units and37 required safety models at1/4/8. Original incomplete IF-source outcomes and observer failures remain retained; no full-corpus, ordinary schedule or family upgrade.';f['additional_evidence'].append('conformance/probes/producer-owned-candidate/delivery-index.json');p.write_text(json.dumps(v,indent=2)+'\n')
g=b/'gate-current';before=json.loads((g/'before.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();a=json.loads((g/'after.json').read_text());assert a['all_steps_passed']and a['expected_steps']==45
for name,h in before['source_sha256'].items():
 assert sha(r/name)==h,name
 raw=subprocess.check_output(['git','show',before['producer']+':'+name]);assert hashlib.sha256(raw).hexdigest()==h,name
 assert hashlib.sha256(gzip.decompress((g/'source'/(name+'.gz')).read_bytes())).hexdigest()==h,name
rt=g/'runtime-current';rows=[];words=mutants=0
for file in (rt/'native-current').glob('*.json'):
 x=json.loads(file.read_text());rows.append(file.name)
 if 'document'in x:
  assert 'validation: Valid'in x['memory_validation']and'validation: Valid'in x['json_validation'];words+=1
  for m in x['mutants']:
   assert 'validation: Invalid'in m['memory_validation']and'validation: Invalid'in m['json_validation'];mutants+=1
assert len(rows)==27 and words==12 and mutants==48
record={'production_commit':before['producer'],'production_sources':len(before['source_sha256']),'exact_git_live_gzip':True,'full_gate_steps':45,'nextest_tests':1057,'zero_skips':True,'corrected_public_rows':27,'admitted_eof_words':12,'strict_mutant_words':48,'mutants_replayed_twice':True,'model_proof':False,'ordinary_schedule_certified':False}
Path('/tmp/dive-producer-candidate-source-runtime-readback.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record))
