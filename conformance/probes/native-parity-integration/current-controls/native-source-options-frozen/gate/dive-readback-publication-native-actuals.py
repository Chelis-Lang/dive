from pathlib import Path
import json,hashlib,subprocess,collections
root=Path('/Users/jeff/code/dive-native-source-options-publication-work');out=Path('/tmp/dive-native-source-options-publication-native-capture');old=Path('/tmp/dive-native-source-options-current-native-capture');bins=Path('/Users/jeff/code/dive-native-source-options-publication-artifacts')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
producer=json.loads((bins/'producer.json').read_text());assert producer['source_commit']==subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
for p,h in producer['source_sha256'].items():assert sha(root/p)==h,p
for p,h in producer['binary_sha256'].items():assert sha(bins/p)==h,p
assert (out/'capture.json').is_file(), 'Capture must already be closed'
rows=[];capture=json.loads((out/'capture.json').read_text());prior=json.loads((old/'capture.json').read_text());assert len(capture['rows'])==len(prior['rows'])==80
for now,before in zip(capture['rows'],prior['rows']):
 assert now['model']==before['model'] and now['requested_workers']==before['requested_workers'] and now['exit']==before['exit'] and not now['timeout'] and now['model_source_sha256']==before['model_source_sha256']
 ident=now['model']+'-'+str(now['requested_workers']);a=json.loads((out/ident/'stdout').read_text());b=json.loads((old/ident/'stdout').read_text());excluded={k:{'current':a.pop(k,None),'historical':b.pop(k,None)} for k in ['file','config','elapsed_ms']};assert a==b,ident
 assert (out/ident/'stderr').read_bytes()==(old/ident/'stderr').read_bytes(),ident
 rows.append({'id':ident,'all_other_fields_identical':True,'excluded_host_observations':excluded})
for p,h in producer['source_sha256'].items():assert sha(root/p)==h,p
for p,h in producer['binary_sha256'].items():assert sha(bins/p)==h,p
record={'source_commit':producer['source_commit'],'historical_source_commit':prior['source_commit'],'selected':80,'rows':rows,'family_certified':False};(out/'independent-comparison.json').write_text(json.dumps(record,indent=2)+'\n');print('80 native controls preserve every comparison field and stderr; only explicit host paths/timing excluded.')
