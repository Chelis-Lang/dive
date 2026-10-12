from pathlib import Path
import json,subprocess,gzip,hashlib,copy,tempfile,collections
import argparse
p=argparse.ArgumentParser()
for key in ['models','binary','out']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();A=a.models.parent;B=a.binary;O=a.out;O.mkdir(exist_ok=False);rows=[]
def sha(b):return hashlib.sha256(b).hexdigest()
report=json.loads((a.models/'report.json').read_text())
for r in report['rows']:
 if not r['id'].endswith('.violation'):continue
 ident=r['id'];name=ident.split('.')[0];first=r['settings'][0]['capture']['stdout']['path'];doc=json.loads(gzip.decompress((a.models/first).read_bytes()));trace=doc['counterexample']['trace']
 with tempfile.TemporaryDirectory(prefix='dive-procedure-replay-')as tmp:
  w=Path(tmp);module=w/(name+'.tla');module.write_bytes((a.models/(ident+'.tla')).read_bytes());cfg=w/(name+'.cfg');cfg.write_bytes((a.models/(ident+'.cfg')).read_bytes())
  for profile in ['dive','tlc']:
   for kind in ['original','invalid-init','invalid-next','missing-final-variable','invalid-closing']:
    t=copy.deepcopy(trace)
    if kind=='invalid-init':t['states'][0]['values'].pop('x',None)
    elif kind=='invalid-next':t['states'][-1]['values']['x']=99
    elif kind=='missing-final-variable':t['states'][-1]['values'].pop('x',None)
    elif kind=='invalid-closing':t['states'].append(copy.deepcopy(t['states'][0]))
    (w/'trace.json').write_text(json.dumps(t));q=subprocess.run([str(B),'validate-trace',module.name,'--config',cfg.name,'--trace','trace.json',*(['--compat=tlc']if profile=='tlc'else[]),'--seed','0','--fp','0','--output','json'],cwd=w,capture_output=True,timeout=30);d=json.loads(q.stdout);key=ident+'.'+profile+'.'+kind
    for stream in ['stdout','stderr']:(O/(key+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
    if kind=='original':assert (q.returncode==0 and d['result']=='valid')or(q.returncode==3 and d['result']=='unsupported'),d
    else:assert q.returncode!=0 and d['result']!='valid',d
    rows.append({'id':key,'profile':profile,'control':kind,'actual_result':d['result'],'exit_code':q.returncode,'outcome':'independently-valid'if d['result']=='valid'else'pending-named-refusal'if d['result']=='unsupported'else'independently-invalid','trace_sha256':sha(json.dumps(t).encode()),'stdout_sha256':sha(q.stdout),'stderr_sha256':sha(q.stderr)})
summary=dict(collections.Counter(r['profile']+'.'+r['actual_result']for r in rows));(O/'report.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'native_sha256':sha(B.read_bytes()),'script_sha256':sha(Path(__file__).read_bytes()),'family_certified':False,'qualified_canonical_is_not_tlc_validation':True,'summary':summary,'rows':rows},indent=2)+'\n');print(summary)
