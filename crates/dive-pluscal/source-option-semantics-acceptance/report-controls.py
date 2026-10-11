from pathlib import Path
import ast,json,gzip,copy,hashlib,argparse
B=Path(__file__).resolve().parent;A=B/'executed-runners';p=argparse.ArgumentParser();p.add_argument('--out',default='comparison-negative-replay');args=p.parse_args();O=B/args.out;O.mkdir(exist_ok=False)
def sha(b):return hashlib.sha256(b).hexdigest()
s=(A/'compare-stage-separated.py.txt').read_text();tree=ast.parse(s);fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='check');ns={};exec(compile(ast.Module(body=[fn],type_ignores=[]),'executed-compare-stage-separated.py','exec'),ns);check=ns['check']
F=B/'primary-v2';row=next(r for r in json.loads((F/'manifest.json').read_text())['rows']if r['id']=='P_plain_weak_cli_sf');ident=row['id']+'_tlc';doc=json.loads(gzip.decompress((B/'stage-separated'/(ident+'.stdout.gz')).read_bytes()));assert check(doc,row,F,(F/(row['id']+'.tla')).read_bytes())[0]=='exact-raw-ast'
mutations={
'forged-proof':lambda d:d.update(model_proved=True),
'forged-artifact-admission':lambda d:d.update(artifact_admitted=True),
'wrong-crc':lambda d:d['checksum']['Ok'].update(checksum='00000000'),
'wrong-serialized-ast':lambda d:d['checksum']['Ok'].update(serialized_ast=d['checksum']['Ok']['serialized_ast']+' '),
'wrong-effective-fairness':lambda d:d['translation']['Ok']['option_context']['effective'].update(fairness='Strong'),
'wrong-effective-root':lambda d:d['translation']['Ok']['option_context']['effective'].update(label_root='WrongRoot'),
'wrong-effective-labels':lambda d:d['translation']['Ok']['option_context']['effective'].update(insert_missing_labels=True),
'wrong-source-argument-span':lambda d:d['checksum']['Ok']['option_context']['source_declaration']['arguments'][0]['span'].update(start=0),
'wrong-source-argument':lambda d:d['checksum']['Ok']['option_context']['source_declaration']['arguments'][0].update(text='-sf'),
'fabricated-success-refusal':lambda d:d['translation'].update(Err={'diagnostic':{'code':'Unsupported'}}),
}
results=[]
for name,f in mutations.items():
 d=copy.deepcopy(doc);f(d);(O/(name+'.json')).write_text(json.dumps(d,indent=2)+'\n')
 try:check(d,row,F,(F/(row['id']+'.tla')).read_bytes());raise RuntimeError('masked '+name)
 except (AssertionError,KeyError,TypeError,ValueError):results.append({'id':name,'outcome':'rejected'})
audited=0
for row in json.loads((B/'stage-separated/report.json').read_text())['rows']:
 if row['outcome'] not in ['exact-raw-ast','primary-invalid-raw-serialization']:continue
 sourceid=row['id'].rsplit('_',1)[0];folder=B/row['folder'];primary=next(r for r in json.loads((folder/'manifest.json').read_text())['rows']if r['id']==sourceid);d=json.loads(gzip.decompress((B/'stage-separated'/(row['id']+'.stdout.gz')).read_bytes()));wanted='Strong'if primary['flags']==['-sf']else'Declared'
 for key in ['translation','checksum']:assert d[key]['Ok']['option_context']['requested']['fairness']==wanted
 audited+=1
# Public validity metadata are tested separately from raw serialization acceptance.
p=json.loads((B/'guides/P-tlc-rewrite.json').read_text())
def public_preconditions(d):
 assert d['accepted']is True and d['exit_code']==0 and d['model_proved']is False and d['exhaustive']is False
 assert d['algorithm_checksum_verified']is True and d['body_checksum_verified']is True
 assert d['translation_options']==d['option_context']['effective'] and d['requested_translation_options']==d['option_context']['requested']
public_preconditions(p)
for name,f in {'public-forged-proof':lambda d:d.update(model_proved=True),'public-forged-exhaustive':lambda d:d.update(exhaustive=True),'public-document-exit':lambda d:d.update(exit_code=3),'public-unsupported-crc':lambda d:d.update(algorithm_checksum_verified=False),'public-inconsistent-context':lambda d:d['translation_options'].update(fairness='Strong')}.items():
 d=copy.deepcopy(p);f(d);(O/(name+'.json')).write_text(json.dumps(d,indent=2)+'\n')
 try:public_preconditions(d);raise RuntimeError('masked '+name)
 except (AssertionError,KeyError,TypeError,ValueError):results.append({'id':name,'outcome':'rejected'})
report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'executed_comparator_sha256':sha(s.encode()),'selected':len(results),'requested_context_audits':audited,'family_certified':False,'rows':results};(O/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Rejected comparator mutations',len(results),'requested context audits',audited,flush=True)
