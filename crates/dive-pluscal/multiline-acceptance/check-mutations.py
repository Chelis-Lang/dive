#!/usr/bin/env python3
"""Reject forged full reports before serialization/admission credit."""
import copy,gzip,hashlib,importlib.util,json
from pathlib import Path
BASE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('full',BASE/'full-controls.py');full=importlib.util.module_from_spec(spec);spec.loader.exec_module(full)
manifest=json.loads((BASE/'primary/manifest.json').read_text());row=next(r for r in manifest['rows'] if r['id']=='P_arithmetic');source=(BASE/'primary/P_arithmetic.tla').read_text();doc=json.loads((BASE/'full-controls/P_arithmetic.accepted.json').read_text())
assert full.integrity(doc,0,source,row,BASE/'primary','tlc')=='full-derivation-and-admission'
mutations=[('boolean-start',['replacement','span','start'],False),('boolean-end',['replacement','span','end'],False),('profile',['profile'],'dive'),('requested-profile',['requested_profile'],'dive'),('overrides',['overrides'],['fake']),('policy',['editing','checksums'],'TranslationOnly'),('stage',['stage'],'checksum-policy'),('proof',['model_proved'],True),('exhaustive',['exhaustive'],True),('exit',['exit_code'],3),('accepted',['accepted'],False),('result',['result'],'unsupported'),('schema',['schema_version'],2),('command',['command'],'check'),('algorithm-unverified',['algorithm_checksum_verified'],False),('body-unverified',['body_checksum_verified'],False),('false-pretty-credit',['exact_primary_generated_bytes'],True),('invalid-admission',['analysis','accepted'],False),('plan-unverified',['replacement','algorithm_checksum_verified'],False),('wrong-ast',['replacement','algorithm_checksum','serialized_ast'],'fake'),('wrong-crc',['replacement','algorithm_checksum','checksum'],'0'),('false-fair',['replacement','algorithm_checksum','fair_header_crc_only'],True),('bad-start',['replacement','span','start'],-1),('bad-end',['replacement','span','end'],len(source.encode())+1),('bad-document',['replacement','module_source'],'fake'),('bad-replacement',['replacement','replacement'],'fake'),('bad-body',['artifact','tla'],'fake'),('bad-body-crc',['replacement','translation_checksum'],'0')]
rows=[]
for name,path,value in mutations+[(f'missing-{k}',[k],None) for k in ['model_proved','exhaustive','algorithm_checksum_verified','body_checksum_verified','replacement','analysis']]:
 changed=copy.deepcopy(doc);target=changed
 for key in path[:-1]:target=target[key]
 if value is None:del target[path[-1]]
 else:target[path[-1]]=value
 try:full.integrity(changed,0,source,row,BASE/'primary','tlc')
 except (AssertionError,KeyError,ValueError,TypeError,StopIteration):rows.append({'control':name,'outcome':'rejected'});continue
 raise AssertionError(('forged report accepted',name))
changed=copy.deepcopy(doc)
for key in ['replacement','artifact','analysis']:del changed[key]
changed.update(accepted=False,result='unsupported',exit_code=3,code='DIVE-P0099',diagnostic={'code':'Internal','message':'internal failure','span':{'start':0,'end':0}})
try:full.integrity(changed,3,source,row,BASE/'primary','tlc')
except (AssertionError,KeyError,ValueError,TypeError):rows.append({'control':'internal-error-masked-as-refusal','outcome':'rejected'})
else:raise AssertionError('internal error masked as refusal')
for kind in ['nested-internal','nested-false-proof']:
 outer=copy.deepcopy(changed);outer.pop('diagnostic');outer.pop('code');inner={'schema_version':1,'command':'translate','exit_code':3,'accepted':False,'model_proved':kind=='nested-false-proof','exhaustive':False,'result':'unsupported','profile':'tlc','diagnostic':{'code':'Internal' if kind=='nested-internal' else 'Unsupported','message':'fake','span':{'start':0,'end':0}}};outer['generation']=inner
 try:full.integrity(outer,3,source,row,BASE/'primary','tlc')
 except (AssertionError,KeyError,ValueError,TypeError):rows.append({'control':kind,'outcome':'rejected'})
 else:raise AssertionError(('nested bad report accepted',kind))
# The reference-weak fair source equality is explicitly reported, not repaired.
report={'schema_version':1,'controls':len(rows),'family_certified':False,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'comparator_sha256':hashlib.sha256((BASE/'full-controls.py').read_bytes()).hexdigest(),'genuine_report_sha256':hashlib.sha256((BASE/'full-controls/P_arithmetic.accepted.json').read_bytes()).hexdigest(),'rows':rows}
(BASE/'mutation-controls.json').write_text(json.dumps(report,indent=2)+'\n');print('Rejected',len(rows),'forged or incomplete Full reports')
