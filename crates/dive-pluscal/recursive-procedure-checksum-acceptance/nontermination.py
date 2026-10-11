"""Bounded recursive execution is unknown, not a checksum-derived model proof."""
from pathlib import Path
import argparse,gzip,hashlib,json,subprocess,tempfile
p=argparse.ArgumentParser()
for key in ['base','native','out']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();a.out.mkdir(exist_ok=False);rows=[]
sha=lambda b:hashlib.sha256(b).hexdigest()
for syntax in ['C','P']:
 name=f'RecursiveHeader_{syntax}_nonterminating_self';source=a.base/'cases'/(name+'.tla')
 with tempfile.TemporaryDirectory(prefix='dive-recursive-bound-')as tmp:
  work=Path(tmp);file=work/(name+'.tla');file.write_bytes(source.read_bytes());q=subprocess.run([str(a.native),'translate',file.name,'--rewrite','--output','json'],cwd=work,capture_output=True,timeout=30);doc=json.loads(q.stdout);assert q.returncode==0 and doc['accepted']and not doc['model_proved']and not doc['exhaustive'];file.write_text(doc['replacement']['module_source']);(work/(name+'.cfg')).write_text('INIT Init\nNEXT Next\n')
  for profile in ['dive','tlc']:
   for depth in [2,4]:
    command=[str(a.native),'dfid',file.name,'--max-level',str(depth),'--seed','0','--fp','0','--output','json',*(['--compat=tlc']if profile=='tlc'else[])];q=subprocess.run(command,cwd=work,capture_output=True,timeout=30);d=json.loads(q.stdout)
    assert q.returncode==d['exit_code'] and d['result']in ['incomplete','unsupported'] and d['model_proved']is False and d['exhaustive']is False and d['violation_validated']is False,(command,d)
    ident=name+'.'+profile+'.'+str(depth)
    for stream in ['stdout','stderr']:(a.out/(ident+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
    rows.append({'id':ident,'command':command,'exit_code':q.returncode,'actual_result':d['result'],'outcome':'bounded-no-model-proof','stdout_sha256':sha(q.stdout),'stderr_sha256':sha(q.stderr)})
(a.out/'report.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'native_sha256':sha(a.native.read_bytes()),'script_sha256':sha(Path(__file__).read_bytes()),'family_certified':False,'tlc_selection_randomness_not_compared':True,'rows':rows},indent=2)+'\n');print('Closed8bounded recursive controls without model proof')
