#!/usr/bin/env python3
"""Fail-closed partial-report mutations and primary Validator corruption controls."""
import argparse,copy,gzip,hashlib,importlib.util,json,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('partial',BASE/'partial-controls.py');partial=importlib.util.module_from_spec(spec);spec.loader.exec_module(partial)
p=argparse.ArgumentParser(description=__doc__)
for name in ['java','jar','validator']:p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();assert partial.sha(a.jar.read_bytes())==partial.PIN
manifest=json.loads((BASE/'primary/manifest.json').read_text());row=next(r for r in manifest['rows'] if r['id']=='P_arithmetic');source=(BASE/'primary/P_arithmetic.tla').read_text();doc=json.loads(gzip.decompress((BASE/'partial-controls/P_arithmetic.tlc.stdout.gz').read_bytes()))
assert partial.integrity(doc,0,source,row,BASE/'primary','tlc')=='body-derivation-and-admission'
out=BASE/'partial-negative-controls';out.mkdir(exist_ok=True);rows=[]
mutations=[('boolean-start',['replacement','span','start'],False),('boolean-end',['replacement','span','end'],False),('profile',['profile'],'dive'),('requested-profile',['requested_profile'],'dive'),('overrides',['overrides'],[]),('policy',['editing','checksums'],'Full'),('stage',['stage'],'checksum-policy'),('proof',['model_proved'],True),('exhaustive',['exhaustive'],True),('exit',['exit_code'],3),('boolean-exit',['exit_code'],False),('accepted',['accepted'],False),('result',['result'],'unsupported'),('schema',['schema_version'],2),('boolean-schema',['schema_version'],True),('command',['command'],'check'),('false-algorithm-credit',['algorithm_checksum_verified'],True),('body-unverified',['body_checksum_verified'],False),('false-pretty-credit',['exact_primary_generated_bytes'],True),('invalid-admission',['analysis','accepted'],False),('plan-algorithm-credit',['replacement','algorithm_checksum_verified'],True),('invented-algorithm',['replacement','algorithm_checksum'],{}),('bad-start',['replacement','span','start'],-1),('bad-end',['replacement','span','end'],len(source.encode())+1),('bad-document',['replacement','module_source'],'fake'),('bad-replacement',['replacement','replacement'],'fake'),('bad-body',['artifact','tla'],'fake'),('bad-body-crc',['replacement','translation_checksum'],'0')]
for name,path,value in mutations+[(f'missing-{key}',[key],None) for key in ['model_proved','exhaustive','algorithm_checksum_verified','body_checksum_verified','replacement','analysis']]:
 changed=copy.deepcopy(doc);target=changed
 for key in path[:-1]:target=target[key]
 if value is None:del target[path[-1]]
 else:target[path[-1]]=value
 try:partial.integrity(changed,0,source,row,BASE/'primary','tlc')
 except (AssertionError,KeyError,ValueError,TypeError,StopIteration):rows.append({'control':name,'outcome':'rejected'});continue
 raise AssertionError(('forged BodyOnly report accepted',name))
changed=copy.deepcopy(doc)
for key in ['replacement','artifact','analysis']:del changed[key]
changed.update(accepted=False,result='unsupported',exit_code=3,diagnostic={'code':'Internal','message':'internal failure','span':{'start':0,'end':0}})
try:partial.integrity(changed,3,source,row,BASE/'primary','tlc')
except (AssertionError,KeyError,ValueError,TypeError):rows.append({'control':'internal-error-masked-as-refusal','outcome':'rejected'})
else:raise AssertionError('internal error masked as refusal')
corruptions=[]
for ident in ['P_arithmetic','P_fair_arithmetic']:
 document=json.loads(gzip.decompress((BASE/f'partial-controls/{ident}.tlc.stdout.gz').read_bytes()));module=document['replacement']['module_source']
 for control in ['unchanged','source-changed','body-changed']:
  changed=module
  if control=='source-changed':changed=module.replace('x=0','x=7',1);assert changed!=module
  if control=='body-changed':
   prefix,bodypart=module.split('BEGIN TRANSLATION',1);changed=prefix+'BEGIN TRANSLATION'+bodypart.replace('x = 0','x = 7',1);assert changed!=module
  with tempfile.TemporaryDirectory(prefix='dive-multiline-partial-negative-') as tmp:
   file=Path(tmp)/'A.tla';file.write_text(changed)
   q=subprocess.run([str(a.java),'-cp',str(a.validator)+':'+str(a.jar),'pcal.ChecksumProbe','validate',str(file)],capture_output=True,timeout=30)
   expected='TLA_DIVERGENCE' if control=='body-changed' else 'NO_DIVERGENCE'
   assert q.returncode==0 and expected.encode() in q.stdout,(ident,control,q.stdout,q.stderr)
   capture=partial.save(out,ident+'.'+control,q);corruptions.append({'id':ident,'control':control,'expected':expected,'capture':capture,'source_sha256':partial.sha(changed.encode())})
report={'schema_version':1,'family_certified':False,'script_sha256':partial.sha(Path(__file__).read_bytes()),'comparator_sha256':partial.sha((BASE/'partial-controls.py').read_bytes()),'genuine_report_sha256':partial.sha((BASE/'partial-controls/P_arithmetic.tlc.stdout.gz').read_bytes()),'mutations':rows,'corruptions':corruptions,'oracle_sha256':partial.PIN}
(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Rejected',len(rows),'forged BodyOnly reports;',len(corruptions),'primary corruption dispositions checked')
