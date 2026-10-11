"""Public closing-line admission, positive TLA structure and negative outcomes."""
from pathlib import Path
import argparse,json,subprocess,gzip,hashlib,tempfile,collections
p=argparse.ArgumentParser()
for key in ['base','binary','cst','out']:p.add_argument('--'+key,type=Path,required=True)
p.add_argument('--observe',action='store_true');a=p.parse_args();a.out.mkdir(exist_ok=False);rows=[];manifest=json.loads((a.base/'primary/manifest.json').read_text())
for r in manifest['rows']:
 for profile in ['dive','tlc']:
  for mode in ['translate','rewrite']:
   ident=r['id']+'.'+profile+'.'+mode
   with tempfile.TemporaryDirectory()as tmp:
    file=Path(tmp)/(r['id']+'.tla');file.write_bytes((a.base/'cases'/file.name).read_bytes());q=subprocess.run([str(a.binary),'translate',str(file),'--output','json',*(['--rewrite']if mode=='rewrite'else[]),*(['--compat=tlc']if profile=='tlc'else[])],capture_output=True,timeout=30)
    for stream in ['stdout','stderr']:(a.out/(ident+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
    d=json.loads(q.stdout);assert d['exit_code']==q.returncode and not d['model_proved']and not d['exhaustive']and d['result']!='internal-error';expected=not r['reference_errors']and r.get('sany',{}).get('accepted')is True;outcome='disagree';detail=''
    if bool(d['accepted'])!=expected:detail={'expected_admitted':expected,'actual_admitted':d['accepted'],'reference_errors':r['reference_errors']}
    elif not expected:
     assert q.returncode==1 and d['result']=='rejected';outcome='reference-rejection-uncredited'
    else:
     module=d['replacement']['module_source']if mode=='rewrite'else d['module_source'];file.write_text(module);primary=Path(tmp)/'primary'/file.name;primary.parent.mkdir();primary.write_bytes((a.base/'primary'/(r['id']+'.translated.tla')).read_bytes());left=subprocess.run([str(a.cst),str(file)],capture_output=True,timeout=30);right=subprocess.run([str(a.cst),str(primary)],capture_output=True,timeout=30)
     for tag,process in [('native-cst',left),('primary-cst',right)]:
      for stream in ['stdout','stderr']:(a.out/(ident+'.'+tag+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(process,stream),mtime=0))
     l=json.loads(left.stdout);rr=json.loads(right.stdout);assert left.returncode==right.returncode==0 and l['accepted']and rr['accepted']and l['tree']==rr['tree'];outcome='admitted-exact-cst'
   rows.append({'id':ident,'outcome':outcome,'detail':detail})
summary=dict(collections.Counter(r['outcome']for r in rows));(a.out/'report.json').write_text(json.dumps({'schema_version':1,'binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'selection_sha256':hashlib.sha256((a.base/'selection.json').read_bytes()).hexdigest(),'selected':len(rows),'summary':summary,'family_certified':False,'rows':rows},indent=2)+'\n');print(summary,flush=True);raise SystemExit(bool(summary.get('disagree'))and not a.observe)
